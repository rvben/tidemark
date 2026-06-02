//! Error types mapped to clispec error kinds.

use std::fmt;

/// Finite, documented set of error categories (clispec `kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    NotFound,
    Conflict,
    InvalidInput,
    Io,
    Unsupported,
}

impl ErrorKind {
    /// Whether retrying the same operation might succeed.
    pub fn retryable(self) -> bool {
        matches!(self, ErrorKind::Io)
    }
}

/// An error with a clispec `kind` and a human-readable message.
#[derive(Debug)]
pub struct TidemarkError {
    pub kind: ErrorKind,
    pub message: String,
}

impl TidemarkError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, m)
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, m)
    }
    pub fn invalid(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, m)
    }
    pub fn io(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Io, m)
    }
}

impl fmt::Display for TidemarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for TidemarkError {}

impl From<std::io::Error> for TidemarkError {
    fn from(e: std::io::Error) -> Self {
        TidemarkError::io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn io_is_retryable_others_not() {
        assert!(ErrorKind::Io.retryable());
        assert!(!ErrorKind::Conflict.retryable());
        assert!(!ErrorKind::NotFound.retryable());
    }
}
