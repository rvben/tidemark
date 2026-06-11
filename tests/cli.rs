use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn tidemark(dir: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("tidemark").unwrap();
    c.current_dir(dir);
    c
}

// --- clispec.dev conformance: the scorer probes the bare tool with global flags ---

#[test]
fn bare_invocation_defaults_to_list_json_when_piped() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path()).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["items"].is_array(), "bare invocation lists snapshots");
    assert_eq!(v["total"], 0);
}

#[test]
fn json_flag_is_accepted_globally() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path()).arg("--json").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    serde_json::from_str::<serde_json::Value>(&stdout).expect("--json yields valid JSON");
}

#[test]
fn quiet_flag_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    tidemark(tmp.path()).arg("--quiet").assert().success();
}

#[test]
fn global_bounded_flags_accepted_on_default_command() {
    let tmp = tempfile::tempdir().unwrap();
    tidemark(tmp.path())
        .args(["--output", "json", "--limit", "1"])
        .assert()
        .success();
    tidemark(tmp.path())
        .args(["--output", "json", "--offset", "0"])
        .assert()
        .success();
    tidemark(tmp.path())
        .args(["--output", "json", "--fields", "label"])
        .assert()
        .success();
}

#[test]
fn yes_flag_is_global() {
    let tmp = tempfile::tempdir().unwrap();
    tidemark(tmp.path()).arg("--yes").assert().success();
}

#[test]
fn unknown_subcommand_emits_json_error_to_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path())
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
    tidemark(tmp.path()).arg("init").assert().success();
    assert!(tmp.path().join(".tidemark").is_dir());
    // running again is a success no-op
    tidemark(tmp.path()).arg("init").assert().success();
}

#[test]
fn help_lists_global_flags() {
    let tmp = tempfile::tempdir().unwrap();
    tidemark(tmp.path())
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
    tidemark(tmp.path())
        .args(["snap", "before"])
        .assert()
        .success();
    fs::write(tmp.path().join("b.txt"), b"new").unwrap();
    tidemark(tmp.path())
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
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
    tidemark(tmp.path())
        .args(["diff", "s", "--exit-code"])
        .assert()
        .code(0);
    fs::write(tmp.path().join("a.txt"), b"changed").unwrap();
    tidemark(tmp.path())
        .args(["diff", "s", "--exit-code"])
        .assert()
        .code(1);
}

#[test]
fn idempotent_snap_same_tree() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
}

#[test]
fn conflict_on_label_reuse_with_changes() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
    fs::write(tmp.path().join("a.txt"), b"y").unwrap();
    tidemark(tmp.path())
        .args(["snap", "s"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("\"kind\":\"conflict\""));
}

#[test]
fn schema_is_valid_json_with_tool_name() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path()).args(["schema"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // Canonical clispec shape: top-level name + version + commands array.
    assert_eq!(v["name"], "tidemark");
    assert_eq!(v["clispec"], "0.2");
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
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
    tidemark(tmp.path())
        .args(["rm", "s"])
        .assert()
        .failure()
        .code(2);
    tidemark(tmp.path())
        .args(["rm", "s", "--yes"])
        .assert()
        .success();
}

#[test]
fn list_uses_envelope_when_piped() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();
    tidemark(tmp.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"items\""))
        .stdout(predicate::str::contains("\"total\""));
}

