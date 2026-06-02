//! Resolve a ref string to a Manifest. A ref is `@` (current tree),
//! a manifest file path, or a store label.

use crate::builder::SnapOptions;
use crate::error::TidemarkError;
use crate::manifest::Manifest;
use crate::store::Store;
use std::path::Path;

/// Resolve `r` against `base` (the directory whose store and tree we use).
///
/// Precedence:
/// - `@` is the current tree.
/// - A path-like ref (contains a path separator) is read as a manifest file.
/// - A bare name is a store label first; only if no such label exists is it
///   tried as a manifest file in the current directory. This keeps a stored
///   label from being silently shadowed by a same-named file in the cwd.
pub fn resolve(
    r: &str,
    base: &Path,
    store: &Store,
    snap_opts: &SnapOptions,
) -> Result<Manifest, TidemarkError> {
    if r == "@" {
        return crate::builder::build_manifest(base, snap_opts);
    }
    if r.contains('/') || r.contains('\\') {
        return load_manifest_file(r);
    }
    match store.load_label(r) {
        Ok(m) => Ok(m),
        Err(label_err) => {
            // Fall back to a bare filename only if it actually exists on disk.
            if Path::new(r).is_file() {
                load_manifest_file(r)
            } else {
                Err(label_err)
            }
        }
    }
}

fn load_manifest_file(path: &str) -> Result<Manifest, TidemarkError> {
    let data = std::fs::read_to_string(path)
        .map_err(|_| TidemarkError::not_found(format!("no such manifest file or label: {path}")))?;
    serde_json::from_str(&data)
        .map_err(|e| TidemarkError::invalid(format!("corrupt manifest {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use std::fs;

    #[test]
    fn at_resolves_to_current_tree() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let store = Store::at(tmp.path());
        let m = resolve("@", tmp.path(), &store, &SnapOptions::default()).unwrap();
        assert_eq!(m.entry_count, 1);
    }

    #[test]
    fn label_resolves_from_store() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let store = Store::at(tmp.path());
        let snap = crate::builder::build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        store.save("before", &snap, false).unwrap();
        let m = resolve("before", tmp.path(), &store, &SnapOptions::default()).unwrap();
        assert_eq!(m.tree_digest, snap.tree_digest);
    }

    #[test]
    fn bare_name_prefers_store_label_over_cwd_file() {
        // A file named exactly like a label exists in cwd; a bare ref must resolve
        // to the stored label, not the cwd file, to avoid silent shadowing.
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let store = Store::at(tmp.path());
        let snap = crate::builder::build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        store.save("before", &snap, false).unwrap();
        // Decoy file in cwd with the same bare name as the label.
        fs::write(tmp.path().join("before"), b"not a manifest").unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = resolve("before", tmp.path(), &store, &SnapOptions::default());
        std::env::set_current_dir(prev).unwrap();
        let m = result.expect("bare name should resolve to the stored label");
        assert_eq!(m.tree_digest, snap.tree_digest);
    }

    #[test]
    fn file_path_resolves_as_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let snap = crate::builder::build_manifest(tmp.path(), &SnapOptions::default()).unwrap();
        let mpath = tmp.path().join("snap.tidemark");
        fs::write(&mpath, serde_json::to_string(&snap).unwrap()).unwrap();
        let store = Store::at(tmp.path());
        let m = resolve(
            mpath.to_str().unwrap(),
            tmp.path(),
            &store,
            &SnapOptions::default(),
        )
        .unwrap();
        assert_eq!(m.tree_digest, snap.tree_digest);
    }
}
