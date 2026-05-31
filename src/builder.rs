//! Compose walk + hash into a Manifest.

use crate::error::KairnError;
use crate::manifest::{Entry, EntryKind, Manifest};
use crate::walk::{WalkOptions, walk_tree};
use std::path::Path;

/// Maximum size of a file whose text content is stored inline for content diffs.
pub const CONTENT_CAP_BYTES: usize = 256 * 1024;

/// Options for building a snapshot: how to walk, and whether to store text content.
pub struct SnapOptions {
    pub walk: WalkOptions,
    /// Store inline text content for small UTF-8 files (enables `diff --content`).
    pub store_content: bool,
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            walk: WalkOptions::default(),
            store_content: true,
        }
    }
}

/// Current UTC time as RFC 3339.
fn now_rfc3339() -> String {
    jiff::Timestamp::now().to_string()
}

/// Build a manifest of `root` applying `opts`.
pub fn build_manifest(root: &Path, opts: &SnapOptions) -> Result<Manifest, KairnError> {
    let canon = std::fs::canonicalize(root)
        .map_err(|_| KairnError::not_found(format!("path not found: {}", root.display())))?;
    let raw = walk_tree(&canon, &opts.walk)?;
    let mut entries = Vec::with_capacity(raw.len());
    for r in raw {
        let meta = std::fs::symlink_metadata(&r.abs)?;
        let ft = meta.file_type();
        let entry = if ft.is_symlink() {
            Entry {
                path: r.rel,
                kind: EntryKind::Symlink,
                size: None,
                mode: mode_of(&meta),
                hash: None,
                target: Some(crate::hash::read_link(&r.abs)?),
                mtime: mtime_of(&meta),
                content: None,
            }
        } else {
            let bytes = std::fs::read(&r.abs)?;
            let hash = crate::hash::hash_bytes(&bytes);
            let content = inline_content(&bytes, opts.store_content);
            Entry {
                path: r.rel,
                kind: EntryKind::File,
                size: Some(meta.len()),
                mode: mode_of(&meta),
                hash: Some(hash),
                target: None,
                mtime: mtime_of(&meta),
                content,
            }
        };
        entries.push(entry);
    }
    let root_str = canon.to_string_lossy().to_string();
    Ok(Manifest::build(root_str, now_rfc3339(), entries))
}

/// Return inline text content for small UTF-8 files when storage is enabled.
fn inline_content(bytes: &[u8], store_content: bool) -> Option<String> {
    if !store_content || bytes.len() > CONTENT_CAP_BYTES {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(unix)]
fn mode_of(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode() & 0o7777)
}
#[cfg(not(unix))]
fn mode_of(_meta: &std::fs::Metadata) -> Option<u32> {
    None
}

fn mtime_of(meta: &std::fs::Metadata) -> Option<String> {
    let mt = meta.modified().ok()?;
    let ts = jiff::Timestamp::try_from(mt).ok()?;
    Some(ts.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builds_manifest_with_hashes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let m = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        assert_eq!(m.entry_count, 1);
        assert_eq!(m.entries[0].path, "a.txt");
        assert!(m.entries[0].hash.as_ref().unwrap().starts_with("blake3:"));
    }

    #[test]
    fn unchanged_tree_same_digest() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let m1 = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        let m2 = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        assert_eq!(m1.tree_digest, m2.tree_digest);
    }

    #[test]
    fn missing_path_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let err = build_manifest(&missing, &SnapOptions::default()).unwrap_err();
        assert_eq!(err.kind, crate::error::ErrorKind::NotFound);
    }

    #[test]
    fn stores_text_content_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"line1\nline2\n").unwrap();
        let m = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        assert_eq!(m.entries[0].content.as_deref(), Some("line1\nline2\n"));
    }

    #[test]
    fn skips_content_when_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let opts = SnapOptions {
            store_content: false,
            ..Default::default()
        };
        let m = build_manifest(tmp.path(), &opts).unwrap();
        assert_eq!(m.entries[0].content, None);
    }

    #[test]
    fn skips_content_for_binary_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("b.bin"), [0u8, 159, 146, 150]).unwrap();
        let m = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        assert_eq!(m.entries[0].content, None);
        // but it is still hashed
        assert!(m.entries[0].hash.as_ref().unwrap().starts_with("blake3:"));
    }

    #[test]
    fn skips_content_for_large_files() {
        let tmp = tempfile::tempdir().unwrap();
        let big = "x".repeat(super::CONTENT_CAP_BYTES + 1);
        fs::write(tmp.path().join("big.txt"), big.as_bytes()).unwrap();
        let m = build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        assert_eq!(m.entries[0].content, None);
    }
}
