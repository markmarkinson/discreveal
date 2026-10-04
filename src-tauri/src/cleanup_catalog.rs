//! discReveal / markMarkinson: compiled allow-list; never imports third-party deletion rules.
use super::{Rule, blocked_processes, browser_roots, ordinary_directory};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(super) fn windows_directory(system: bool) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::{
        GetSystemDirectoryW, GetWindowsDirectoryW,
    };
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        if system {
            GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32)
        } else {
            GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32)
        }
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length],
    )))
}
fn subdirs(base: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|name| base.join(name)).collect()
}
fn opera_roots(base: &Path) -> Vec<PathBuf> {
    let mut roots = subdirs(base, &["Cache", "Code Cache"]);
    roots.extend(browser_roots(base, false));
    roots
}
fn adobe_cache(base: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return vec![];
    };
    entries
        .flatten()
        .take(100)
        .filter(|e| ordinary_directory(&e.path()))
        .map(|e| e.path().join("Cache"))
        .collect()
}
pub(super) fn discover(
    local: &Path,
    roaming: &Path,
    program: &Path,
    running: Option<&HashSet<String>>,
) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut add = |id: &str,
                   label: &str,
                   category: &str,
                   effect: &str,
                   roots: Vec<PathBuf>,
                   names: &[&str],
                   recommended: bool,
                   days: u32,
                   log_only: bool| {
        let mut roots: Vec<_> = roots
            .into_iter()
            .filter(|p| ordinary_directory(p))
            .collect();
        roots.sort();
        roots.dedup();
        let processes: Vec<_> = names.iter().map(|name| name.to_lowercase()).collect();
        rules.push(Rule {
            id: id.into(),
            label: label.into(),
            category: category.into(),
            effect: effect.into(),
            recommended,
            available: !roots.is_empty(),
            blocked: blocked_processes(&processes, running),
            min_days: days,
            processes,
            log_only,
            roots,
        });
    };
    add(
        "temp",
        "cleanupTemp",
        "windows",
        "cleanupTempEffect",
        vec![local.join("Temp")],
        &[],
        false,
        7,
        false,
    );
    if let Ok(windows) = windows_directory(false) {
        add(
            "system-temp",
            "cleanupSystemTemp",
            "windows",
            "cleanupSystemTempEffect",
            vec![windows.join("Temp")],
            &[],
            false,
            7,
            false,
        );
    }
    for (id, label, bases, process) in [
        (
            "chrome",
            "cleanupChrome",
            vec![
                "Google/Chrome/User Data",
                "Google/Chrome Beta/User Data",
                "Google/Chrome Dev/User Data",
                "Google/Chrome SxS/User Data",
            ],
            "chrome.exe",
        ),
        (
            "edge",
            "cleanupEdge",
            vec![
                "Microsoft/Edge/User Data",
                "Microsoft/Edge Beta/User Data",
                "Microsoft/Edge Dev/User Data",
                "Microsoft/Edge SxS/User Data",
            ],
            "msedge.exe",
        ),
        (
            "brave",
            "cleanupBrave",
            vec![
                "BraveSoftware/Brave-Browser/User Data",
                "BraveSoftware/Brave-Browser-Beta/User Data",
                "BraveSoftware/Brave-Browser-Nightly/User Data",
            ],
            "brave.exe",
        ),
        (
            "vivaldi",
            "cleanupVivaldi",
            vec!["Vivaldi/User Data"],
            "vivaldi.exe",
        ),
    ] {
        let roots = bases
            .iter()
            .flat_map(|base| browser_roots(&local.join(base), false))
            .collect();
        add(
            id,
            label,
            "browsers",
            "cleanupBrowserEffect",
            roots,
            &[process],
            true,
            1,
            false,
        );
    }
    add(
        "firefox",
        "cleanupFirefox",
        "browsers",
        "cleanupBrowserEffect",
        browser_roots(&local.join("Mozilla/Firefox/Profiles"), true),
        &["firefox.exe"],
        true,
        1,
        false,
    );
    for (id, label, folder) in [
        ("opera", "cleanupOpera", "Opera Stable"),
        ("opera-gx", "cleanupOperaGX", "Opera GX Stable"),
    ] {
        let mut roots = opera_roots(&local.join("Opera Software").join(folder));
        roots.extend(opera_roots(&roaming.join("Opera Software").join(folder)));
        add(
            id,
            label,
            "browsers",
            "cleanupBrowserEffect",
            roots,
            &["opera.exe", "opera_crashreporter.exe"],
            true,
            1,
            false,
        );
    }
    for (id, label, folder, process) in [
        ("discord", "cleanupDiscord", "discord", "discord.exe"),
        (
            "discord-ptb",
            "cleanupDiscordPTB",
            "discordptb",
            "discordptb.exe",
        ),
        (
            "discord-canary",
            "cleanupDiscordCanary",
            "discordcanary",
            "discordcanary.exe",
        ),
        ("vscode", "cleanupVSCode", "Code", "code.exe"),
        (
            "vscode-insiders",
            "cleanupVSCodeInsiders",
            "Code - Insiders",
            "code - insiders.exe",
        ),
    ] {
        let names: &[&str] = if id.starts_with("vscode") {
            &["Cache", "Code Cache", "GPUCache", "CachedData"]
        } else {
            &["Cache", "Code Cache", "GPUCache"]
        };
        add(
            id,
            label,
            "apps",
            "cleanupAppEffect",
            subdirs(&roaming.join(folder), names),
            &[process],
            true,
            1,
            false,
        );
        if id.starts_with("vscode") {
            add(
                &format!("{id}-logs"),
                if id == "vscode" {
                    "cleanupVSCodeLogs"
                } else {
                    "cleanupVSCodeInsidersLogs"
                },
                "diagnostics",
                "cleanupReportsEffect",
                vec![roaming.join(folder).join("logs")],
                &[process],
                false,
                30,
                true,
            );
        }
    }
    let teams = local.join("Packages/MSTeams_8wekyb3d8bbwe/LocalCache/Microsoft/MSTeams/EBWebView");
    let mut teams_roots = browser_roots(&teams, false);
    teams_roots.extend(subdirs(
        &roaming.join("Microsoft/Teams"),
        &["Cache", "Code Cache", "GPUCache"],
    ));
    add(
        "teams",
        "cleanupTeams",
        "apps",
        "cleanupAppEffect",
        teams_roots,
        &["ms-teams.exe", "teams.exe"],
        true,
        1,
        false,
    );
    add(
        "adobe",
        "cleanupAdobe",
        "apps",
        "cleanupAppEffect",
        adobe_cache(&local.join("Adobe/Acrobat")),
        &["acrobat.exe", "acrord32.exe"],
        true,
        7,
        false,
    );
    add(
        "reports",
        "cleanupReports",
        "diagnostics",
        "cleanupReportsEffect",
        vec![
            local.join("Microsoft/Windows/WER/ReportArchive"),
            local.join("Microsoft/Windows/WER/ReportQueue"),
            local.join("CrashDumps"),
        ],
        &[],
        false,
        30,
        false,
    );
    add(
        "system-reports",
        "cleanupSystemReports",
        "diagnostics",
        "cleanupReportsEffect",
        subdirs(
            &program.join("Microsoft/Windows/WER"),
            &["ReportArchive", "ReportQueue"],
        ),
        &[],
        false,
        30,
        false,
    );
    add(
        "pip",
        "cleanupPip",
        "development",
        "cleanupDeveloperEffect",
        subdirs(&local.join("pip/Cache"), &["http", "http-v2", "wheels"]),
        &["python.exe", "pythonw.exe", "pip.exe", "pip3.exe"],
        false,
        30,
        false,
    );
    add(
        "npm",
        "cleanupNpm",
        "development",
        "cleanupDeveloperEffect",
        vec![
            local.join("npm-cache/_cacache"),
            roaming.join("npm-cache/_cacache"),
        ],
        &["node.exe", "npm.exe"],
        false,
        30,
        false,
    );
    add(
        "nuget",
        "cleanupNuGet",
        "development",
        "cleanupDeveloperEffect",
        vec![local.join("NuGet/v3-cache")],
        &["dotnet.exe", "msbuild.exe", "nuget.exe", "devenv.exe"],
        false,
        30,
        false,
    );
    add(
        "directx",
        "cleanupDirectX",
        "shaders",
        "cleanupShaderEffect",
        vec![local.join("D3DSCache")],
        &[],
        false,
        30,
        false,
    );
    add(
        "nvidia",
        "cleanupNvidia",
        "shaders",
        "cleanupShaderEffect",
        vec![
            local.join("NVIDIA/DXCache"),
            local.join("NVIDIA/GLCache"),
            local.join("NVIDIA Corporation/NV_Cache"),
        ],
        &[],
        false,
        30,
        false,
    );
    add(
        "amd",
        "cleanupAMD",
        "shaders",
        "cleanupShaderEffect",
        subdirs(
            &local.join("AMD"),
            &["DxCache", "DxcCache", "GLCache", "VkCache"],
        ),
        &[],
        false,
        30,
        false,
    );
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_covers_variants_without_profile_or_recovery_roots() {
        let dir =
            std::env::temp_dir().join(format!("discreveal-catalog-{}", rand::random::<u64>()));
        let local = dir.join("Local");
        let roaming = dir.join("Roaming");
        let program = dir.join("Program");
        for name in [
            "Code/Cache",
            "Code/Backups",
            "Code/User",
            "discord/Cache",
            "discord/Local Storage",
            "Opera Software/Opera GX Stable/Default/Cache",
        ] {
            std::fs::create_dir_all(roaming.join(name)).unwrap();
        }
        let browser = local.join("BraveSoftware/Brave-Browser/User Data");
        for name in [
            "Default/Cache",
            "Default/GPUCache",
            "Default/DawnCache",
            "component_crx_cache",
            "extensions_crx_cache",
            "Default/Extensions",
            "Default/IndexedDB",
            "Default/Service Worker",
            "Default/Local Storage",
            "InstalledComponent",
        ] {
            std::fs::create_dir_all(browser.join(name)).unwrap();
        }
        let rules = discover(
            &local,
            &roaming,
            &program,
            Some(&HashSet::from(["discord.exe".into()])),
        );
        assert!(rules.len() > 20);
        assert!(rules.iter().find(|r| r.id == "discord").unwrap().blocked);
        let brave = rules.iter().find(|r| r.id == "brave").unwrap();
        assert!(brave.available);
        for name in [
            "Default/GPUCache",
            "Default/DawnCache",
            "component_crx_cache",
            "extensions_crx_cache",
        ] {
            assert!(brave.roots.contains(&browser.join(name)));
        }
        for name in [
            "Default/Extensions",
            "Default/IndexedDB",
            "Default/Service Worker",
            "Default/Local Storage",
            "InstalledComponent",
        ] {
            assert!(!brave.roots.contains(&browser.join(name)));
        }
        assert!(rules.iter().find(|r| r.id == "opera-gx").unwrap().available);
        assert!(
            rules
                .iter()
                .flat_map(|r| &r.roots)
                .all(|p| !p.ends_with("Backups")
                    && !p.ends_with("User")
                    && !p.ends_with("Local Storage"))
        );
        assert!(
            rules
                .iter()
                .filter(|r| r.category == "shaders" || r.category == "development")
                .all(|r| !r.recommended && r.min_days >= 30)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
