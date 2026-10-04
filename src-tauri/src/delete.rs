//! Deletion of scanned entries: recycle bin, permanent and file overwriting.

use crate::error::{AppError, AppResult};
use crate::scan::ScanState;
use crate::{drives, guard};
use rand::RngCore;

mod recycle;
use serde::Deserialize;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::*;

const OVERWRITE_CHUNK_BYTES: u64 = 256 * 1024;

/// Nesting depth at which secure deletion gives up instead of risking a
/// stack overflow on a hostile directory structure.
const MAX_SECURE_DELETE_DEPTH: u32 = 200;

/// How an entry is removed.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeleteMode {
    /// Move to the recycle bin (recoverable).
    Trash,
    /// Remove from the filesystem without overwriting.
    Permanent,
    /// Overwrite file contents with random data, then remove.
    Secure,
}

/// Reports why an entry is protected, or `None` if it is an ordinary item.
///
/// The frontend asks before showing the confirmation dialog, so protected
/// items get an explicit warning.
#[tauri::command(async)]
pub fn get_protection(
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    scope: Option<guard::ResultScope>,
    target_path: String,
) -> AppResult<Option<&'static str>> {
    let target = guard::plain_path(guard::resolve_for_result(
        &state,
        &result_roots,
        scope,
        &target_path,
        false,
    )?);
    Ok(guard::protection_of(&target).map(guard::Protection::code))
}

/// Deletes an entry inside the current scan root.
///
/// The path is validated by [`guard::resolve_for_result`], so neither the
/// scan root itself nor anything outside it can be removed. Protected
/// operating-system items are only deleted if the caller states that the
/// user acknowledged the warning.
#[tauri::command(async)]
pub fn delete_item(
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    scope: Option<guard::ResultScope>,
    target_path: String,
    mode: DeleteMode,
    acknowledge_protected: bool,
) -> AppResult<()> {
    let target = guard::resolve_for_result(&state, &result_roots, scope, &target_path, false)?;
    let plain = guard::plain_path(target.clone());
    if guard::protection_of(&plain).is_some() && !acknowledge_protected {
        return Err(AppError::new("protectedNotAcknowledged"));
    }

    let _raw_parents = hold_original_path(Path::new(&target_path))?;
    let _parents = guard::hold_mutation_parents(&target)?;
    match mode {
        // The Recycle Bin goes through a Shell API, which — like protection_of's string
        // matching above — needs the plain path, not the verbatim one used below.
        DeleteMode::Trash => move_to_recycle_bin(&guard::checked_shell_path(&target)?),
        // Handle-based deletion retains the verbatim (\\?\) name. Win32's
        // ordinary path parser may normalize trailing dots/spaces to another
        // entry, so no plain-path fallback is permitted for these modes.
        DeleteMode::Permanent => remove_locked(&target, false, 0),
        DeleteMode::Secure => secure_delete(&target, 0),
    }
}

/// Duplicate actions always name a retained original and revalidate content before deleting.
#[tauri::command(async)]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri command parameters remain explicit at the IPC boundary"
)]
pub fn delete_duplicate(
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    duplicates: State<crate::duplicates::DuplicatesState>,
    scope: Option<guard::ResultScope>,
    target_path: String,
    keeper_path: String,
    expected_size: u64,
    mode: DeleteMode,
    acknowledge_protected: bool,
) -> AppResult<()> {
    let (operation, cancelled) = duplicates.begin_cleanup()?;
    let target = guard::resolve_for_result(&state, &result_roots, scope, &target_path, false)?;
    let keeper = guard::resolve_for_result(&state, &result_roots, scope, &keeper_path, false)?;
    if guard::protection_of(&guard::plain_path(target.clone())).is_some() && !acknowledge_protected
    {
        return Err(AppError::new("protectedNotAcknowledged"));
    }
    let _raw_target_parents = hold_original_path(Path::new(&target_path))?;
    let _raw_keeper_parents = hold_original_path(Path::new(&keeper_path))?;
    let _target_parents = guard::hold_mutation_parents(&target)?;
    let _keeper_parents = guard::hold_mutation_parents(&keeper)?;
    let mut keeper_file = open_locked(&keeper, false, false, false)?;
    let mut target_file = open_locked(
        &target,
        true,
        matches!(mode, DeleteMode::Secure),
        matches!(mode, DeleteMode::Trash),
    )?;
    compare_locked_pair(
        &mut target_file,
        &mut keeper_file,
        expected_size,
        &cancelled,
    )?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::new("duplicateCleanupCancelled"));
    }
    let result = match mode {
        DeleteMode::Trash => {
            // The original stays readable, unmodifiable and unrenamable until
            // the Shell operation completes. Shell still requires DELETE sharing.
            let shell_path = guard::checked_shell_path(&target)?;
            // Carry the compared object's identity into the Shell worker. A
            // fresh lookup must never become the new expected object.
            move_locked_to_recycle_bin(&shell_path, &target_file)
        }
        DeleteMode::Permanent => dispose_locked(&target_file),
        DeleteMode::Secure => {
            overwrite_locked(&mut target_file)?;
            dispose_locked(&target_file)
        }
    };
    if result.is_ok() {
        operation.finish(false);
    }
    result
}

