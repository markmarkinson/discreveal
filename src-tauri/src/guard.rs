//! Path validation for commands that act on filesystem entries.
//!
//! The frontend is treated as untrusted input: a path is only accepted if it
//! resolves to an entry strictly inside the root of the most recent scan.
//! Operating-system items are additionally classified as protected; deleting
//! them requires an explicit acknowledgement from the user.

use crate::error::{AppError, AppResult};
use crate::scan::ScanState;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::{env, fs};

/// Identifies a result whose roots were validated by the backend, including cached scans.
#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultScope {
    kind: ResultKind,
    id: u64,
}

impl ResultScope {
    pub(crate) fn scan_id(self) -> Option<u64> {
        (self.kind == ResultKind::Scan).then_some(self.id)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResultKind {
    Scan,
    Duplicates,
}

/// Session-only authorization registry; callers supply an identity, never arbitrary roots.
#[derive(Default)]
pub struct ResultRoots(Mutex<HashMap<ResultScope, Vec<PathBuf>>>);

impl ResultRoots {
    pub fn register(&self, kind: ResultKind, id: u64, roots: Vec<PathBuf>) {
        let mut entries = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if kind == ResultKind::Duplicates {
            entries.retain(|scope, _| scope.kind != kind);
        }
        entries.insert(ResultScope { kind, id }, roots);
    }
}

/// Validates against the exact backend-registered result, falling back to the current scan
/// only for older clients that do not yet provide a result identity.
pub fn resolve_for_result(
    state: &ScanState,
    registry: &ResultRoots,
    scope: Option<ResultScope>,
    raw: &str,
    allow_root: bool,
) -> AppResult<PathBuf> {
    let Some(scope) = scope else {
        return if allow_root {
            resolve_within_scan_root(state, raw)
        } else {
            resolve_in_scan_root_verbatim(state, raw)
        };
    };
    let entries = registry
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let roots = entries
        .get(&scope)
        .cloned()
        .ok_or_else(|| AppError::new("noScanResult"))?;
    drop(entries);
    // Preserve invalid/missing-path errors; only try another root on a genuine scope mismatch.
    for root in &roots {
        let result = if allow_root {
            resolve_within_root(root, raw)
        } else {
            resolve_canonical_in_root(root, raw)
        };
        match result {
            Ok(path) => return Ok(path),
            Err(error) if error.code == "outsideScanRoot" => {}
            Err(error) => return Err(error),
        }
    }
    Err(AppError::new("outsideScanRoot"))
}

const VERBATIM_PREFIX: &str = r"\\?\";

/// Top-level folders of the system drive that are protected themselves.
const PROTECTED_SYSTEM_DRIVE_FOLDERS: &[(&str, Protection)] = &[
    ("program files", Protection::Programs),
    ("program files (x86)", Protection::Programs),
    ("programdata", Protection::Programs),
    ("users", Protection::Profile),
    ("recovery", Protection::SystemData),
    ("boot", Protection::SystemData),
];

/// Folders on any drive that are protected at every depth.
const PROTECTED_ON_EVERY_DRIVE: &[&str] = &["system volume information", "$recycle.bin"];

/// Files that Windows needs; matched by file name (case-insensitive prefix
/// for registry hives, which come with transaction-log siblings).
const PROTECTED_FILE_NAMES: &[&str] = &[
    "pagefile.sys",
    "hiberfil.sys",
    "swapfile.sys",
    "bootmgr",
    "bootnxt",
];
const PROTECTED_FILE_PREFIXES: &[&str] = &["ntuser.dat", "usrclass.dat"];

/// Why an item is protected. The frontend shows a matching warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// Part of the Windows folder.
    Windows,
    /// Program installation or program data folder.
    Programs,
    /// User profile or the folder holding all profiles.
    Profile,
    /// Recovery, boot, recycle bin or volume metadata.
    SystemData,
    /// A file Windows needs to run, such as the page file or a registry hive.
    SystemFile,
}

impl Protection {
    /// Stable identifier used by the frontend.
    pub fn code(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Programs => "programs",
            Self::Profile => "profile",
            Self::SystemData => "systemData",
            Self::SystemFile => "systemFile",
        }
    }
}

