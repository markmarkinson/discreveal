//! DiscReveal backend: filesystem scanning, deletion and OS integration.
//!
//! The webview frontend talks to this crate exclusively through the
//! `#[tauri::command]` functions registered below. Every command that
//! touches a user-supplied path validates it against that result's registered
//! roots (see [`guard`]) before acting on it.

mod cache;
mod cleanup;
mod delete;
mod diagnostics;
mod drives;
mod duplicates;
mod error;
mod guard;
mod icons;
mod operation_ipc;
mod operations;
mod runtime;
mod scan;
mod scan_directory;
mod scan_policy;
mod settings;
mod shell;
mod storage;
mod win;

// Tauri links its Common Controls 6 manifest into app binaries only. Tests
// also use native dialog code, so link that same resource only for the harness.
#[cfg(test)]
#[link(name = "resource", kind = "static", modifiers = "-bundle")]
unsafe extern "C" {}

use std::env;
use std::path::PathBuf;

/// Identifies build artifacts as belonging to this project, even one that
/// has drifted away from the executable — for example a stray settings file
/// found without DiscReveal installed nearby. Uses the exact provenance names
/// discReveal and markMarkinson. Compiled into the binary as a plain string, so
/// it also survives a rebuild that strips comments and documentation.
pub(crate) const DISC_REVEAL_MARK_MARKINSON_SIGNATURE: &str = "discReveal by markMarkinson";

/// The folder the running portable executable lives in.
pub(crate) fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default()
}

/// Portable settings are grouped here; the opt-in cache uses LocalAppData.
pub(crate) fn data_dir() -> PathBuf {
    exe_dir().join("discreveal_data")
}

/// Builds and runs the Tauri application.
pub fn run() {
    // Visible only in a console-attached dev run; release builds have no
    // console (`windows_subsystem = "windows"`), so this never reaches a user.
    if cfg!(debug_assertions) {
        eprintln!("{DISC_REVEAL_MARK_MARKINSON_SIGNATURE}");
    }

    runtime::install_panic_hook();
    cleanup::system::wait_restart_parent();
    if !runtime::ensure_webview2_available() {
        return;
    }

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(scan::ScanState::new())
        .manage(guard::ResultRoots::default())
        .manage(duplicates::DuplicatesState::new())
        .manage(cleanup::CleanupState::default())
        .invoke_handler(tauri::generate_handler![
            settings::load_settings,
            settings::save_settings,
            drives::list_drives,
            icons::get_file_icons,
            scan::start_scan,
            scan::cancel_scan,
            operation_ipc::get_operation_statuses,
            scan::expand_folder,
            scan::cancel_expand_folder,
            duplicates::find_duplicates,
            duplicates::cancel_duplicates,
            duplicates::cancel_duplicate_cleanup,
            cache::clear_hash_cache,
            cache::hash_cache_stats,
            cleanup::cleanup_rules,
            cleanup::analyze_cleanup,
            cleanup::execute_cleanup,
            cleanup::cancel_cleanup,
            cleanup::cleanup_items,
            cleanup::open_cleanup_settings,
            cleanup::run_windows_cleanup,
            cleanup::system::analyze_windows_cleanup,
            cleanup::system::execute_windows_cleanup,
            cleanup::system::restart_cleanup_as_admin,
            cleanup::pick_cleanup_project,
            cleanup::cleanup_recycle_bins,
            cleanup::empty_cleanup_recycle_bin,
            delete::delete_item,
            delete::delete_duplicate,
            delete::get_protection,
            shell::open_path,
            shell::pick_folder,
            shell::check_for_updates,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        runtime::report_startup_failure(&error);
    }
}
