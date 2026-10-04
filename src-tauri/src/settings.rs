//! Persistent user settings.
//!
//! Stored in `discreveal_data` next to the executable, so settings travel
//! with the portable app. Legacy settings are migrated without losing preferences.

use crate::error::{AppError, AppResult};
use crate::exe_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

const SETTINGS_FILE_NAME: &str = "discreveal_settings.ini";
const EXCLUDE_SEPARATOR: char = '|';
const MAX_EXCLUDES: usize = 500;
const MAX_VALUE_LEN: usize = 1024;
const MAX_LIMIT: f64 = 1.0e9;

/// Primary language identifier of German within a Windows language id.
const PRIMARY_LANGUAGE_GERMAN: u16 = 0x07;
const PRIMARY_LANGUAGE_MASK: u16 = 0x3ff;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    last_drive: String,
    #[serde(default = "default_excludes")]
    excludes: Vec<String>,
    #[serde(default, rename = "minFolderSizeMB")]
    min_folder_size_mb: f64,
    #[serde(default, rename = "maxFolderSizeGB")]
    max_folder_size_gb: f64,
    /// Unit the min-size field is displayed and typed in, `MB` or `GB`.
    /// Storage stays in `min_folder_size_mb` regardless of this choice.
    #[serde(default = "default_min_size_unit", rename = "minFolderSizeUnit")]
    min_folder_size_unit: String,
    /// Unit the max-size field is displayed and typed in, `MB` or `GB`.
    #[serde(default = "default_max_size_unit", rename = "maxFolderSizeUnit")]
    max_folder_size_unit: String,
    #[serde(default = "default_view_mode")]
    view_mode: String,
    /// User interface language, `de` or `en`.
    #[serde(default = "detect_language")]
    language: String,
    /// Opt-in: whether the duplicate finder stores and reuses file hashes in
    /// `cache.rs`'s SQLite file across scans. Off by default — see
    /// `cache.rs`'s module doc for why this stays opt-in.
    #[serde(default)]
    cache_hashes_enabled: bool,
    #[serde(default)]
    measure_allocated: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            last_drive: String::new(),
            excludes: default_excludes(),
            min_folder_size_mb: 0.0,
            max_folder_size_gb: 0.0,
            min_folder_size_unit: default_min_size_unit(),
            max_folder_size_unit: default_max_size_unit(),
            view_mode: default_view_mode(),
            language: detect_language(),
            cache_hashes_enabled: false,
            measure_allocated: false,
        }
    }
}

impl Settings {
    /// Normalises untrusted values so they can be serialised to the INI file
    /// without breaking its line-based format.
    fn sanitized(self) -> Self {
        Self {
            last_drive: clean_value(&self.last_drive),
            excludes: self
                .excludes
                .iter()
                .map(|exclude| clean_value(exclude).replace(EXCLUDE_SEPARATOR, ""))
                .filter(|exclude| !exclude.is_empty())
                .take(MAX_EXCLUDES)
                .collect(),
            min_folder_size_mb: clamp_limit(self.min_folder_size_mb),
            max_folder_size_gb: clamp_limit(self.max_folder_size_gb),
            min_folder_size_unit: clean_size_unit(
                &self.min_folder_size_unit,
                default_min_size_unit,
            ),
            max_folder_size_unit: clean_size_unit(
                &self.max_folder_size_unit,
                default_max_size_unit,
            ),
            view_mode: if self.view_mode == "grid" {
                "grid".into()
            } else {
                default_view_mode()
            },
            language: match self.language.as_str() {
                language @ ("de" | "en") => language.to_string(),
                _ => detect_language(),
            },
            cache_hashes_enabled: self.cache_hashes_enabled,
            measure_allocated: self.measure_allocated,
        }
    }
}

fn default_view_mode() -> String {
    "list".to_string()
}

fn default_min_size_unit() -> String {
    "MB".to_string()
}

fn default_max_size_unit() -> String {
    "GB".to_string()
}

/// Keeps only `MB`/`GB`, falling back to the field's own default otherwise.
fn clean_size_unit(value: &str, default: fn() -> String) -> String {
    match value {
        unit @ ("MB" | "GB") => unit.to_string(),
        _ => default(),
    }
}

/// The language of the Windows user interface, reduced to the two supported
/// languages: German for German systems, English otherwise.
fn detect_language() -> String {
    // SAFETY: no arguments.
    let language_id = unsafe { GetUserDefaultUILanguage() };
    let is_german = language_id & PRIMARY_LANGUAGE_MASK == PRIMARY_LANGUAGE_GERMAN;
    if is_german { "de" } else { "en" }.to_string()
}

/// The language to use before the frontend is running, for native message
/// boxes: the saved setting, or the system language if none is saved.
pub fn preferred_language() -> String {
    load_settings().language
}

/// Game library folders excluded by default, matched case-insensitively as
/// substrings of the full path. Leading backslashes keep short names such as
/// `ea games` from matching unrelated folders.
fn default_excludes() -> Vec<String> {
    [
        r"\steamapps",
        r"\epic games",
        r"\gog games",
        r"\gog galaxy\games",
        r"\riot games",
        r"\ea games",
        r"\origin games",
        r"\ubisoft game launcher\games",
        r"\xboxgames",
    ]
    .map(String::from)
    .to_vec()
}