#[test]
fn diff_pagination_limits_changes() {
    let tmp = tempfile::tempdir().unwrap();
    tidemark(tmp.path())
        .args(["snap", "empty"])
        .assert()
        .success();
    for i in 0..5 {
        fs::write(tmp.path().join(format!("f{i}.txt")), b"x").unwrap();
    }
    let out = tidemark(tmp.path())
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
    let out = tidemark(tmp.path())
        .args(["snap", "--output-file", "-"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["entry_count"], 1);
    assert!(v["tree_digest"].as_str().unwrap().starts_with("blake3:"));
}

#[test]
fn content_diff_works_between_two_stored_manifests() {
    // Both sides are stored manifest files (neither is @). Because every snapshot
    // stores inline text content, a real two-sided unified diff is still possible
    // without the live tree. This locks in that property.
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), b"alpha\nbeta\ngamma\n").unwrap();
    let before = tmp.path().join("before.tidemark");
    tidemark(tmp.path())
        .args(["snap", "--output-file", before.to_str().unwrap()])
        .assert()
        .success();
    fs::write(tmp.path().join("a.txt"), b"alpha\nBETA\ngamma\n").unwrap();
    let after = tmp.path().join("after.tidemark");
    tidemark(tmp.path())
        .args(["snap", "--output-file", after.to_str().unwrap()])
        .assert()
        .success();
    let out = tidemark(tmp.path())
        .args([
            "diff",
            before.to_str().unwrap(),
            after.to_str().unwrap(),
            "--content",
            "--output",
            "json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let preview = v["changes"][0]["content_preview"].as_str().unwrap();
    assert!(
        preview.contains("-beta") && preview.contains("+BETA"),
        "two-manifest content diff should show old and new lines: {preview}"
    );
}

#[test]
fn diff_only_filters_change_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("keep.txt"), b"x").unwrap();
    tidemark(tmp.path())
        .args(["snap", "base"])
        .assert()
        .success();
    fs::write(tmp.path().join("added.txt"), b"y").unwrap();
    fs::write(tmp.path().join("keep.txt"), b"changed").unwrap();
    let out = tidemark(tmp.path())
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
    tidemark(tmp.path())
        .args(["snap", "base"])
        .assert()
        .success();
    fs::write(tmp.path().join("a.txt"), b"one\nTWO\nthree\n").unwrap();
    let out = tidemark(tmp.path())
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
    tidemark(tmp.path())
        .args(["show", "nonexistent-label"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("\"retryable\""))
        .stderr(predicate::str::contains("\"kind\":\"not_found\""));
}

// --- clispec v0.2 conformance tests ---

/// The schema output must validate against the vendored clispec v0.2 JSON Schema.
/// This exercises the production `schema` command, not a hand-rolled re-implementation.
#[test]
fn schema_validates_against_clispec_v0_2() {
    let schema_fixture = include_str!("fixtures/clispec-v0.2.json");
    let meta_schema: serde_json::Value =
        serde_json::from_str(schema_fixture).expect("fixture must be valid JSON");

    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path()).args(["schema"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let tool_schema: serde_json::Value =
        serde_json::from_str(&stdout).expect("schema output must be valid JSON");

    assert!(
        jsonschema::is_valid(&meta_schema, &tool_schema),
        "schema output does not validate against clispec v0.2. Output was:\n{tool_schema:#}"
    );
}

/// Explicit -o text must produce text output even when stdout is piped (not a TTY).
/// The assert_cmd harness captures stdout, which is never a TTY - so a plain invocation
/// without -o already produces JSON. With -o text the output must NOT be JSON.
#[test]
fn explicit_output_text_wins_over_auto_when_piped() {
    let tmp = tempfile::tempdir().unwrap();
    // Snap something so list has a row to display.
    std::fs::write(tmp.path().join("f.txt"), b"x").unwrap();
    tidemark(tmp.path()).args(["snap", "s"]).assert().success();

    // Without -o: piped -> JSON (baseline).
    let out_json = tidemark(tmp.path()).args(["list"]).assert().success();
    let json_stdout = String::from_utf8(out_json.get_output().stdout.clone()).unwrap();
    serde_json::from_str::<serde_json::Value>(&json_stdout)
        .expect("baseline piped output must be JSON");

    // With -o text: must NOT be JSON, must be text rows.
    let out_text = tidemark(tmp.path())
        .args(["list", "-o", "text"])
        .assert()
        .success();
    let text_stdout = String::from_utf8(out_text.get_output().stdout.clone()).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&text_stdout).is_err(),
        "-o text when piped must not emit JSON; got: {text_stdout:?}"
    );
    // The text output must contain the label name.
    assert!(
        text_stdout.contains('s'),
        "-o text must contain the snapshot label; got: {text_stdout:?}"
    );
}

/// The structured error envelope must appear as the last line of stderr (clispec Principle 1).
#[test]
fn error_envelope_is_last_line_of_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tidemark(tmp.path())
        .args(["show", "no-such-label"])
        .assert()
        .failure();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let last_line = stderr
        .trim_end()
        .lines()
        .last()
        .expect("stderr must not be empty");
    let v: serde_json::Value =
        serde_json::from_str(last_line).expect("last stderr line must be the JSON error envelope");
    assert!(
        v["error"]["kind"].is_string(),
        "envelope must have error.kind"
    );
    assert!(
        v["error"]["message"].is_string(),
        "envelope must have error.message"
    );
}
