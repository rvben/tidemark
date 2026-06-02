//! Directory traversal with .gitignore/.tidemarkignore support.

use crate::error::{ErrorKind, TidemarkError};
use std::path::{Path, PathBuf};

/// Options controlling how the tree is walked.
pub struct WalkOptions {
    /// Include dotfiles.
    pub hidden: bool,
    /// Honor .gitignore/.tidemarkignore files.
    pub use_ignore: bool,
    /// Additional globs to exclude.
    pub extra_ignores: Vec<String>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            hidden: false,
            use_ignore: true,
            extra_ignores: Vec::new(),
        }
    }
}

/// A path discovered by the walker, relative to the root, plus its absolute path.
pub struct RawEntry {
    pub rel: String,
    pub abs: PathBuf,
}

/// Walk `root`, returning files and symlinks (not directories) as relative paths.
pub fn walk_tree(root: &Path, opts: &WalkOptions) -> Result<Vec<RawEntry>, TidemarkError> {
    use ignore::WalkBuilder;
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!opts.hidden)
        .git_ignore(opts.use_ignore)
        .git_global(opts.use_ignore)
        .git_exclude(opts.use_ignore)
        .ignore(opts.use_ignore)
        .parents(opts.use_ignore)
        .require_git(false)
        .add_custom_ignore_filename(".tidemarkignore")
        .follow_links(false);

    if !opts.extra_ignores.is_empty() {
        let mut ov = ignore::overrides::OverrideBuilder::new(root);
        for g in &opts.extra_ignores {
            // An override entry beginning with '!' marks a glob as ignored.
            ov.add(&format!("!{g}"))
                .map_err(|e| TidemarkError::invalid(e.to_string()))?;
        }
        let ov = ov
            .build()
            .map_err(|e| TidemarkError::invalid(e.to_string()))?;
        builder.overrides(ov);
    }

    let mut out = Vec::new();
    for result in builder.build() {
        let dent = result.map_err(|e| TidemarkError::io(e.to_string()))?;
        let path = dent.path();
        if path == root {
            continue;
        }
        let ft = match dent.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if ft.is_dir() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|e| TidemarkError::io(e.to_string()))?;
        let rel = rel.to_str().ok_or_else(|| {
            TidemarkError::new(
                ErrorKind::Unsupported,
                format!("non-UTF-8 path: {}", rel.display()),
            )
        })?;
        let rel = rel.replace('\\', "/");
        // tidemark never records its own store, even when --hidden is set.
        if rel == ".tidemark" || rel.starts_with(".tidemark/") {
            continue;
        }
        out.push(RawEntry {
            rel,
            abs: path.to_path_buf(),
        });
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_files_skips_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        fs::write(tmp.path().join("sub/b.txt"), b"yo").unwrap();
        let got = walk_tree(tmp.path(), &WalkOptions::default()).unwrap();
        let rels: Vec<_> = got.iter().map(|e| e.rel.clone()).collect();
        assert_eq!(rels, vec!["a.txt", "sub/b.txt"]);
    }

    #[test]
    fn honors_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), b"ignored.txt\n").unwrap();
        fs::write(tmp.path().join("ignored.txt"), b"x").unwrap();
        fs::write(tmp.path().join("kept.txt"), b"y").unwrap();
        let got = walk_tree(tmp.path(), &WalkOptions::default()).unwrap();
        let rels: Vec<_> = got.iter().map(|e| e.rel.clone()).collect();
        assert!(rels.contains(&"kept.txt".to_string()));
        assert!(!rels.contains(&"ignored.txt".to_string()));
    }

    #[test]
    fn hidden_excluded_by_default_included_with_flag() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".secret"), b"x").unwrap();
        let def = walk_tree(tmp.path(), &WalkOptions::default()).unwrap();
        assert!(def.is_empty());
        let opts = WalkOptions {
            hidden: true,
            ..Default::default()
        };
        let all = walk_tree(tmp.path(), &opts).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn always_excludes_own_store_even_when_hidden() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".tidemark/snapshots")).unwrap();
        fs::write(tmp.path().join(".tidemark/snapshots/x.json"), b"{}").unwrap();
        fs::write(tmp.path().join("real.txt"), b"x").unwrap();
        let opts = WalkOptions {
            hidden: true,
            ..Default::default()
        };
        let got = walk_tree(tmp.path(), &opts).unwrap();
        let rels: Vec<_> = got.iter().map(|e| e.rel.clone()).collect();
        assert_eq!(rels, vec!["real.txt"]);
    }

    #[test]
    fn extra_ignore_glob_excludes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("keep.rs"), b"x").unwrap();
        fs::write(tmp.path().join("skip.log"), b"x").unwrap();
        let opts = WalkOptions {
            extra_ignores: vec!["*.log".into()],
            ..Default::default()
        };
        let got = walk_tree(tmp.path(), &opts).unwrap();
        let rels: Vec<_> = got.iter().map(|e| e.rel.clone()).collect();
        assert_eq!(rels, vec!["keep.rs"]);
    }
}