/// Drops control characters (including line breaks) and caps the length.
fn clean_value(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_VALUE_LEN)
        .collect()
}

fn clamp_limit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, MAX_LIMIT)
    } else {
        0.0
    }
}

fn settings_file_path() -> PathBuf {
    crate::data_dir().join(SETTINGS_FILE_NAME)
}

fn load_settings_from(directory: &std::path::Path) -> Settings {
    let legacy = directory.join(SETTINGS_FILE_NAME);
    let path = directory.join("discreveal_data").join(SETTINGS_FILE_NAME);
    let source = if path.exists() { &path } else { &legacy };
    let Ok(bytes) = fs::read(source) else {
        return Settings::default();
    };
    if source == &legacy {
        // Rename only after reading successfully. On a read-only drive keep using
        // the legacy preferences; a later save will report its normal error.
        if fs::create_dir_all(path.parent().unwrap()).is_ok() {
            let _ = fs::rename(&legacy, &path);
        }
    }
    parse_settings(&String::from_utf8_lossy(&bytes))
}

/// Reads the `[general]` section into a key/value map. Other sections,
/// comments and malformed lines are ignored.
fn parse_general_section(contents: &str) -> HashMap<&str, &str> {
    let mut values = HashMap::new();
    let mut in_general = false;
    for line in contents.lines().map(str::trim) {
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_general = section == "general";
        } else if in_general && let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim(), value.trim());
        }
    }
    values
}

fn parse_settings(contents: &str) -> Settings {
    // Editors may prepend a byte order mark, which would hide the section header.
    let values = parse_general_section(contents.trim_start_matches('\u{feff}'));
    let excludes = values.get("excludes").map(|list| {
        list.split(EXCLUDE_SEPARATOR)
            .filter(|item| !item.is_empty())
            .map(String::from)
            .collect()
    });
    let number = |key: &str| values.get(key).and_then(|v| v.parse().ok()).unwrap_or(0.0);

    Settings {
        last_drive: values
            .get("lastDrive")
            .copied()
            .unwrap_or_default()
            .to_string(),
        excludes: excludes.unwrap_or_else(default_excludes),
        min_folder_size_mb: number("minFolderSizeMB"),
        max_folder_size_gb: number("maxFolderSizeGB"),
        min_folder_size_unit: values
            .get("minFolderSizeUnit")
            .copied()
            .unwrap_or_default()
            .to_string(),
        max_folder_size_unit: values
            .get("maxFolderSizeUnit")
            .copied()
            .unwrap_or_default()
            .to_string(),
        view_mode: values
            .get("viewMode")
            .copied()
            .unwrap_or_default()
            .to_string(),
        language: values
            .get("language")
            .copied()
            .unwrap_or_default()
            .to_string(),
        cache_hashes_enabled: values.get("cacheHashesEnabled").copied() == Some("1"),
        measure_allocated: values.get("measureAllocated").copied() == Some("1"),
    }
    .sanitized()
}

fn serialize_settings(settings: &Settings) -> String {
    format!(
        "; {}\n[general]\nlastDrive={}\nexcludes={}\nminFolderSizeMB={}\nmaxFolderSizeGB={}\nminFolderSizeUnit={}\nmaxFolderSizeUnit={}\nviewMode={}\nlanguage={}\ncacheHashesEnabled={}\nmeasureAllocated={}\n",
        crate::DISC_REVEAL_MARK_MARKINSON_SIGNATURE,
        settings.last_drive,
        settings.excludes.join(&EXCLUDE_SEPARATOR.to_string()),
        settings.min_folder_size_mb,
        settings.max_folder_size_gb,
        settings.min_folder_size_unit,
        settings.max_folder_size_unit,
        settings.view_mode,
        settings.language,
        if settings.cache_hashes_enabled { 1 } else { 0 },
        if settings.measure_allocated { 1 } else { 0 },
    )
}

/// Loads the settings, falling back to defaults if the file is missing.
#[tauri::command]
pub fn load_settings() -> Settings {
    load_settings_from(&exe_dir())
}

