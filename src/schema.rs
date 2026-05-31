//! The clispec v0.1 schema document for kairn.

use serde_json::{Value, json};

/// Build the clispec schema describing kairn's commands, args, output, and errors.
pub fn schema() -> Value {
    json!({
        "clispec_version": "0.1",
        "tool": "kairn",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Snapshot a directory tree and diff what changed - no git required.",
        "error_kinds": [
            {"kind": "not_found", "retryable": false},
            {"kind": "conflict", "retryable": false},
            {"kind": "invalid_input", "retryable": false},
            {"kind": "io", "retryable": true},
            {"kind": "unsupported", "retryable": false}
        ],
        "exit_codes": {
            "default": {"0": "success", "2": "error"},
            "with_exit_code_flag": {"0": "no changes", "1": "changes found", "2": "error"}
        },
        "commands": [
            {
                "name": "snap",
                "mutates": true,
                "summary": "Snapshot a directory into a manifest (store label and/or -o file).",
                "args": [
                    {"name": "label", "type": "string", "required": false},
                    {"name": "--path", "type": "string", "required": false, "default": "."},
                    {"name": "--output-file", "type": "path", "required": false, "note": "- for stdout"},
                    {"name": "--ignore", "type": "glob[]", "required": false},
                    {"name": "--hidden", "type": "bool", "required": false},
                    {"name": "--no-ignore", "type": "bool", "required": false},
                    {"name": "--force", "type": "bool", "required": false}
                ],
                "output_fields": [
                    {"name": "label", "type": "string|null"},
                    {"name": "tree_digest", "type": "string"},
                    {"name": "entry_count", "type": "integer"},
                    {"name": "created", "type": "boolean"}
                ]
            },
            {
                "name": "diff",
                "mutates": false,
                "summary": "Diff two refs (label | manifest file | @ current tree).",
                "args": [
                    {"name": "a", "type": "ref", "required": false},
                    {"name": "b", "type": "ref", "required": false, "default": "@"},
                    {"name": "--content", "type": "bool", "required": false},
                    {"name": "--only", "type": "enum[]", "values": ["added","modified","deleted","renamed"]},
                    {"name": "--limit", "type": "integer", "required": false},
                    {"name": "--offset", "type": "integer", "required": false},
                    {"name": "--fields", "type": "string[]", "required": false},
                    {"name": "--exit-code", "type": "bool", "required": false}
                ],
                "output_fields": [
                    {"name": "changes", "type": "Change[]"},
                    {"name": "added", "type": "integer"},
                    {"name": "modified", "type": "integer"},
                    {"name": "deleted", "type": "integer"},
                    {"name": "renamed", "type": "integer"},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer|null"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "list",
                "mutates": false,
                "summary": "List stored snapshots.",
                "args": [
                    {"name": "--limit", "type": "integer", "required": false},
                    {"name": "--offset", "type": "integer", "required": false},
                    {"name": "--fields", "type": "string[]", "required": false}
                ],
                "output_fields": [
                    {"name": "items", "type": "StoreItem[]"},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer|null"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "show",
                "mutates": false,
                "summary": "Show a manifest (by ref).",
                "args": [{"name": "reference", "type": "ref", "required": true}],
                "output_fields": [{"name": "manifest", "type": "Manifest"}]
            },
            {
                "name": "rm",
                "mutates": true,
                "summary": "Remove stored snapshots.",
                "args": [
                    {"name": "labels", "type": "string[]", "required": true},
                    {"name": "--yes", "type": "bool", "required": false}
                ],
                "output_fields": [{"name": "removed", "type": "string[]"}]
            },
            {
                "name": "schema",
                "mutates": false,
                "summary": "Emit this schema.",
                "args": [],
                "output_fields": []
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_required_top_level_keys() {
        let s = schema();
        assert_eq!(s["clispec_version"], "0.1");
        assert_eq!(s["tool"], "kairn");
        assert!(s["commands"].as_array().unwrap().len() >= 6);
        assert!(
            s["error_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["kind"] == "conflict")
        );
    }

    #[test]
    fn every_error_kind_has_retryable() {
        for k in schema()["error_kinds"].as_array().unwrap() {
            assert!(k.get("retryable").is_some(), "kind {k:?} missing retryable");
        }
    }
}