/// Resolves a path strictly inside the current scan, retaining the Windows verbatim prefix.
/// Shell callers convert the validated path with `plain_path`; direct filesystem calls keep it.
pub fn resolve_in_scan_root_verbatim(state: &ScanState, raw: &str) -> AppResult<PathBuf> {
    resolve_canonical_in_scan_root(state, raw)
}

fn resolve_canonical_in_scan_root(state: &ScanState, raw: &str) -> AppResult<PathBuf> {
    let root = state.root().ok_or_else(|| AppError::new("noScanResult"))?;
    resolve_canonical_in_root(&root, raw)
}

fn resolve_canonical_in_root(root: &Path, raw: &str) -> AppResult<PathBuf> {
    let candidate = Path::new(raw);
    if !candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::new("invalidPath"));
    }

    let canonical = canonicalize_entry(candidate)
        .map_err(|error| AppError::with_detail("pathNotFound", error))?;

    if canonical == root {
        return Err(AppError::new("scanRootProtected"));
    }
    if !canonical.starts_with(root) {
        return Err(AppError::new("outsideScanRoot"));
    }

    Ok(canonical)
}

/// Resolves `raw` the same way as [`resolve_in_scan_root_verbatim`], but also
/// accepts the scan root itself. Used by read-only listing commands
/// (re-reading a folded directory's children) rather than delete or open, so
/// there is nothing unsafe about the target being the root. Returns the
/// verbatim form, like the delete/open split above: the only caller today
/// (`expand_folder`) reads the filesystem directly.
pub fn resolve_within_scan_root(state: &ScanState, raw: &str) -> AppResult<PathBuf> {
    let root = state.root().ok_or_else(|| AppError::new("noScanResult"))?;
    resolve_within_root(&root, raw)
}

fn resolve_within_root(root: &Path, raw: &str) -> AppResult<PathBuf> {
    let candidate = Path::new(raw);
    if !candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::new("invalidPath"));
    }

    let canonical = canonicalize_entry(candidate)
        .map_err(|error| AppError::with_detail("pathNotFound", error))?;

    if canonical != root && !canonical.starts_with(root) {
        return Err(AppError::new("outsideScanRoot"));
    }

    // Verbatim, not plain: expand_folder's only caller reads the filesystem directly.
    Ok(canonical)
}

/// Canonical form of `path`. Files that are locked by the system, such as the
/// page file, cannot be opened for canonicalisation, so the entry is resolved
/// through its canonical parent folder instead.
fn canonicalize_entry(path: &Path) -> std::io::Result<PathBuf> {
    fs::canonicalize(path).or_else(|error| {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(error);
        };
        let resolved = fs::canonicalize(parent)?.join(name);
        fs::symlink_metadata(&resolved)?;
        Ok(resolved)
    })
}

/// Classifies operating-system locations. Everything below the Windows
/// folder is protected. Of the other well-known folders only the folder
/// itself (and each user profile) is protected, so removing a single program
/// folder or leftover data does not trigger the warning.
pub fn protection_of(path: &Path) -> Option<Protection> {
    let system_root = env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    classify_protection(
        &path.to_string_lossy().to_lowercase(),
        &system_root.to_lowercase(),
    )
}

