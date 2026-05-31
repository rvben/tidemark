//! Pure diff over two manifests.

use crate::manifest::{Entry, Manifest};
use serde::Serialize;
use std::collections::BTreeMap;

/// The classification of a single change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// One change between two manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_delta: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_preview: Option<String>,
}

/// The full delta between two manifests with summary counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffReport {
    pub changes: Vec<Change>,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
}

impl DiffReport {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

fn entry_changed(a: &Entry, b: &Entry) -> bool {
    a.hash != b.hash || a.mode != b.mode || a.target != b.target || a.kind != b.kind
}

/// Compute the delta from `old` to `new`. Detects renames as a (deleted, added)
/// pair sharing identical content hash and mode.
pub fn diff(old: &Manifest, new: &Manifest) -> DiffReport {
    let old_map: BTreeMap<&str, &Entry> =
        old.entries.iter().map(|e| (e.path.as_str(), e)).collect();
    let new_map: BTreeMap<&str, &Entry> =
        new.entries.iter().map(|e| (e.path.as_str(), e)).collect();

    let mut added = Vec::new();
    let mut deleted = Vec::new();
    let mut modified = Vec::new();

    for (path, ne) in &new_map {
        match old_map.get(path) {
            None => added.push(*ne),
            Some(oe) if entry_changed(oe, ne) => modified.push((*oe, *ne)),
            Some(_) => {}
        }
    }
    for (path, oe) in &old_map {
        if !new_map.contains_key(path) {
            deleted.push(*oe);
        }
    }

    // Rename detection: match deleted+added with identical hash+mode+kind.
    let mut renamed: Vec<(&Entry, &Entry)> = Vec::new();
    let mut used_added = vec![false; added.len()];
    let mut used_deleted = vec![false; deleted.len()];
    for (di, de) in deleted.iter().enumerate() {
        if de.hash.is_none() {
            continue;
        }
        for (ai, ae) in added.iter().enumerate() {
            if used_added[ai] {
                continue;
            }
            if de.hash == ae.hash && de.mode == ae.mode && de.kind == ae.kind {
                renamed.push((de, ae));
                used_added[ai] = true;
                used_deleted[di] = true;
                break;
            }
        }
    }

    let mut changes = Vec::new();
    for (i, ae) in added.iter().enumerate() {
        if used_added[i] {
            continue;
        }
        changes.push(Change {
            kind: ChangeKind::Added,
            path: ae.path.clone(),
            from_path: None,
            old_hash: None,
            new_hash: ae.hash.clone(),
            size_delta: ae.size.map(|s| s as i64),
            content_preview: None,
        });
    }
    for (i, de) in deleted.iter().enumerate() {
        if used_deleted[i] {
            continue;
        }
        changes.push(Change {
            kind: ChangeKind::Deleted,
            path: de.path.clone(),
            from_path: None,
            old_hash: de.hash.clone(),
            new_hash: None,
            size_delta: de.size.map(|s| -(s as i64)),
            content_preview: None,
        });
    }
    for (oe, ne) in &modified {
        let sd = match (oe.size, ne.size) {
            (Some(o), Some(n)) => Some(n as i64 - o as i64),
            _ => None,
        };
        changes.push(Change {
            kind: ChangeKind::Modified,
            path: ne.path.clone(),
            from_path: None,
            old_hash: oe.hash.clone(),
            new_hash: ne.hash.clone(),
            size_delta: sd,
            content_preview: None,
        });
    }
    for (de, ae) in &renamed {
        changes.push(Change {
            kind: ChangeKind::Renamed,
            path: ae.path.clone(),
            from_path: Some(de.path.clone()),
            old_hash: de.hash.clone(),
            new_hash: ae.hash.clone(),
            size_delta: None,
            content_preview: None,
        });
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));

    let counts = |k: &ChangeKind| changes.iter().filter(|c| &c.kind == k).count();
    DiffReport {
        added: counts(&ChangeKind::Added),
        modified: counts(&ChangeKind::Modified),
        deleted: counts(&ChangeKind::Deleted),
        renamed: counts(&ChangeKind::Renamed),
        changes,
    }
}

/// Produce a unified diff between two text blobs for display.
pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(old, new);
    let mut out = format!("--- {path}\n+++ {path}\n");
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        out.push_str(sign);
        out.push_str(change.value().trim_end_matches('\n'));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Entry, EntryKind, Manifest};

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
    fn man(entries: Vec<Entry>) -> Manifest {
        Manifest::build("/r".into(), "t".into(), entries)
    }

    #[test]
    fn detects_added_and_deleted() {
        let old = man(vec![file("a", "h1")]);
        let new = man(vec![file("a", "h1"), file("b", "h2")]);
        let d = diff(&old, &new);
        assert_eq!(d.added, 1);
        assert_eq!(d.deleted, 0);
        let d2 = diff(&new, &old);
        assert_eq!(d2.deleted, 1);
    }

    #[test]
    fn detects_modified() {
        let old = man(vec![file("a", "h1")]);
        let new = man(vec![file("a", "h2")]);
        let d = diff(&old, &new);
        assert_eq!(d.modified, 1);
        assert_eq!(d.changes[0].old_hash.as_deref(), Some("h1"));
        assert_eq!(d.changes[0].new_hash.as_deref(), Some("h2"));
    }

    #[test]
    fn detects_rename_not_add_delete() {
        let old = man(vec![file("old_name", "samehash")]);
        let new = man(vec![file("new_name", "samehash")]);
        let d = diff(&old, &new);
        assert_eq!(d.renamed, 1, "identical content at new path is a rename");
        assert_eq!(d.added, 0);
        assert_eq!(d.deleted, 0);
        assert_eq!(d.changes[0].from_path.as_deref(), Some("old_name"));
    }

    #[test]
    fn no_changes_when_identical() {
        let old = man(vec![file("a", "h1")]);
        let new = man(vec![file("a", "h1")]);
        assert!(diff(&old, &new).is_empty());
    }

    #[test]
    fn mode_only_change_is_modified() {
        let mut e = file("a", "h1");
        let old = man(vec![e.clone()]);
        e.mode = Some(0o755);
        let new = man(vec![e]);
        assert_eq!(diff(&old, &new).modified, 1);
    }

    #[test]
    fn unified_diff_marks_changed_lines() {
        let d = unified_diff("a\nb\nc\n", "a\nB\nc\n", "f.txt");
        assert!(d.contains("-b"));
        assert!(d.contains("+B"));
    }
}
