//! Start-up checks and user-visible error reporting.
//!
//! The release build has no console and aborts on panic, so a failure would
//! end the process without a trace. This module reports a missing WebView2
//! runtime and unexpected panics with a message box instead.

use crate::settings;
use crate::win::wide;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONERROR, MB_OK, MB_YESNO, MessageBoxW,
};

/// Product code of the WebView2 runtime in the Edge updater registry.
const CLIENT_KEY_SUFFIX: &str =
    r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
const NOT_INSTALLED_VERSION: &str = "0.0.0.0";

/// HRESULT reported when the WebView2 profile is locked by another process.
const PROFILE_LOCKED_HRESULT: &str = "0x8007139F";

/// Texts of the native message boxes, which appear before the frontend (and
/// its translations) exist.
struct Messages {
    webview_missing: &'static str,
    unexpected_exit: &'static str,
    profile_locked_hint: &'static str,
}

const MESSAGES_DE: Messages = Messages {
    webview_missing: "DiscReveal benötigt die Microsoft Edge WebView2-Runtime, die auf diesem System nicht installiert ist.\n\nDie Download-Seite jetzt öffnen?",
    unexpected_exit: "DiscReveal wurde unerwartet beendet.",
    profile_locked_hint: "Ein anderer DiscReveal-Prozess oder ein hängengebliebener WebView2-Prozess (msedgewebview2.exe) blockiert den Start. Beende ihn im Task-Manager oder starte den Rechner neu.",
};

const MESSAGES_EN: Messages = Messages {
    webview_missing: "DiscReveal requires the Microsoft Edge WebView2 runtime, which is not installed on this system.\n\nOpen the download page now?",
    unexpected_exit: "DiscReveal exited unexpectedly.",
    profile_locked_hint: "Another DiscReveal process or a stuck WebView2 process (msedgewebview2.exe) is blocking the start. End it in Task Manager or restart the computer.",
};

fn messages() -> &'static Messages {
    if settings::preferred_language() == "de" {
        &MESSAGES_DE
    } else {
        &MESSAGES_EN
    }
}

/// Version string of the runtime registered under `hive`, if any.
fn registered_version(hive: HKEY, software_path: &str) -> Option<String> {
    let key = wide(&format!(r"{software_path}\{CLIENT_KEY_SUFFIX}"));
    let value = wide("pv");
    let mut buffer = [0u16; 64];
    let mut size = size_of_val(&buffer) as u32;
    // SAFETY: both strings are null-terminated; `buffer` and `size` describe a valid output buffer.
    let status = unsafe {
        RegGetValueW(
            hive,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let version = String::from_utf16_lossy(&buffer[..length]);
    (!version.is_empty() && version != NOT_INSTALLED_VERSION).then_some(version)
}

fn is_installed() -> bool {
    registered_version(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node").is_some()
        || registered_version(HKEY_LOCAL_MACHINE, "SOFTWARE").is_some()
        || registered_version(HKEY_CURRENT_USER, "Software").is_some()
}

/// Windows Explorer by absolute path, so a planted `explorer.exe` next to
/// the portable executable is never started.
fn explorer_path() -> PathBuf {
    let system_root = env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(system_root).join("explorer.exe")
}

/// Shows a modal error message box.
fn show_error(text: &str) {
    let title = wide("DiscReveal");
    let text = wide(text);
    // SAFETY: both strings are null-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        )
    };
}

/// Returns `true` if the runtime is present. Otherwise shows a message box
/// offering the download page and returns `false`.
pub fn ensure_webview2_available() -> bool {
    if is_installed() {
        return true;
    }

    let title = wide("DiscReveal");
    let text = wide(messages().webview_missing);
    // SAFETY: both strings are null-terminated and outlive the call.
    let answer = unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONERROR,
        )
    };
    if answer == IDYES {
        // Failure to launch the browser is not actionable here.
        let _ = Command::new(explorer_path()).arg(DOWNLOAD_URL).spawn();
    }
    false
}

/// Turns an unexpected failure into a readable message.
fn describe_failure(details: &str, messages: &Messages) -> String {
    let hint = if details.contains(PROFILE_LOCKED_HRESULT) {
        format!("\n\n{}", messages.profile_locked_hint)
    } else {
        String::new()
    };
    format!("{}\n\n{details}{hint}", messages.unexpected_exit)
}

/// Shows unexpected panics in a message box before the process aborts.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        show_error(&describe_failure(&info.to_string(), messages()))
    }));
}

/// Reports an error returned by the application runtime.
pub fn report_startup_failure(error: &dyn std::fmt::Display) {
    show_error(&describe_failure(&error.to_string(), messages()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_profile_gets_a_specific_hint() {
        assert!(
            describe_failure("WebView2 error 0x8007139F", &MESSAGES_EN)
                .contains("msedgewebview2.exe")
        );
        assert!(
            describe_failure("WebView2 error 0x8007139F", &MESSAGES_DE)
                .contains("msedgewebview2.exe")
        );
        assert!(!describe_failure("something else", &MESSAGES_EN).contains("msedgewebview2.exe"));
    }

    #[test]
    fn failure_message_starts_with_the_localised_headline() {
        assert!(
            describe_failure("x", &MESSAGES_DE).starts_with("DiscReveal wurde unerwartet beendet.")
        );
        assert!(describe_failure("x", &MESSAGES_EN).starts_with("DiscReveal exited unexpectedly."));
    }
    #[test]
    fn abort_profile_panic_is_isolated_and_cannot_run_operation_guards() {
        use std::os::windows::process::CommandExt;
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        let fixture = temporary.join(format!(
            "discreveal-panic-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&fixture).unwrap();
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                if self
                    .0
                    .parent()
                    .is_some_and(|parent| parent == std::env::temp_dir().canonicalize().unwrap())
                {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
        }
        let _fixture = Fixture(fixture.clone());
        let source = fixture.join("probe.rs");
        let binary = fixture.join("probe.exe");
        let marker = fixture.join("guard-ran");
        std::fs::write(&source,r#"
            #[link(name="kernel32")]
            unsafe extern "system" { fn SetErrorMode(mode:u32)->u32; }
            struct Guard(String);
            impl Drop for Guard { fn drop(&mut self) {std::fs::write(&self.0,"guard ran").unwrap();} }
            fn main() {
                unsafe {SetErrorMode(3);}
                let _guard=Guard(std::env::args().nth(1).unwrap());
                panic!("injected abort-profile panic");
            }
        "#).unwrap();
        let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2024", "-C", "panic=abort"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let child = Command::new(&binary)
            .arg(&marker)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(!child.status.success());
        assert!(String::from_utf8_lossy(&child.stderr).contains("injected abort-profile panic"));
        assert!(
            !marker.exists(),
            "abort cannot be represented as a recovered operation"
        );
    }
}
