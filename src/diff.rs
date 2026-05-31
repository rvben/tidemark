//! Pure diff over two manifests.

use crate::manifest::{Entry, EntryKind, Manifest};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

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

    /// Recompute the summary counts from `self.changes`. Call after mutating
    /// `changes` (e.g. applying an `--only` filter) so the counts always match
    /// the change list.
    pub fn recount(&mut self) {
        let count = |k: &ChangeKind| self.changes.iter().filter(|c| &c.kind == k).count();
        self.added = count(&ChangeKind::Added);
        self.modified = count(&ChangeKind::Modified);
        self.deleted = count(&ChangeKind::Deleted);
        self.renamed = count(&ChangeKind::Renamed);
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

    // Rename detection: pair a deleted entry with an added entry only when their
    // (hash, mode, kind) signature is unambiguous - exactly one deleted and one
    // added entry share it. Duplicate-content files (e.g. several empty files all
    // sharing the empty-input hash) stay as plain add/delete rather than being
    // cross-paired into misleading renames.
    type Sig<'a> = (&'a str, Option<u32>, EntryKind);
    let mut del_by_sig: HashMap<Sig, Vec<usize>> = HashMap::new();
    let mut add_by_sig: HashMap<Sig, Vec<usize>> = HashMap::new();
    for (i, de) in deleted.iter().enumerate() {
        if let Some(h) = de.hash.as_deref() {
            del_by_sig.entry((h, de.mode, de.kind)).or_default().push(i);
        }
    }
    for (i, ae) in added.iter().enumerate() {
        if let Some(h) = ae.hash.as_deref() {
            add_by_sig.entry((h, ae.mode, ae.kind)).or_default().push(i);
        }
    }

    let mut renamed: Vec<(&Entry, &Entry)> = Vec::new();
    let mut used_added = vec![false; added.len()];
    let mut used_deleted = vec![false; deleted.len()];
    for (sig, dels) in &del_by_sig {
        if dels.len() != 1 {
            continue;
        }
        if let Some(adds) = add_by_sig.get(sig) {
            if adds.len() != 1 {
                continue;
            }
            let di = dels[0];
            let ai = adds[0];
            renamed.push((deleted[di], added[ai]));
            used_deleted[di] = true;
            used_added[ai] = true;
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

    let mut report = DiffReport {
        changes,
        added: 0,
        modified: 0,
        deleted: 0,
        renamed: 0,
    };
    report.recount();
    report
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
            content: None,
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
    fn ambiguous_identical_content_is_not_a_rename() {
        // Two deleted + two added files all sharing one hash (e.g. empty files):
        // the pairing is ambiguous, so they must be reported as plain add/delete,
        // never as misleading renames.
        let old = man(vec![file("d1", "EMPTY"), file("d2", "EMPTY")]);
        let new = man(vec![file("a1", "EMPTY"), file("a2", "EMPTY")]);
        let d = diff(&old, &new);
        assert_eq!(
            d.renamed, 0,
            "ambiguous identical-content files must not be renames"
        );
        assert_eq!(d.deleted, 2);
        assert_eq!(d.added, 2);
    }

    #[test]
    fn one_to_many_identical_content_is_not_a_rename() {
        // One deleted, two added with the same hash: still ambiguous.
        let old = man(vec![file("d1", "DUP")]);
        let new = man(vec![file("a1", "DUP"), file("a2", "DUP")]);
        let d = diff(&old, &new);
        assert_eq!(d.renamed, 0);
        assert_eq!(d.deleted, 1);
        assert_eq!(d.added, 2);
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
