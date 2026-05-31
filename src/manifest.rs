//! Manifest data model and deterministic Merkle digest. Pure (no filesystem).

use serde::{Deserialize, Serialize};

pub const MANIFEST_VERSION: u32 = 1;

/// The kind of filesystem entry recorded in a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Symlink,
    Dir,
}

/// A single recorded path and its tracked attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub kind: EntryKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime: Option<String>,
}

/// A deterministic snapshot of a directory tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub kairn_version: String,
    pub manifest_version: u32,
    pub root: String,
    pub created_at: String,
    pub tree_digest: String,
    pub entry_count: usize,
    pub entries: Vec<Entry>,
}

impl Manifest {
    /// Build from entries, sorting by path and computing the Merkle digest.
    /// `created_at` is informational and excluded from the digest.
    pub fn build(root: String, created_at: String, mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let tree_digest = compute_tree_digest(&entries);
        let entry_count = entries.len();
        Manifest {
            kairn_version: env!("CARGO_PKG_VERSION").to_string(),
            manifest_version: MANIFEST_VERSION,
            root,
            created_at,
            tree_digest,
            entry_count,
            entries,
        }
    }
}

/// Merkle root over sorted entries. Excludes mtime (informational) so that
/// `touch` does not change the digest.
pub fn compute_tree_digest(entries: &[Entry]) -> String {
    let mut hasher = blake3::Hasher::new();
    for e in entries {
        hasher.update(e.path.as_bytes());
        hasher.update(&[0]);
        hasher.update(format!("{:?}", e.kind).as_bytes());
        hasher.update(&[0]);
        hasher.update(e.hash.as_deref().unwrap_or("").as_bytes());
        hasher.update(&[0]);
        hasher.update(e.target.as_deref().unwrap_or("").as_bytes());
        hasher.update(&[0]);
        hasher.update(e.mode.unwrap_or(0).to_le_bytes().as_slice());
        hasher.update(&[0xff]);
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, hash: &str) -> Entry {
        Entry {
            path: path.into(),
            kind: EntryKind::File,
            size: Some(1),
            mode: Some(0o644),
            hash: Some(hash.into()),
            target: None,
            mtime: None,
        }
    }

    #[test]
    fn digest_is_order_independent() {
        let a = Manifest::build("/r".into(), "t1".into(), vec![file("b", "h2"), file("a", "h1")]);
        let b = Manifest::build("/r".into(), "t2".into(), vec![file("a", "h1"), file("b", "h2")]);
        assert_eq!(
            a.tree_digest, b.tree_digest,
            "digest must not depend on input order or created_at"
        );
    }

    #[test]
    fn digest_changes_when_content_changes() {
        let a = Manifest::build("/r".into(), "t".into(), vec![file("a", "h1")]);
        let b = Manifest::build("/r".into(), "t".into(), vec![file("a", "h2")]);
        assert_ne!(a.tree_digest, b.tree_digest);
    }

    #[test]
    fn digest_changes_when_mode_changes() {
        let mut e = file("a", "h1");
        let a = Manifest::build("/r".into(), "t".into(), vec![e.clone()]);
        e.mode = Some(0o755);
        let b = Manifest::build("/r".into(), "t".into(), vec![e]);
        assert_ne!(a.tree_digest, b.tree_digest, "chmod must register as a change");
    }

    #[test]
    fn roundtrips_through_json() {
        let m = Manifest::build("/r".into(), "t".into(), vec![file("a", "h1")]);
        let s = serde_json::to_string(&m).unwrap();
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }
}