fn move_to_recycle_bin(target: &Path) -> AppResult<()> {
    let held = open_locked(target, false, false, true)?;
    move_locked_to_recycle_bin(target, &held)
}

fn move_locked_to_recycle_bin(target: &Path, held: &fs::File) -> AppResult<()> {
    let shell_path = guard::checked_shell_path(target)?;
    if !drives::is_fixed_drive(&shell_path) {
        return Err(AppError::new("noRecycleBin"));
    }
    let expected = file_identity(held)?;
    // IFileOperation requires an STA. Tauri commands may execute on an MTA,
    // so each operation owns a short-lived apartment instead of reusing one.
    std::thread::Builder::new()
        .name("discReveal-recycle".into())
        .spawn(move || recycle::recycle_checked(&shell_path, expected))
        .map_err(|error| AppError::with_detail("recycleBinFailed", error))?
        .join()
        .map_err(|_| AppError::new("recycleBinFailed"))?
}

fn ensure_recycle_identity(target: &Path, expected: (u64, [u8; 16])) -> AppResult<fs::File> {
    let current = open_locked(target, false, false, true)?;
    if file_identity(&current)? != expected
        || current.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(AppError::new("duplicateChanged"));
    }
    Ok(current)
}

fn hold_original_path(path: &Path) -> AppResult<Vec<fs::File>> {
    let held = guard::hold_mutation_parents(path)?;
    if fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(AppError::new("invalidPath"));
    }
    Ok(held)
}

#[cfg(test)]
fn remove_permanently(target: &Path) -> io::Result<()> {
    remove_locked(target, false, 0).map_err(|error| io::Error::other(error.to_string()))
}

/// Read and (optionally) mutate the same object, denying concurrent content
/// changes. Only the Shell variant permits DELETE sharing; see the documented
/// remaining Shell path-rename limitation.
fn open_locked(path: &Path, deleting: bool, writing: bool, shell: bool) -> AppResult<fs::File> {
    Ok(fs::OpenOptions::new()
        .access_mode(
            FILE_READ_ATTRIBUTES
                | FILE_READ_DATA
                | if deleting { DELETE } else { 0 }
                | if writing { FILE_WRITE_DATA } else { 0 },
        )
        .share_mode(FILE_SHARE_READ | if shell { FILE_SHARE_DELETE } else { 0 })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?)
}

fn file_information(file: &fs::File) -> AppResult<BY_HANDLE_FILE_INFORMATION> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: checked live file handle and correctly sized output structure.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(info)
}
fn file_identity(file: &fs::File) -> AppResult<(u64, [u8; 16])> {
    let mut info: FILE_ID_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: live handle and correctly sized output structure. Use the full
    // 128-bit ID; the legacy 64-bit index may be truncated on ReFS. Unsupported
    // filesystems fail closed instead of comparing incomplete identities.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as HANDLE,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error().into());
    }
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}
fn dispose_locked(file: &fs::File) -> AppResult<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: DELETE access belongs to this same held handle, never a reopened path.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

