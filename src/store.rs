//! Labeled snapshot store under `.tidemark/snapshots/`.

use crate::error::TidemarkError;
use crate::manifest::Manifest;
use std::path::{Path, PathBuf};

/// A labeled snapshot store rooted at `<base>/.tidemark`.
pub struct Store {
    root: PathBuf,
}

/// A summary row describing one stored snapshot.
#[derive(serde::Serialize)]
pub struct StoreItem {
    pub label: String,
    pub created_at: String,
    pub entry_count: usize,
    pub tree_digest: String,
}

impl Store {
    /// Construct a store rooted at `<base>/.tidemark`.
    pub fn at(base: &Path) -> Self {
        Store {
            root: base.join(".tidemark"),
        }
    }

    /// Create the store directory. Idempotent: succeeds if it already exists.
    pub fn init(&self) -> Result<(), TidemarkError> {
        std::fs::create_dir_all(self.snap_dir())?;
        Ok(())
    }

    fn snap_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }
    fn path_for(&self, label: &str) -> PathBuf {
        self.snap_dir().join(format!("{label}.json"))
    }

    fn validate_label(label: &str) -> Result<(), TidemarkError> {
        if label.is_empty()
            || label.contains('/')
            || label.contains('\\')
            || label.contains("..")
            || label.chars().any(|c| c.is_control())
        {
            return Err(TidemarkError::invalid(format!("invalid label: {label:?}")));
        }
        Ok(())
    }

    /// Save a manifest under `label`. Idempotent: an identical tree is a no-op
    /// success. A different tree under an existing label returns `conflict`
    /// unless `force`.
    pub fn save(&self, label: &str, m: &Manifest, force: bool) -> Result<(), TidemarkError> {
        Self::validate_label(label)?;
        let path = self.path_for(label);
        if path.exists() && !force {
            let existing = self.load_label(label)?;
            if existing.tree_digest == m.tree_digest {
                return Ok(());
            }
            return Err(TidemarkError::conflict(format!(
                "label {label:?} exists with a different tree (use --force to overwrite)"
            )));
        }
        std::fs::create_dir_all(self.snap_dir())?;
        let json = serde_json::to_string_pretty(m).map_err(|e| TidemarkError::io(e.to_string()))?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load a manifest by label.
    pub fn load_label(&self, label: &str) -> Result<Manifest, TidemarkError> {
        Self::validate_label(label)?;
        let path = self.path_for(label);
        if !path.exists() {
            return Err(TidemarkError::not_found(format!(
                "no snapshot labeled {label:?}"
            )));
        }
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data)
            .map_err(|e| TidemarkError::invalid(format!("corrupt manifest: {e}")))
    }

    /// List stored snapshots, oldest first.
    pub fn list(&self) -> Result<Vec<StoreItem>, TidemarkError> {
        let dir = self.snap_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let label = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if let Ok(m) = self.load_label(&label) {
                items.push(StoreItem {
                    label,
                    created_at: m.created_at,
                    entry_count: m.entry_count,
                    tree_digest: m.tree_digest,
                });
            }
        }
        items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(items)
    }

    /// Most recently created label, if any.
    pub fn latest(&self) -> Result<Option<String>, TidemarkError> {
        Ok(self.list()?.into_iter().next_back().map(|i| i.label))
    }

    /// Remove a stored snapshot by label.
    pub fn remove(&self, label: &str) -> Result<(), TidemarkError> {
        Self::validate_label(label)?;
        let path = self.path_for(label);
        if !path.exists() {
            return Err(TidemarkError::not_found(format!(
                "no snapshot labeled {label:?}"
            )));
        }
        std::fs::remove_file(&path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Entry, EntryKind, Manifest};

    fn man(hash: &str) -> Manifest {
        Manifest::build(
            "/r".into(),
            "t".into(),
            vec![Entry {
                path: "a".into(),
                kind: EntryKind::File,
                size: Some(1),
                mode: Some(0o644),
                hash: Some(hash.into()),
                target: None,
                mtime: None,
                content: None,
            }],
        )
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        s.save("before", &man("h1"), false).unwrap();
        let got = s.load_label("before").unwrap();
        assert_eq!(got.tree_digest, man("h1").tree_digest);
    }

    #[test]
    fn save_same_tree_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        s.save("x", &man("h1"), false).unwrap();
        s.save("x", &man("h1"), false).unwrap();
    }

    #[test]
    fn save_different_tree_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        s.save("x", &man("h1"), false).unwrap();
        let err = s.save("x", &man("h2"), false).unwrap_err();
        assert_eq!(err.kind, crate::error::ErrorKind::Conflict);
        s.save("x", &man("h2"), true).unwrap();
    }

    #[test]
    fn rejects_path_traversal_label() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        let err = s.save("../evil", &man("h1"), false).unwrap_err();
        assert_eq!(err.kind, crate::error::ErrorKind::InvalidInput);
    }

    #[test]
    fn load_missing_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        assert_eq!(
            s.load_label("nope").unwrap_err().kind,
            crate::error::ErrorKind::NotFound
        );
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        s.init().unwrap();
        assert!(tmp.path().join(".tidemark/snapshots").is_dir());
        s.init().unwrap(); // second call must not error
    }

    #[test]
    fn remove_deletes_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::at(tmp.path());
        s.save("x", &man("h1"), false).unwrap();
        s.remove("x").unwrap();
        assert_eq!(
            s.load_label("x").unwrap_err().kind,
            crate::error::ErrorKind::NotFound
        );
    }
}