/// Validates and persists the settings. The file is written to a temporary
/// name first and renamed into place, so an interrupted write cannot leave a
/// truncated settings file behind.
#[tauri::command]
pub fn save_settings(settings: Settings) -> AppResult<()> {
    let path = settings_file_path();
    let temporary = path.with_extension("ini.tmp");
    let contents = serialize_settings(&settings.sanitized());

    let write = || -> std::io::Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = fs::File::create(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)
    };
    write().map_err(|error| {
        let _ = fs::remove_file(&temporary);
        AppError::with_detail(
            "settingsWriteFailed",
            format!("{} ({error})", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_move_into_data_folder_and_new_settings_win() {
        let directory =
            std::env::temp_dir().join(format!("discreveal-settings-{}", rand::random::<u64>()));
        fs::create_dir_all(&directory).unwrap();
        let legacy = directory.join(SETTINGS_FILE_NAME);
        fs::write(&legacy, "[general]\nlanguage=en\ncacheHashesEnabled=1\n").unwrap();
        let migrated = load_settings_from(&directory);
        assert_eq!(migrated.language, "en");
        assert!(migrated.cache_hashes_enabled);
        assert!(!legacy.exists());
        assert!(
            directory
                .join("discreveal_data")
                .join(SETTINGS_FILE_NAME)
                .exists()
        );
        fs::write(&legacy, "[general]\nlanguage=de\n").unwrap();
        assert_eq!(load_settings_from(&directory).language, "en");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn parses_sections_and_ignores_others() {
        let settings =
            parse_settings("[other]\nlastDrive=X:\\\n[general]\nlastDrive=C:\\\nviewMode=grid\n");
        assert_eq!(settings.last_drive, "C:\\");
        assert_eq!(settings.view_mode, "grid");
    }

    #[test]
    fn byte_order_mark_and_crlf_are_tolerated() {
        let settings =
            parse_settings("\u{feff}[general]\r\nlastDrive=D:\\\r\nminFolderSizeMB=5\r\n");
        assert_eq!(settings.last_drive, "D:\\");
        assert_eq!(settings.min_folder_size_mb, 5.0);
    }

    #[test]
    fn missing_excludes_fall_back_to_defaults_but_empty_ones_are_kept() {
        assert_eq!(parse_settings("[general]\n").excludes, default_excludes());
        assert!(parse_settings("[general]\nexcludes=\n").excludes.is_empty());
    }

    #[test]
    fn sanitising_removes_separators_control_characters_and_bad_numbers() {
        let dirty = Settings {
            last_drive: "C:\\\nevil=1".to_string(),
            excludes: vec!["a|b".to_string(), "line\nbreak".to_string()],
            min_folder_size_mb: f64::NAN,
            max_folder_size_gb: 1.0e30,
            min_folder_size_unit: "evil".to_string(),
            max_folder_size_unit: "TB".to_string(),
            view_mode: "evil".to_string(),
            language: "fr".to_string(),
            cache_hashes_enabled: false,
            measure_allocated: false,
        };
        let clean = dirty.sanitized();
        assert_eq!(clean.last_drive, "C:\\evil=1");
        assert_eq!(
            clean.excludes,
            vec!["ab".to_string(), "linebreak".to_string()]
        );
        assert_eq!(clean.min_folder_size_mb, 0.0);
        assert_eq!(clean.max_folder_size_gb, MAX_LIMIT);
        assert_eq!(clean.min_folder_size_unit, "MB");
        assert_eq!(clean.max_folder_size_unit, "GB");
        assert_eq!(clean.view_mode, "list");
        assert!(clean.language == "de" || clean.language == "en");
    }

    #[test]
    fn a_leftover_treemap_view_mode_falls_back_to_list() {
        // The treemap view existed in an earlier build and could still be sitting in an
        // existing user's settings file; it must degrade to the default, not be kept as an
        // unhandled value or crash the view-mode switch in app.js.
        assert_eq!(
            parse_settings("[general]\nviewMode=treemap\n").view_mode,
            "list"
        );
    }

    #[test]
    fn serialisation_round_trips() {
        let original = Settings {
            last_drive: "C:\\".to_string(),
            excludes: vec!["\\a".to_string(), "\\b c".to_string()],
            min_folder_size_mb: 12.5,
            max_folder_size_gb: 3.0,
            min_folder_size_unit: "GB".to_string(),
            max_folder_size_unit: "MB".to_string(),
            view_mode: "grid".to_string(),
            language: "en".to_string(),
            cache_hashes_enabled: true,
            measure_allocated: true,
        };
        let restored = parse_settings(&serialize_settings(&original));
        assert_eq!(restored.excludes, original.excludes);
        assert_eq!(restored.min_folder_size_mb, 12.5);
        assert_eq!(restored.min_folder_size_unit, "GB");
        assert_eq!(restored.max_folder_size_unit, "MB");
        assert_eq!(restored.view_mode, "grid");
        assert_eq!(restored.language, "en");
        assert!(restored.cache_hashes_enabled);
        assert!(restored.measure_allocated);
        assert!(!parse_settings("[general]\n").measure_allocated);
    }

    #[test]
    fn cache_hashes_enabled_defaults_to_off_and_round_trips() {
        assert!(!parse_settings("[general]\n").cache_hashes_enabled);
        assert!(parse_settings("[general]\ncacheHashesEnabled=1\n").cache_hashes_enabled);
        assert!(!parse_settings("[general]\ncacheHashesEnabled=0\n").cache_hashes_enabled);
    }

    #[test]
    fn only_supported_languages_are_kept() {
        assert_eq!(parse_settings("[general]\nlanguage=de\n").language, "de");
        assert_eq!(parse_settings("[general]\nlanguage=en\n").language, "en");
        let fallback = parse_settings("[general]\nlanguage=xx\n").language;
        assert!(fallback == "de" || fallback == "en");
    }
}
