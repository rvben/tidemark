//! Compose walk + hash into a Manifest.

use crate::error::KairnError;
use crate::manifest::{Entry, EntryKind, Manifest};
use crate::walk::{WalkOptions, walk_tree};
use std::path::Path;

/// Current UTC time as RFC 3339.
fn now_rfc3339() -> String {
    jiff::Timestamp::now().to_string()
}

/// Build a manifest of `root` applying `opts`.
pub fn build_manifest(root: &Path, opts: &WalkOptions) -> Result<Manifest, KairnError> {
    let canon = std::fs::canonicalize(root)
        .map_err(|_| KairnError::not_found(format!("path not found: {}", root.display())))?;
    let raw = walk_tree(&canon, opts)?;
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
            }
        } else {
            Entry {
                path: r.rel,
                kind: EntryKind::File,
                size: Some(meta.len()),
                mode: mode_of(&meta),
                hash: Some(crate::hash::hash_file(&r.abs)?),
                target: None,
                mtime: mtime_of(&meta),
            }
        };
        entries.push(entry);
    }
    let root_str = canon.to_string_lossy().to_string();
    Ok(Manifest::build(root_str, now_rfc3339(), entries))
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
    use crate::walk::WalkOptions;
    use std::fs;

    #[test]
    fn builds_manifest_with_hashes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let m = build_manifest(tmp.path(), &WalkOptions::default()).unwrap();
        assert_eq!(m.entry_count, 1);
        assert_eq!(m.entries[0].path, "a.txt");
        assert!(m.entries[0].hash.as_ref().unwrap().starts_with("blake3:"));
    }

    #[test]
    fn unchanged_tree_same_digest() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let m1 = build_manifest(tmp.path(), &WalkOptions::default()).unwrap();
        let m2 = build_manifest(tmp.path(), &WalkOptions::default()).unwrap();
        assert_eq!(m1.tree_digest, m2.tree_digest);
    }

    #[test]
    fn missing_path_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let err = build_manifest(&missing, &WalkOptions::default()).unwrap_err();
        assert_eq!(err.kind, crate::error::ErrorKind::NotFound);
    }
}