/// `path` and `system_root` are lower case, e.g. `c:\windows`.
fn classify_protection(path: &str, system_root: &str) -> Option<Protection> {
    let system_drive = system_root.split('\\').next().unwrap_or_default();

    if path == system_root
        || path
            .strip_prefix(system_root)
            .is_some_and(|rest| rest.starts_with('\\'))
    {
        return Some(Protection::Windows);
    }

    let Some((drive, rest)) = path.split_once('\\') else {
        return Some(Protection::SystemData);
    };
    let components: Vec<&str> = rest.split('\\').filter(|part| !part.is_empty()).collect();
    let Some(&first) = components.first() else {
        return Some(Protection::SystemData);
    };

    if PROTECTED_ON_EVERY_DRIVE.contains(&first) {
        return Some(Protection::SystemData);
    }

    let file_name = components.last().copied().unwrap_or_default();
    let is_system_file = (components.len() == 1 && PROTECTED_FILE_NAMES.contains(&file_name))
        || PROTECTED_FILE_PREFIXES
            .iter()
            .any(|prefix| file_name.starts_with(prefix));
    if is_system_file {
        return Some(Protection::SystemFile);
    }

    if drive != system_drive {
        return None;
    }
    if components.len() == 1
        && let Some((_, protection)) = PROTECTED_SYSTEM_DRIVE_FOLDERS
            .iter()
            .find(|(name, _)| *name == first)
    {
        return Some(*protection);
    }
    // A user profile itself, e.g. `c:\users\name`.
    (first == "users" && components.len() == 2).then_some(Protection::Profile)
}

/// Strips the `\\?\` prefix that `fs::canonicalize` adds on Windows.
///
/// Only plain drive-letter paths are converted; UNC paths keep their prefix.
/// Shell APIs (recycle bin, Explorer) handle the plain form more reliably.
pub fn plain_path(path: PathBuf) -> PathBuf {
    match path
        .to_str()
        .and_then(|text| text.strip_prefix(VERBATIM_PREFIX))
    {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// Shell parsing normalizes trailing dots/spaces and DOS device names. Never
/// pass such an entry to a destructive Shell action under a different name.
pub(crate) fn checked_shell_path(path: &Path) -> AppResult<PathBuf> {
    for component in path.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let Some(name) = name.to_str() else {
            return Err(AppError::new("unsafeShellPath"));
        };
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches([' ', '.'])
            .to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || stem
                .strip_prefix("COM")
                .or_else(|| stem.strip_prefix("LPT"))
                .is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                });
        if name.ends_with([' ', '.']) || name.contains(':') || device {
            return Err(AppError::new("unsafeShellPath"));
        }
    }
    Ok(plain_path(path.to_path_buf()))
}

