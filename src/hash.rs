//! Content hashing (blake3) and symlink target capture.

use crate::error::{ErrorKind, KairnError};
use std::path::Path;

/// Hash a byte slice, returning `"blake3:<hex>"`.
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

/// Read a symlink's target as a UTF-8 string.
pub fn read_link(path: &Path) -> Result<String, KairnError> {
    let target = std::fs::read_link(path)?;
    target
        .to_str()
        .map(|s| s.replace('\\', "/"))
        .ok_or_else(|| {
            KairnError::new(
                ErrorKind::Unsupported,
                "non-UTF-8 symlink target".to_string(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_hash() {
        assert_eq!(hash_bytes(b"hello"), hash_bytes(b"hello"));
    }

    #[test]
    fn different_content_different_hash() {
        assert_ne!(hash_bytes(b"hello"), hash_bytes(b"world"));
    }

    #[test]
    fn hash_has_prefix() {
        assert!(hash_bytes(b"x").starts_with("blake3:"));
    }
}
