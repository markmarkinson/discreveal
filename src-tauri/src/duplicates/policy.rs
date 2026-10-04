//! Pure duplicate scan options and cleanup suggestion policy.
//! File identity, byte equality and Windows protection are checked by their I/O adapters.
use serde::Deserialize;
use std::path::Path;

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DuplicateFilter {
    pub(super) min_bytes: u64,
    pub(super) max_bytes: u64,
    pub(super) type_mode: TypeMode,
    pub(super) extensions: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum TypeMode {
    #[default]
    All,
    Include,
    Exclude,
}

impl DuplicateFilter {
    pub(super) fn normalized(mut self) -> crate::error::AppResult<Self> {
        if self.max_bytes > 0 && self.min_bytes > self.max_bytes {
            return Err(crate::error::AppError::new("invalidSizeLimits"));
        }
        self.extensions = self
            .extensions
            .into_iter()
            .map(|extension| extension.trim().trim_start_matches('.').to_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        Ok(self)
    }

    pub(super) fn accepts_size(&self, size: u64) -> bool {
        size >= self.min_bytes && (self.max_bytes == 0 || size <= self.max_bytes)
    }
    pub(super) fn accepts(&self, path: &Path, size: u64) -> bool {
        if !self.accepts_size(size) {
            return false;
        }
        if matches!(self.type_mode, TypeMode::All) {
            return true;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let matches = self
            .extensions
            .iter()
            .any(|extension| name.ends_with(&format!(".{extension}")));
        match self.type_mode {
            TypeMode::All => true,
            TypeMode::Include => matches,
            TypeMode::Exclude => !matches,
        }
    }
}

/// Bulk suggestions cover personal documents/media; application data requires individual review.
pub(super) fn cleanup_eligible(path: &Path, protected: bool) -> bool {
    if protected {
        return false;
    }
    let lower = path.to_string_lossy().to_lowercase();
    if lower.split(['\\', '/']).any(|part| {
        matches!(
            part,
            "appdata"
                | "program files"
                | "program files (x86)"
                | "programdata"
                | "windows"
                | "node_modules"
                | ".git"
                | ".rustup"
                | ".cargo"
                | ".venv"
                | "venv"
        )
    }) {
        return false;
    }
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "txt"
            | "pdf"
            | "doc"
            | "docx"
            | "odt"
            | "rtf"
            | "xls"
            | "xlsx"
            | "ods"
            | "ppt"
            | "pptx"
            | "odp"
            | "csv"
            | "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "webp"
            | "bmp"
            | "tif"
            | "tiff"
            | "heic"
            | "svg"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "m4a"
            | "mp4"
            | "mkv"
            | "avi"
            | "mov"
            | "webm"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn size_and_type_validation_is_independent_of_filesystem_and_ipc() {
        let invalid = DuplicateFilter {
            min_bytes: 20,
            max_bytes: 10,
            ..Default::default()
        };
        assert_eq!(
            invalid.normalized().err().unwrap().code,
            "invalidSizeLimits"
        );
        let filter = DuplicateFilter {
            min_bytes: 10,
            max_bytes: 20,
            type_mode: TypeMode::Include,
            extensions: vec![" .PDF ".into(), "".into()],
        }
        .normalized()
        .unwrap();
        assert!(filter.accepts(Path::new("REPORT.PDF"), 10));
        assert!(filter.accepts(Path::new("REPORT.PDF"), 20));
        assert!(!filter.accepts(Path::new("REPORT.PDF"), 21));
        assert!(!filter.accepts(Path::new("REPORT.JPG"), 10));
    }
    #[test]
    fn suggestions_require_personal_media_location_and_external_protection_clearance() {
        assert!(cleanup_eligible(
            Path::new(r"C:\Users\Example\Documents\Report.pdf"),
            false
        ));
        assert!(!cleanup_eligible(
            Path::new(r"C:\Users\Example\Documents\Report.pdf"),
            true
        ));
        assert!(!cleanup_eligible(
            Path::new(r"C:\Users\Example\AppData\Report.pdf"),
            false
        ));
        assert!(!cleanup_eligible(
            Path::new(r"C:\Users\Example\Documents\Program.exe"),
            false
        ));
    }
}
