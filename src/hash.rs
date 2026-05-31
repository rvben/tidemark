//! Content hashing (blake3) and symlink target capture.

use crate::error::{ErrorKind, KairnError};
use std::path::Path;

/// Hash a file's contents, returning `"blake3:<hex>"`.
pub fn hash_file(path: &Path) -> Result<String, KairnError> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("blake3:{}", hasher.finalize().to_hex()))
}

/// Read a symlink's target as a UTF-8 string.
pub fn read_link(path: &Path) -> Result<String, KairnError> {
    let target = std::fs::read_link(path)?;
    target
        .to_str()
        .map(|s| s.replace('\\', "/"))
        .ok_or_else(|| {
            KairnError::new(ErrorKind::Unsupported, "non-UTF-8 symlink target".to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn same_content_same_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::write(&a, b"hello").unwrap();
        fs::write(&b, b"hello").unwrap();
        assert_eq!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
    }

    #[test]
    fn different_content_different_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::write(&a, b"hello").unwrap();
        fs::write(&b, b"world").unwrap();
        assert_ne!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
    }

    #[test]
    fn hash_has_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        fs::write(&a, b"x").unwrap();
        assert!(hash_file(&a).unwrap().starts_with("blake3:"));
    }
}