/// Keep ancestors open while mutating their children, denying parent rename
/// and refusing every reparse/offline directory. Handles are opened top-down.
pub(crate) fn hold_mutation_parents(path: &Path) -> AppResult<Vec<fs::File>> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::*;
    let parent = path.parent().ok_or_else(|| AppError::new("invalidPath"))?;
    let mut parents: Vec<_> = parent
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    parents.reverse();
    let mut held = Vec::with_capacity(parents.len());
    for parent in parents {
        let file = fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(parent)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir()
            || metadata.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE)
                != 0
        {
            return Err(AppError::new("invalidPath"));
        }
        held.push(file);
    }
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r"c:\windows";

    #[test]
    fn shell_paths_reject_names_that_windows_would_normalize() {
        for name in [
            r"\\?\C:\data\copy.txt ",
            r"\\?\C:\data.\copy.txt",
            r"\\?\C:\data\NUL.txt",
            r"\\?\C:\data\CON .txt",
            r"\\?\C:\data\COM¹.txt",
            r"\\?\C:\data\copy.txt:secret",
        ] {
            assert!(checked_shell_path(Path::new(name)).is_err(), "{name}");
        }
        assert_eq!(
            checked_shell_path(Path::new(r"\\?\C:\data\copy.txt")).unwrap(),
            PathBuf::from(r"C:\data\copy.txt")
        );
    }

    #[test]
    fn held_mutation_parents_block_rename_until_released() {
        let base = env::temp_dir().join(format!("discreveal_parent_lock_{}", std::process::id()));
        let parent = base.join("parent");
        fs::create_dir_all(&parent).unwrap();
        let target = parent.join("file.txt");
        fs::write(&target, b"unchanged").unwrap();
        let held = hold_mutation_parents(&target).unwrap();
        let moved = base.join("moved");
        assert!(fs::rename(&parent, &moved).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"unchanged");
        drop(held);
        fs::rename(&parent, &moved).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    fn protection(path: &str) -> Option<Protection> {
        classify_protection(path, ROOT)
    }

    #[test]
    fn result_scopes_keep_cached_scans_and_duplicate_roots_independent() {
        let base = env::temp_dir().join(format!("discreveal_scopes_{}", std::process::id()));
        let a = base.join("a");
        let b = base.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("file"), b"a").unwrap();
        fs::write(b.join("file"), b"b").unwrap();
        let roots = ResultRoots::default();
        let state = ScanState::new();
        let a_real = fs::canonicalize(&a).unwrap();
        let b_real = fs::canonicalize(&b).unwrap();
        roots.register(ResultKind::Scan, 1, vec![a_real.clone()]);
        roots.register(ResultKind::Scan, 2, vec![b_real.clone()]);
        roots.register(ResultKind::Duplicates, 1, vec![a_real, b_real]);
        let check = |kind, id, path: &Path| {
            resolve_for_result(
                &state,
                &roots,
                Some(ResultScope { kind, id }),
                &path.to_string_lossy(),
                false,
            )
        };
        assert!(check(ResultKind::Scan, 1, &a.join("file")).is_ok());
        assert!(check(ResultKind::Scan, 2, &a.join("file")).is_err());
        assert!(check(ResultKind::Scan, 1, &b.join("file")).is_err());
        assert!(check(ResultKind::Duplicates, 1, &a.join("file")).is_ok());
        assert!(check(ResultKind::Duplicates, 1, &b.join("file")).is_ok());
        assert!(check(ResultKind::Duplicates, 1, &a).is_err());
        assert!(check(ResultKind::Scan, 999, &a.join("file")).is_err());
        roots.register(
            ResultKind::Duplicates,
            2,
            vec![fs::canonicalize(&a).unwrap()],
        );
        assert!(check(ResultKind::Duplicates, 1, &a.join("file")).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn windows_folder_is_protected_at_every_depth() {
        assert_eq!(protection(r"c:\windows"), Some(Protection::Windows));
        assert_eq!(
            protection(r"c:\windows\system32\drivers"),
            Some(Protection::Windows)
        );
        assert_eq!(protection(r"c:\windows.old"), None);
    }

    #[test]
    fn well_known_folders_are_protected_only_themselves() {
        assert_eq!(protection(r"c:\program files"), Some(Protection::Programs));
        assert_eq!(
            protection(r"c:\program files (x86)"),
            Some(Protection::Programs)
        );
        assert_eq!(protection(r"c:\users"), Some(Protection::Profile));
        assert_eq!(protection(r"c:\program files\some app"), None);
        assert_eq!(protection(r"c:\programdata\vendor\cache"), None);
    }

    #[test]
    fn user_profile_is_protected_but_its_contents_are_not() {
        assert_eq!(protection(r"c:\users\name"), Some(Protection::Profile));
        assert_eq!(protection(r"c:\users\name\downloads"), None);
        assert_eq!(protection(r"c:\users\name\downloads\big.iso"), None);
    }

    #[test]
    fn registry_hives_and_special_files_are_protected() {
        assert_eq!(protection(r"c:\pagefile.sys"), Some(Protection::SystemFile));
        assert_eq!(protection(r"c:\hiberfil.sys"), Some(Protection::SystemFile));
        assert_eq!(
            protection(r"c:\users\name\ntuser.dat"),
            Some(Protection::SystemFile)
        );
        assert_eq!(
            protection(r"c:\users\name\ntuser.dat.log1"),
            Some(Protection::SystemFile)
        );
        assert_eq!(protection(r"d:\pagefile.sys"), Some(Protection::SystemFile));
        assert_eq!(protection(r"d:\backups\pagefile.sys"), None);
    }

    #[test]
    fn system_metadata_folders_are_protected_on_every_drive() {
        assert_eq!(
            protection(r"d:\system volume information"),
            Some(Protection::SystemData)
        );
        assert_eq!(
            protection(r"d:\$recycle.bin\s-1-5"),
            Some(Protection::SystemData)
        );
    }

    #[test]
    fn other_drives_are_unrestricted() {
        assert_eq!(protection(r"d:\program files"), None);
        assert_eq!(protection(r"d:\users"), None);
        assert_eq!(protection(r"d:\games\big"), None);
    }

    #[test]
    fn drive_roots_are_protected() {
        assert_eq!(protection(r"d:\"), Some(Protection::SystemData));
    }

    /// Regression test for a real bug report: deleting a file whose name ends in a space
    /// (or a dot) failed with "file not found" even though it was right there in the list.
    /// Win32 silently drops a trailing dot/space from an ordinary path before a filesystem
    /// call reaches the disk — unless the path uses the verbatim `\\?\` form, which passes
    /// the name through exactly as given. `resolve_in_scan_root`'s plain result hits that
    /// normalization and therefore names a *different*, non-existent file;
    /// `resolve_in_scan_root_verbatim`'s does not.
    #[test]
    fn verbatim_path_deletes_trailing_space_names_the_plain_path_cannot() {
        let base =
            env::temp_dir().join(format!("discreveal_guard_trailing_{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let canonical_root = fs::canonicalize(&base).unwrap();

        // Created through the canonical (verbatim) root, so the trailing space is honoured.
        let file = format!("{}\\trailing name ", canonical_root.display());
        fs::write(&file, b"x").unwrap();
        assert!(
            fs::metadata(&file).is_ok(),
            "setup: file should exist under its exact name"
        );

        let raw = format!("{}\\trailing name ", base.display());

        let plain = resolve_canonical_in_root(&canonical_root, &raw)
            .map(plain_path)
            .unwrap();
        assert!(
            fs::remove_file(&plain).is_err(),
            "plain path should fail to find the real file"
        );

        let verbatim = resolve_canonical_in_root(&canonical_root, &raw).unwrap();
        fs::remove_file(&verbatim).expect("verbatim path should delete the real file");

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn scan_root_containment() {
        let base = env::temp_dir().join(format!("discreveal_guard_{}", std::process::id()));
        let root = base.join("root");
        let sibling = base.join("rootx");
        fs::create_dir_all(root.join("inner")).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::write(root.join("inner").join("file.txt"), b"x").unwrap();
        fs::write(sibling.join("other.txt"), b"x").unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        let inside = root.join("inner").join("file.txt");
        assert!(resolve_canonical_in_root(&canonical_root, &inside.to_string_lossy()).is_ok());
        assert!(resolve_canonical_in_root(&canonical_root, &root.to_string_lossy()).is_err());
        assert!(
            resolve_canonical_in_root(
                &canonical_root,
                &sibling.join("other.txt").to_string_lossy()
            )
            .is_err()
        );
        assert!(resolve_canonical_in_root(&canonical_root, "relative\\path.txt").is_err());
        let traversal = format!("{}\\inner\\..\\..\\rootx\\other.txt", root.display());
        assert!(resolve_canonical_in_root(&canonical_root, &traversal).is_err());

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn resolve_within_root_accepts_the_root_itself() {
        let base = env::temp_dir().join(format!("discreveal_guard_within_{}", std::process::id()));
        let root = base.join("root");
        let sibling = base.join("rootx");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        assert!(resolve_within_root(&canonical_root, &root.to_string_lossy()).is_ok());
        assert!(resolve_within_root(&canonical_root, &sibling.to_string_lossy()).is_err());
        assert!(resolve_within_root(&canonical_root, "relative\\path").is_err());

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn verbatim_prefix_is_removed_for_drive_paths_only() {
        assert_eq!(
            plain_path(PathBuf::from(r"\\?\C:\a")),
            PathBuf::from(r"C:\a")
        );
        assert_eq!(
            plain_path(PathBuf::from(r"\\?\UNC\srv\share")),
            PathBuf::from(r"\\?\UNC\srv\share")
        );
    }
}
