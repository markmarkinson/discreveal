//! System shell icons for file types.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use std::path::Path;

const ICON_SIZE: i32 = 32;
const MAX_PATHS_PER_REQUEST: usize = 256;
const MAX_EXTENSION_LEN: usize = 16;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIcon {
    path: String,
    data_url: Option<String>,
}

/// Only short alphanumeric extensions are looked up, so arbitrary strings
/// never reach the shell API.
fn valid_extension(path: &str) -> Option<&str> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| {
            !ext.is_empty()
                && ext.len() <= MAX_EXTENSION_LEN
                && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// Returns the shell icon for the extension of each path as a PNG data URL.
/// The icon depends on the file type only, so the caller passes one
/// representative path per extension.
#[tauri::command(async)]
pub fn get_file_icons(paths: Vec<String>) -> Vec<FileIcon> {
    paths
        .into_iter()
        .take(MAX_PATHS_PER_REQUEST)
        .map(|path| {
            let data_url = valid_extension(&path)
                // The shell API resolves the icon from the leading dot, not from a bare word.
                .and_then(|ext| systemicons::get_icon(&format!(".{ext}"), ICON_SIZE).ok())
                .map(|png| format!("data:image/png;base64,{}", STANDARD.encode(png)));
            FileIcon { path, data_url }
        })
        .collect()
}
