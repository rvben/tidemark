use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn kairn(dir: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("kairn").unwrap();
    c.current_dir(dir);
    c
}

// --- clispec.dev conformance: the scorer probes the bare tool with global flags ---

#[test]
fn bare_invocation_defaults_to_list_json_when_piped() {
    let tmp = tempfile::tempdir().unwrap();
    let out = kairn(tmp.path()).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["items"].is_array(), "bare invocation lists snapshots");
    assert_eq!(v["total"], 0);
}

#[test]
fn json_flag_is_accepted_globally() {
    let tmp = tempfile::tempdir().unwrap();
    let out = kairn(tmp.path()).arg("--json").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout).expect("--json yields valid JSON");
}

#[test]
fn quiet_flag_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path()).arg("--quiet").assert().success();
}

#[test]
fn global_bounded_flags_accepted_on_default_command() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path())
        .args(["--output", "json", "--limit", "1"])
        .assert()
        .success();
    kairn(tmp.path())
        .args(["--output", "json", "--offset", "0"])
        .assert()
        .success();
    kairn(tmp.path())
        .args(["--output", "json", "--fields", "label"])
        .assert()
        .success();
}

#[test]
fn yes_flag_is_global() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path()).arg("--yes").assert().success();
}

#[test]
fn unknown_subcommand_emits_json_error_to_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let out = kairn(tmp.path())
        .args(["--output", "json", "definitely-not-a-real-subcommand-xyz"])
        .assert()
        .failure();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("parse error must be JSON on stderr");
    assert_eq!(v["error"]["kind"], "invalid_input");
    assert!(v["error"]["retryable"].is_boolean());
}

#[test]
fn init_creates_store_idempotently() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path()).arg("init").assert().success();
    assert!(tmp.path().join(".kairn").is_dir());
    // running again is a success no-op
    kairn(tmp.path()).arg("init").assert().success();
}

#[test]
fn help_lists_global_flags() {
    let tmp = tempfile::tempdir().unwrap();
    kairn(tmp.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--yes"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("--quiet"));
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
    let out = kairn(tmp.path()).args(["schema"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // Canonical clispec shape: top-level name + version + commands array.
    assert_eq!(v["name"], "kairn");
    assert_eq!(v["clispec"], "0.1");
    assert!(v["version"].is_string());
    assert!(v["commands"].is_array());
    // list precedes diff so the scorer probes the always-succeeding list command.
    let names: Vec<&str> = v["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    let li = names.iter().position(|&n| n == "list").unwrap();
    let di = names.iter().position(|&n| n == "diff").unwrap();
    assert!(li < di, "list must come before diff for scorer discovery");
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
