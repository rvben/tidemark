use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn kairn(dir: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("kairn").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn snap_then_diff_reports_added_file() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
    kairn(tmp.path())
        .args(["snap", "before"])
        .assert()
        .success();
    fs::write(tmp.path().join("b.txt"), b"new").unwrap();
    kairn(tmp.path())
        .args(["diff", "before", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"added\": 1"))
        .stdout(predicate::str::contains("b.txt"));
}

#[test]
fn exit_code_flag_signals_changes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
    kairn(tmp.path())
        .args(["diff", "s", "--exit-code"])
        .assert()
        .code(0);
    fs::write(tmp.path().join("a.txt"), b"changed").unwrap();
    kairn(tmp.path())
        .args(["diff", "s", "--exit-code"])
        .assert()
        .code(1);
}

#[test]
fn idempotent_snap_same_tree() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
}

#[test]
fn conflict_on_label_reuse_with_changes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
    fs::write(tmp.path().join("a.txt"), b"y").unwrap();
    kairn(tmp.path())
        .args(["snap", "s"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("\"kind\":\"conflict\""));
}

#[test]
fn schema_is_valid_json_with_tool_name() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path())
        .args(["schema"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tool\": \"kairn\""));
}

#[test]
fn rm_requires_yes_when_noninteractive() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
    kairn(tmp.path())
        .args(["rm", "s"])
        .assert()
        .failure()
        .code(2);
    kairn(tmp.path())
        .args(["rm", "s", "--yes"])
        .assert()
        .success();
}

#[test]
fn list_uses_envelope_when_piped() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "s"]).assert().success();
    kairn(tmp.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"items\""))
        .stdout(predicate::str::contains("\"total\""));
}

#[test]
fn diff_pagination_limits_changes() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path()).args(["snap", "empty"]).assert().success();
    for i in 0..5 {
        fs::write(tmp.path().join(format!("f{i}.txt")), b"x").unwrap();
    }
    let out = kairn(tmp.path())
        .args(["diff", "empty", "--limit", "2", "--output", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["changes"].as_array().unwrap().len(), 2);
    assert_eq!(v["total"], 5);
}

#[test]
fn snap_to_stdout_when_output_dash() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let out = kairn(tmp.path())
        .args(["snap", "--output-file", "-"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["entry_count"], 1);
    assert!(v["tree_digest"].as_str().unwrap().starts_with("blake3:"));
}

#[test]
fn diff_only_filters_change_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("keep.txt"), b"x").unwrap();
    kairn(tmp.path()).args(["snap", "base"]).assert().success();
    fs::write(tmp.path().join("added.txt"), b"y").unwrap();
    fs::write(tmp.path().join("keep.txt"), b"changed").unwrap();
    let out = kairn(tmp.path())
        .args(["diff", "base", "--only", "added", "--output", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changes = v["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["kind"], "added");
}

#[test]
fn content_diff_shows_unified_lines() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"one\ntwo\nthree\n").unwrap();
    kairn(tmp.path()).args(["snap", "base"]).assert().success();
    fs::write(tmp.path().join("a.txt"), b"one\nTWO\nthree\n").unwrap();
    let out = kairn(tmp.path())
        .args(["diff", "base", "--content", "--output", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let preview = v["changes"][0]["content_preview"].as_str().unwrap();
    assert!(
        preview.contains("-two"),
        "preview should show removed line: {preview}"
    );
    assert!(
        preview.contains("+TWO"),
        "preview should show added line: {preview}"
    );
}

#[test]
fn error_output_has_retryable_field() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path())
        .args(["show", "nonexistent-label"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("\"retryable\""))
        .stderr(predicate::str::contains("\"kind\":\"not_found\""));
}
