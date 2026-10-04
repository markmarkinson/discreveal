//! Error type returned to the frontend.
//!
//! Commands never return display text. They return a stable `code` (and an
//! optional `detail` such as an operating-system message) which the frontend
//! translates into the user's language.

use serde::Serialize;
use std::fmt;

/// An error the frontend can translate: `{ "code": "...", "detail": "..." }`.
#[derive(Debug, Serialize)]
pub struct AppError {
    pub(crate) code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    pub fn with_detail(code: &'static str, detail: impl fmt::Display) -> Self {
        Self {
            code,
            detail: Some(detail.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::with_detail("ioError", error)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(formatter, "{}: {detail}", self.code),
            None => formatter.write_str(self.code),
        }
    }
}
