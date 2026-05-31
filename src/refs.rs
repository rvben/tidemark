//! Resolve a ref string to a Manifest. A ref is `@` (current tree),
//! a manifest file path, or a store label.

use crate::error::KairnError;
use crate::manifest::Manifest;
use crate::store::Store;
use crate::walk::WalkOptions;
use std::path::Path;

/// Resolve `r` against `base` (the directory whose store and tree we use).
pub fn resolve(
    r: &str,
    base: &Path,
    store: &Store,
    walk_opts: &WalkOptions,
) -> Result<Manifest, KairnError> {
    if r == "@" {
        return crate::builder::build_manifest(base, walk_opts);
    }
    let as_path = Path::new(r);
    if as_path.is_file() {
        let data = std::fs::read_to_string(as_path)?;
        return serde_json::from_str(&data)
            .map_err(|e| KairnError::invalid(format!("corrupt manifest {r}: {e}")));
    }
    store.load_label(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::walk::WalkOptions;
    use std::fs;

    #[test]
    fn at_resolves_to_current_tree() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let store = Store::at(tmp.path());
        let m = resolve("@", tmp.path(), &store, &WalkOptions::default()).unwrap();
        assert_eq!(m.entry_count, 1);
    }

    #[test]
    fn label_resolves_from_store() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let store = Store::at(tmp.path());
        let snap = crate::builder::build_manifest(tmp.path(), &WalkOptions::default()).unwrap();
        store.save("before", &snap, false).unwrap();
        let m = resolve("before", tmp.path(), &store, &WalkOptions::default()).unwrap();
        assert_eq!(m.tree_digest, snap.tree_digest);
    }

    #[test]
    fn file_path_resolves_as_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let snap = crate::builder::build_manifest(tmp.path(), &WalkOptions::default()).unwrap();
        let mpath = tmp.path().join("snap.kairn");
        fs::write(&mpath, serde_json::to_string(&snap).unwrap()).unwrap();
        let store = Store::at(tmp.path());
        let m = resolve(
            mpath.to_str().unwrap(),
            tmp.path(),
            &store,
            &WalkOptions::default(),
        )
        .unwrap();
        assert_eq!(m.tree_digest, snap.tree_digest);
    }
}