fn compare_locked_pair(
    target: &mut fs::File,
    keeper: &mut fs::File,
    expected_size: u64,
    cancel: &AtomicBool,
) -> AppResult<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::new("duplicateCleanupCancelled"));
    }
    ensure_plain_single_link_file(target)?;
    ensure_plain_single_link_file(keeper)?;
    // Identical ordinary bytes do not prove identical alternate streams.
    // Refuse removal rather than discard unique data hidden in an ADS.
    ensure_duplicate_streams(target)?;
    ensure_duplicate_streams(keeper)?;
    if file_identity(target)? == file_identity(keeper)?
        || !target.metadata()?.is_file()
        || !keeper.metadata()?.is_file()
        || target.metadata()?.len() != expected_size
        || keeper.metadata()?.len() != expected_size
    {
        return Err(AppError::new("duplicateChanged"));
    }
    target.seek(SeekFrom::Start(0))?;
    keeper.seek(SeekFrom::Start(0))?;
    let mut a = vec![0u8; OVERWRITE_CHUNK_BYTES as usize];
    let mut b = vec![0u8; OVERWRITE_CHUNK_BYTES as usize];
    let mut remaining = expected_size;
    while remaining > 0 {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::new("duplicateCleanupCancelled"));
        }
        let bytes = remaining.min(OVERWRITE_CHUNK_BYTES) as usize;
        target.read_exact(&mut a[..bytes])?;
        keeper.read_exact(&mut b[..bytes])?;
        if a[..bytes] != b[..bytes] {
            return Err(AppError::new("duplicateChanged"));
        }
        remaining -= bytes as u64;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::new("duplicateCleanupCancelled"));
    }
    Ok(())
}

/// Query streams on the held file, rather than enumerating a changing path.
pub(crate) fn duplicate_cleanup_problem(path: &Path) -> Option<&'static str> {
    let result = fs::File::open(path)
        .map_err(AppError::from)
        .and_then(|file| {
            ensure_plain_single_link_file(&file)?;
            ensure_duplicate_streams(&file)
        });
    result.err().map(|error| error.code)
}

fn ensure_duplicate_streams(file: &fs::File) -> AppResult<()> {
    ensure_only_default_stream(file).map_err(|error| {
        AppError::new(match error.code {
            "secureUnsupportedFilesystem" => "duplicateUnsupportedFilesystem",
            _ => "duplicateAdditionalStreams",
        })
    })
}

/// Unsupported stream enumeration fails closed; alternate streams are not
/// covered by our ordinary-data-stream overwrite.
fn ensure_only_default_stream(file: &fs::File) -> AppResult<()> {
    let mut buffer = vec![0u64; 8192];
    let size = (buffer.len() * std::mem::size_of::<u64>()) as u32;
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as HANDLE,
            FileStreamInfo,
            buffer.as_mut_ptr().cast(),
            size,
        )
    } == 0
    {
        return Err(AppError::with_detail(
            "secureUnsupportedFilesystem",
            io::Error::last_os_error(),
        ));
    }
    let raw = buffer.as_ptr().cast::<u8>();
    let mut offset = 0usize;
    loop {
        let name_offset = std::mem::offset_of!(FILE_STREAM_INFO, StreamName);
        if offset + std::mem::size_of::<FILE_STREAM_INFO>() > size as usize {
            return Err(AppError::new("secureSpecialFile"));
        }
        let stream = unsafe { &*raw.add(offset).cast::<FILE_STREAM_INFO>() };
        let bytes = stream.StreamNameLength as usize;
        if !bytes.is_multiple_of(2) || offset + name_offset + bytes > size as usize {
            return Err(AppError::new("secureSpecialFile"));
        }
        let name = unsafe {
            std::slice::from_raw_parts(raw.add(offset + name_offset).cast::<u16>(), bytes / 2)
        };
        if String::from_utf16_lossy(name) != "::$DATA" {
            return Err(AppError::new("secureSpecialFile"));
        }
        if stream.NextEntryOffset == 0 {
            break;
        }
        let next = stream.NextEntryOffset as usize;
        if next < name_offset || !next.is_multiple_of(8) || offset + next >= size as usize {
            return Err(AppError::new("secureSpecialFile"));
        }
        offset += next;
    }
    Ok(())
}
fn overwrite_locked(file: &mut fs::File) -> AppResult<()> {
    ensure_plain_single_link_file(file)?;
    let info = file_information(file)?;
    if info.dwFileAttributes
        & (FILE_ATTRIBUTE_COMPRESSED
            | FILE_ATTRIBUTE_ENCRYPTED
            | FILE_ATTRIBUTE_SPARSE_FILE
            | FILE_ATTRIBUTE_OFFLINE)
        != 0
    {
        return Err(AppError::new("secureSpecialFile"));
    }
    ensure_only_default_stream(file)?;
    // The current length comes from this exclusive held object, not an earlier
    // path snapshot. No concurrent write or rename is shared in secure mode.
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(0))?;
    let mut rng = rand::thread_rng();
    let mut buffer = vec![0u8; size.min(OVERWRITE_CHUNK_BYTES) as usize];
    let mut written = 0;
    while written < size {
        let bytes = (size - written).min(OVERWRITE_CHUNK_BYTES) as usize;
        rng.fill_bytes(&mut buffer[..bytes]);
        file.write_all(&buffer[..bytes])?;
        written += bytes as u64;
    }
    file.sync_all()?;
    Ok(())
}

