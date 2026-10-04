//! Small helpers for calling Win32 APIs.

/// Null-terminated UTF-16 copy of `text` for wide-string Win32 calls.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
