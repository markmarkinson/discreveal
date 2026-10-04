//! Operating-system integration: opening entries and choosing folders.

use crate::error::{AppError, AppResult};
use crate::guard;
use crate::scan::ScanState;
use sha2::{Digest, Sha256};
use std::path::Path;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// File types that are opened with their default application. Everything
/// else, including all executable, script and shortcut types, is only
/// revealed in Explorer.
const OPENABLE_EXTENSIONS: &[&str] = &[
    // Documents and text
    "txt", "log", "md", "pdf", "rtf", "doc", "docx", "odt", "xls", "xlsx", "ods", "ppt", "pptx",
    "odp", "csv", "json", "xml", "ini", "cfg", "yaml", "yml", "epub", // Images
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "ico", "heic", "raw",
    // Audio and video
    "mp3", "wav", "flac", "ogg", "m4a", "aac", "wma", "mp4", "mkv", "avi", "mov", "wmv", "webm",
    "flv", "m4v", "mpg", "mpeg",
    // Archives (opened by the archive viewer, never executed)
    "zip", "rar", "7z", "tar", "gz", // Fonts
    "ttf", "otf",
];

fn is_openable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            OPENABLE_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// Opens an entry of the current scan with its default application.
/// Files of types not on the allow-list are only revealed in Explorer.
#[tauri::command(async)]
pub fn open_path(
    app: AppHandle,
    state: State<ScanState>,
    result_roots: State<guard::ResultRoots>,
    scope: Option<guard::ResultScope>,
    target_path: String,
) -> AppResult<()> {
    let target = guard::plain_path(guard::resolve_for_result(
        &state,
        &result_roots,
        scope,
        &target_path,
        false,
    )?);
    let opener = app.opener();
    let result = if target.is_file() && !is_openable(&target) {
        opener.reveal_item_in_dir(&target)
    } else {
        opener.open_path(target.to_string_lossy(), None::<&str>)
    };
    result.map_err(|error| AppError::with_detail("openFailed", error))
}

/// Opens the update-check page for the running version in the user's default
/// browser. The URL is built entirely here, not from a frontend-supplied
/// value — this stays the app's only outbound network-adjacent action (it
/// never makes a request itself, it just opens a page) and there is no
/// generic "open any URL" command for anything else to misuse. The page
/// itself (a 404 fallback on the existing GitHub Pages site, since this
/// version segment never matches a real file) compares the version in the
/// URL against the latest release and tells the visitor directly.
///
/// The exe's own SHA-256 is appended as a second path segment when it can be
/// computed, so the page can also confirm this is really the official build
/// for that version — not just an outdated one, but a corrupted, tampered,
/// or impostor one. Best-effort: a failure to read/hash the running exe
/// (should never happen, but nothing about reading your own binary is
/// guaranteed) just falls back to the plain version-only URL that already
/// worked before this existed.
#[tauri::command(async)]
pub fn check_for_updates(app: AppHandle, language: String) -> AppResult<()> {
    let version = app.package_info().version.to_string();
    let url = update_check_url(&version, exe_sha256().as_deref(), &language);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| AppError::with_detail("openFailed", error))
}

/// Builds the update-check URL, with the exe's hash as an optional second
/// path segment. A pure function so the URL shape is tested directly,
/// without needing a real `AppHandle` or a real exe on disk.
fn update_check_url(version: &str, hash: Option<&str>, language: &str) -> String {
    let prefix = if language == "de" {
        "https://discreveal.com/de"
    } else {
        "https://discreveal.com"
    };
    match hash {
        Some(hash) => format!("{prefix}/v-{version}/{hash}"),
        None => format!("{prefix}/v-{version}"),
    }
}

/// Lowercase hex SHA-256 of the currently running executable, or `None` if
/// its path or contents can't be read.
fn exe_sha256() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    let bytes = std::fs::read(path).ok()?;
    Some(sha256_hex(&bytes))
}

/// Lowercase hex SHA-256 of `bytes` — the format GitHub's own Releases API
/// uses for an asset's `digest` field, which `docs/404.html` compares this
/// against.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Longest accepted dialog title; the title comes from the frontend.
const MAX_DIALOG_TITLE_LEN: usize = 100;

/// Shows the native folder picker with the given title and returns the chosen
/// path, or `None` if the dialog was cancelled.
#[tauri::command(async)]
pub fn pick_folder(app: AppHandle, title: String) -> Option<String> {
    let title: String = title
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_DIALOG_TITLE_LEN)
        .collect();
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog
        .blocking_pick_folder()
        .and_then(|folder| folder.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_and_media_are_opened() {
        assert!(is_openable(Path::new(r"C:\a\report.PDF")));
        assert!(is_openable(Path::new(r"C:\a\movie.mkv")));
    }

    #[test]
    fn executable_and_script_types_are_only_revealed() {
        for name in [
            "setup.exe",
            "x.bat",
            "x.ps1",
            "link.lnk",
            "a.msc",
            "b.theme",
            "c.library-ms",
            "d.docm",
            "e.iso",
            "noext",
        ] {
            assert!(!is_openable(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn sha256_hex_matches_a_known_test_vector() {
        // The standard "abc" SHA-256 vector (NIST FIPS 180-4) — proves the hashing and hex-encoding are
        // both correct, independent of hitting a real file on disk.
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn update_check_url_appends_the_hash_only_when_present() {
        assert_eq!(
            update_check_url("0.2.0", Some("abc123"), "de"),
            "https://discreveal.com/de/v-0.2.0/abc123"
        );
        assert_eq!(
            update_check_url("0.2.0", None, "de"),
            "https://discreveal.com/de/v-0.2.0"
        );
        assert_eq!(
            update_check_url("0.2.0", Some("abc123"), "en"),
            "https://discreveal.com/v-0.2.0/abc123"
        );
        assert_eq!(
            update_check_url("0.2.0", None, "invalid"),
            "https://discreveal.com/v-0.2.0"
        );
    }
}