/// Parent handles are held by the caller. Each directory handle stays held
/// throughout child traversal so directories cannot be replaced by junctions.
fn remove_locked(target: &Path, secure: bool, depth: u32) -> AppResult<()> {
    if depth > MAX_SECURE_DELETE_DEPTH {
        return Err(AppError::new("secureTooDeep"));
    }
    let mut file = open_locked(target, true, secure, false)?;
    let metadata = file.metadata()?;
    if file_information(&file)?.dwFileAttributes
        & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE)
        != 0
    {
        return Err(AppError::new("secureIsLink"));
    }
    if metadata.is_dir() {
        for child in fs::read_dir(target)? {
            remove_locked(&child?.path(), secure, depth + 1)?;
        }
    } else if metadata.is_file() {
        if secure {
            overwrite_locked(&mut file)?;
        }
    } else {
        return Err(AppError::new("invalidPath"));
    }
    dispose_locked(&file)
}
fn secure_delete(target: &Path, depth: u32) -> AppResult<()> {
    remove_locked(target, true, depth)
}
#[cfg(test)]
fn secure_delete_file(target: &Path) -> AppResult<()> {
    remove_locked(target, true, 0)
}

/// Rejects reparse points and files that have other hard links.
pub(crate) fn ensure_plain_single_link_file(file: &fs::File) -> AppResult<()> {
    // SAFETY: `info` is a valid out structure and the handle belongs to `file`, which outlives the call.
    let info = unsafe {
        let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        if GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) == 0 {
            return Err(io::Error::last_os_error().into());
        }
        info
    };
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(AppError::new("secureIsLink"));
    }
    if info.nNumberOfLinks > 1 {
        return Err(AppError::new("secureHardLinks"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    struct ScopedTestDir(std::path::PathBuf);
    struct RestoreTestItem(std::path::PathBuf);

    /// The test dependency exposes a Shell display name, which can hide .htm
    /// even though the actual recycled file retains that extension. Its restore
    /// API also uses that display name as destination name. Restrict lookup to
    /// our unique fixture directory and supply our known original leaf name;
    /// general user-file restoration must not make this assumption.
    fn fixture_trash_item(path: &Path) -> Result<trash::TrashItem, String> {
        let original = guard::plain_path(path.to_path_buf());
        let parent = original.parent().ok_or("fixture has no parent")?;
        let root = guard::plain_path(fs::canonicalize(env::temp_dir()).map_err(|e| e.to_string())?);
        let scope = parent
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid fixture scope")?;
        let suffix = scope
            .strip_prefix("discreveal_recycle_fixture_")
            .ok_or("not our fixture scope")?;
        if parent.parent() != Some(root.as_path())
            || suffix.len() != 16
            || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("fixture scope must be a unique direct child of Temp".into());
        }
        let name = original.file_name().ok_or("fixture has no file name")?;
        if !matches!(
            name.to_str(),
            Some(
                "discReveal-recycle-fixture.bin"
                    | "discReveal-recycle-fixture-folder"
                    | "discReveal-recycle-fixture.htm"
            )
        ) {
            return Err("not a known fixture leaf name".into());
        }
        let mut entries: Vec<_> = trash::os_limited::list()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|item| item.original_parent == parent)
            .collect();
        if entries.len() != 1 {
            return Err(format!(
                "expected exactly one recycled item from our unique fixture scope, got {}",
                entries.len()
            ));
        }
        let mut entry = entries.pop().ok_or("missing fixture item")?;
        if entry.name != name && Some(entry.name.as_os_str()) != original.file_stem() {
            return Err("unexpected recycled fixture display name".into());
        }
        entry.name = name.to_os_string();
        Ok(entry)
    }

    impl Drop for RestoreTestItem {
        fn drop(&mut self) {
            if self.0.exists() {
                return;
            }
            // This guard precedes mutation handles in each test, so unwinding
            // drops those handles before restoring this exact fixture item.
            if let Ok(entry) = fixture_trash_item(&self.0)
                && let Err(error) = trash::os_limited::restore_all([entry])
            {
                eprintln!(
                    "Could not restore isolated fixture {}: {error}",
                    self.0.display()
                );
            }
        }
    }

    impl ScopedTestDir {
        fn new() -> Self {
            let root = fs::canonicalize(env::temp_dir()).unwrap();
            let path = root.join(format!(
                "discreveal_recycle_fixture_{:016x}",
                rand::random::<u64>()
            ));
            // Atomic create: never reuse or recursively clear somebody else's
            // pre-existing directory, even in an unlikely random collision.
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScopedTestDir {
        fn drop(&mut self) {
            let root = fs::canonicalize(env::temp_dir()).unwrap();
            assert_eq!(self.0.parent(), Some(root.as_path()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("discreveal_recycle_fixture_")
            );
            if self.0.exists() {
                let resolved = fs::canonicalize(&self.0).unwrap();
                assert_eq!(resolved, self.0);
                fs::remove_dir_all(&resolved).unwrap();
            }
        }
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!("discreveal_delete_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn secure_delete_removes_files_and_folders() {
        let dir = temp_dir("secure");
        fs::create_dir_all(dir.join("a").join("b")).unwrap();
        fs::write(dir.join("a").join("b").join("data.bin"), vec![7u8; 5000]).unwrap();
        fs::write(dir.join("empty.txt"), b"").unwrap();
        secure_delete(&dir, 0).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn secure_delete_refuses_read_only_files_without_changing_them() {
        let dir = temp_dir("readonly");
        let file = dir.join("ro.txt");
        fs::write(&file, b"protected").unwrap();
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file, permissions).unwrap();
        assert!(secure_delete_file(&file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"protected");
        assert!(fs::metadata(&file).unwrap().permissions().readonly());
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&file, permissions).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn secure_delete_refuses_hard_linked_files() {
        let dir = temp_dir("hardlink");
        let original = dir.join("original.txt");
        let link = dir.join("link.txt");
        fs::write(&original, b"shared").unwrap();
        fs::hard_link(&original, &link).unwrap();

        assert!(secure_delete_file(&original).is_err());
        assert_eq!(fs::read(&link).unwrap(), b"shared");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn permanent_removal_handles_files_and_folders() {
        let dir = temp_dir("permanent");
        fs::create_dir_all(dir.join("x")).unwrap();
        fs::write(dir.join("x").join("f.txt"), b"1").unwrap();
        remove_permanently(&dir.join("x")).unwrap();
        assert!(!dir.join("x").exists());
        fs::write(dir.join("g.txt"), b"1").unwrap();
        remove_permanently(&dir.join("g.txt")).unwrap();
        assert!(!dir.join("g.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deep_structures_are_refused() {
        let dir = temp_dir("deep");
        assert!(secure_delete(&dir, MAX_SECURE_DELETE_DEPTH + 1).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn held_duplicate_pair_preserves_original_and_denies_parallel_changes() {
        let dir = temp_dir("locked_pair");
        let copy = dir.join("copy.bin");
        let original = dir.join("original.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, false).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        assert!(fs::rename(&original, dir.join("moved.bin")).is_err());
        assert!(fs::write(&copy, b"new data").is_err());
        assert!(fs::rename(&copy, dir.join("swapped.bin")).is_err());
        dispose_locked(&target).unwrap();
        drop(target);
        assert!(!copy.exists());
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        drop(keeper);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancelled_duplicate_compare_does_not_mutate_either_file() {
        let dir = temp_dir("compare_cancel");
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        fs::write(&a, b"identical").unwrap();
        fs::write(&b, b"identical").unwrap();
        let mut target = open_locked(&a, true, false, false).unwrap();
        let mut keeper = open_locked(&b, false, false, false).unwrap();
        let error =
            compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(true)).unwrap_err();
        assert_eq!(error.code, "duplicateCleanupCancelled");
        assert_eq!(fs::read(&a).unwrap(), b"identical");
        assert_eq!(fs::read(&b).unwrap(), b"identical");
        drop(target);
        drop(keeper);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recycle_identity_recheck_rejects_replaced_duplicate_without_deleting_it() {
        let dir = ScopedTestDir::new();
        let copy = dir.path().join("copy.bin");
        let original = dir.path().join("original.bin");
        let moved = dir.path().join("moved-copy.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, true).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        let expected = file_identity(&target).unwrap();
        assert!(fs::write(&copy, b"modified").is_err());
        assert!(fs::rename(&original, dir.path().join("lost-original.bin")).is_err());
        // DELETE sharing is necessary for the Shell. A deliberate replacement
        // before dispatch must fail against the compared identity, not reset it.
        fs::rename(&copy, &moved).unwrap();
        fs::write(&copy, b"unique new file").unwrap();
        assert_eq!(
            ensure_recycle_identity(&copy, expected).unwrap_err().code,
            "duplicateChanged"
        );
        assert_eq!(fs::read(&copy).unwrap(), b"unique new file");
        assert_eq!(fs::read(&moved).unwrap(), b"identical");
        assert_eq!(fs::read(&original).unwrap(), b"identical");
    }

    // These fixtures genuinely touch the Windows bin, so normal quality gates
    // leave them opt-in. Only the uniquely named test item is restored; the
    // user's other bin contents are never purged or modified.
    #[test]
    #[ignore = "Windows integration: recycle and restore only an isolated temporary test file"]
    fn recycle_fixture_file_restores_original_name_and_contents() {
        let dir = ScopedTestDir::new();
        let copy = dir.path().join("discReveal-recycle-fixture.bin");
        let _restore = RestoreTestItem(copy.clone());
        let original = dir.path().join("retained-original.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let parents = guard::hold_mutation_parents(&copy).unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, true).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        move_locked_to_recycle_bin(&copy, &target).unwrap();
        assert!(!copy.exists());
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        drop(target);
        drop(keeper);
        drop(parents);
        let entry = fixture_trash_item(&copy)
            .expect("test file must exist in Recycle Bin at its exact original path");
        trash::os_limited::restore_all([entry]).unwrap();
        assert_eq!(fs::read(&copy).unwrap(), b"identical");
    }

    #[test]
    #[ignore = "Windows integration: recycle and restore only an isolated temporary test directory"]
    fn recycle_fixture_directory_restores_original_tree() {
        let dir = ScopedTestDir::new();
        let folder = dir.path().join("discReveal-recycle-fixture-folder");
        let _restore = RestoreTestItem(folder.clone());
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("nested").join("content.bin"), b"retain tree").unwrap();
        let parents = guard::hold_mutation_parents(&folder).unwrap();
        move_to_recycle_bin(&folder).unwrap();
        assert!(!folder.exists());
        drop(parents);
        let entry = fixture_trash_item(&folder)
            .expect("test directory must exist in Recycle Bin at its exact original path");
        trash::os_limited::restore_all([entry]).unwrap();
        assert_eq!(
            fs::read(folder.join("nested").join("content.bin")).unwrap(),
            b"retain tree"
        );
    }

    #[test]
    #[ignore = "Windows integration: recycle only test HTML, preserve its companion folder"]
    fn recycle_fixture_html_does_not_remove_connected_files() {
        let dir = ScopedTestDir::new();
        let page = dir.path().join("discReveal-recycle-fixture.htm");
        let _restore = RestoreTestItem(page.clone());
        let companion = dir.path().join("discReveal-recycle-fixture_files");
        fs::create_dir(&companion).unwrap();
        fs::write(companion.join("unique-image.bin"), b"unique companion").unwrap();
        fs::write(&page, b"<html>test</html>").unwrap();
        let parents = guard::hold_mutation_parents(&page).unwrap();
        move_to_recycle_bin(&page).unwrap();
        assert!(!page.exists());
        assert_eq!(
            fs::read(companion.join("unique-image.bin")).unwrap(),
            b"unique companion"
        );
        drop(parents);
        let entry = fixture_trash_item(&page)
            .expect("only the explicitly requested HTML test file must be recycled");
        trash::os_limited::restore_all([entry]).unwrap();
        assert_eq!(fs::read(&page).unwrap(), b"<html>test</html>");
        assert_eq!(
            fs::read(companion.join("unique-image.bin")).unwrap(),
            b"unique companion"
        );
    }

    #[test]
    #[ignore = "Windows adversarial integration: detect replacement at final Shell-open boundary, restore only our fixture"]
    fn recycle_fixture_final_replacement_is_not_reported_as_success() {
        let dir = ScopedTestDir::new();
        let copy = dir.path().join("discReveal-recycle-fixture.bin");
        let _restore = RestoreTestItem(copy.clone());
        let original = dir.path().join("retained-original.bin");
        let moved = dir.path().join("moved-compared-copy.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let parents = guard::hold_mutation_parents(&copy).unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, true).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        let expected = file_identity(&target).unwrap();
        let shell_path = guard::checked_shell_path(&copy).unwrap();
        let replacement_path = copy.clone();
        let moved_path = moved.clone();
        let result = std::thread::spawn(move || {
            recycle::recycle_checked_with_hook(
                &shell_path,
                expected,
                Box::new(move || {
                    fs::rename(&replacement_path, &moved_path)?;
                    fs::write(&replacement_path, b"unique replacement")?;
                    Ok(())
                }),
            )
        })
        .join()
        .unwrap();
        let error = result.unwrap_err();
        eprintln!("Final-boundary fixture error: {error}");
        assert_eq!(error.code, "recycleTargetChanged");
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        assert_eq!(fs::read(&moved).unwrap(), b"identical");
        assert!(
            copy.exists(),
            "the unexpected item should be restored automatically without replacing another file"
        );
        drop(target);
        drop(keeper);
        drop(parents);
        assert_eq!(fs::read(&copy).unwrap(), b"unique replacement");
    }

    #[test]
    #[ignore = "Windows adversarial integration: rollback must not replace a newly occupied destination"]
    fn recycle_fixture_rollback_refuses_occupied_destination() {
        let dir = ScopedTestDir::new();
        let copy = dir.path().join("discReveal-recycle-fixture.bin");
        let _restore = RestoreTestItem(copy.clone());
        let original = dir.path().join("retained-original.bin");
        let moved = dir.path().join("moved-compared-copy.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let parents = guard::hold_mutation_parents(&copy).unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, true).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        let expected = file_identity(&target).unwrap();
        let shell_path = guard::checked_shell_path(&copy).unwrap();
        let replacement = copy.clone();
        let moved_path = moved.clone();
        let occupied = copy.clone();
        let error = std::thread::spawn(move || {
            recycle::recycle_checked_with_rollback_hook(
                &shell_path,
                expected,
                Box::new(move || {
                    fs::rename(&replacement, &moved_path)?;
                    fs::write(&replacement, b"unique replacement")?;
                    Ok(())
                }),
                Box::new(move || {
                    fs::write(&occupied, b"do not overwrite this new file")?;
                    Ok(())
                }),
            )
        })
        .join()
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code, "recycleTargetChanged");
        assert_eq!(fs::read(&copy).unwrap(), b"do not overwrite this new file");
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        assert_eq!(fs::read(&moved).unwrap(), b"identical");
        drop(target);
        drop(keeper);
        drop(parents);
        // Only remove our exact deliberately created occupant, then restore the
        // other exact fixture to verify it was not lost or replaced either.
        fs::remove_file(&copy).unwrap();
        let entry = fixture_trash_item(&copy)
            .expect("unexpected item must remain recoverable after a refused rollback collision");
        trash::os_limited::restore_all([entry]).unwrap();
        assert_eq!(fs::read(&copy).unwrap(), b"unique replacement");
    }

    #[test]
    #[ignore = "Windows adversarial integration: late rollback collision must preserve both objects without overwrite"]
    fn recycle_fixture_late_rollback_collision_preserves_both_objects() {
        let dir = ScopedTestDir::new();
        let copy = dir.path().join("discReveal-recycle-fixture.bin");
        let _restore = RestoreTestItem(copy.clone());
        let original = dir.path().join("retained-original.bin");
        let moved = dir.path().join("moved-compared-copy.bin");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let parents = guard::hold_mutation_parents(&copy).unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        let mut target = open_locked(&copy, true, false, true).unwrap();
        compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false)).unwrap();
        let expected = file_identity(&target).unwrap();
        let shell_path = guard::checked_shell_path(&copy).unwrap();
        let replacement = copy.clone();
        let moved_path = moved.clone();
        let occupied = copy.clone();
        let error = std::thread::spawn(move || {
            recycle::recycle_checked_with_late_collision(
                &shell_path,
                expected,
                Box::new(move || {
                    fs::rename(&replacement, &moved_path)?;
                    fs::write(&replacement, b"unique replacement")?;
                    Ok(())
                }),
                Box::new(move || {
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&occupied)?;
                    file.write_all(b"do not overwrite this late file")?;
                    Ok(())
                }),
            )
        })
        .join()
        .unwrap()
        .unwrap_err();
        eprintln!("Late collision fixture error: {error}");
        assert_eq!(error.code, "recycleRecoveryMoved");
        assert_eq!(fs::read(&copy).unwrap(), b"do not overwrite this late file");
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        assert_eq!(fs::read(&moved).unwrap(), b"identical");
        let recovered: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path != &copy && path != &original && path != &moved)
            .filter(|path| fs::read(path).is_ok_and(|data| data == b"unique replacement"))
            .collect();
        assert_eq!(
            recovered.len(),
            1,
            "the unexpected item must remain intact at the generated non-overwriting recovery name"
        );
        assert!(
            error.to_string().contains(
                &guard::plain_path(recovered[0].clone())
                    .display()
                    .to_string()
            )
        );
        drop(target);
        drop(keeper);
        drop(parents);
    }

    #[test]
    fn secure_overwrite_refuses_additional_stream_without_changing_contents() {
        let dir = temp_dir("alternate_stream");
        let path = dir.join("private.txt");
        fs::write(&path, b"main content").unwrap();
        let stream = format!("{}:secret", path.display());
        fs::write(&stream, b"private extra stream").unwrap();
        assert_eq!(
            secure_delete_file(&path).unwrap_err().code,
            "secureSpecialFile"
        );
        assert_eq!(fs::read(&path).unwrap(), b"main content");
        assert_eq!(fs::read(&stream).unwrap(), b"private extra stream");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn duplicate_compare_refuses_unique_alternate_stream() {
        let dir = temp_dir("duplicate_stream");
        let copy = dir.join("copy.txt");
        let original = dir.join("original.txt");
        fs::write(&copy, b"identical").unwrap();
        fs::write(&original, b"identical").unwrap();
        let stream = format!("{}:unique", copy.display());
        fs::write(&stream, b"retain this unique data").unwrap();
        let mut target = open_locked(&copy, true, false, false).unwrap();
        let mut keeper = open_locked(&original, false, false, false).unwrap();
        assert_eq!(
            compare_locked_pair(&mut target, &mut keeper, 9, &AtomicBool::new(false))
                .unwrap_err()
                .code,
            "duplicateAdditionalStreams"
        );
        assert_eq!(fs::read(&stream).unwrap(), b"retain this unique data");
        assert_eq!(fs::read(&original).unwrap(), b"identical");
        drop(target);
        drop(keeper);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn secure_size_is_taken_from_exclusive_handle_and_writers_are_denied() {
        let dir = temp_dir("exclusive_size");
        let path = dir.join("data.bin");
        fs::write(&path, b"initial").unwrap();
        let mut file = open_locked(&path, true, true, false).unwrap();
        assert!(fs::OpenOptions::new().append(true).open(&path).is_err());
        assert!(fs::rename(&path, dir.join("moved.bin")).is_err());
        overwrite_locked(&mut file).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 7);
        dispose_locked(&file).unwrap();
        drop(file);
        assert!(!path.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
