//! The clispec v0.1 schema document for kairn.

use serde_json::{Value, json};

/// Build the clispec v0.1 schema describing kairn's commands, args, output, and
/// errors. The shape matches `clispec.dev/schema/v0.1.json`: top-level `name`,
/// `version`, `commands`, and `errors`; each command carries a `mutating` marker
/// and `output_fields`.
pub fn schema() -> Value {
    json!({
        "clispec": "0.1",
        "name": "kairn",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Snapshot a directory tree and diff what changed - no git required.",
        "errors": [
            {"kind": "not_found", "retryable": false, "description": "A label, manifest file, or path does not exist."},
            {"kind": "conflict", "retryable": false, "description": "A label exists with a different tree (use --force to overwrite)."},
            {"kind": "invalid_input", "retryable": false, "description": "A ref, glob, label, or manifest was malformed."},
            {"kind": "io", "retryable": true, "description": "A filesystem operation failed; retrying may succeed."},
            {"kind": "unsupported", "retryable": false, "description": "An input could not be represented (e.g. a non-UTF-8 path)."}
        ],
        "exit_codes": {
            "default": {"0": "success", "2": "error"},
            "with_exit_code_flag": {"0": "no changes", "1": "changes found", "2": "error"}
        },
        "commands": [
            {
                "name": "snap",
                "mutating": true,
                "stability": "stable",
                "description": "Snapshot a directory into a manifest (store label and/or -o file).",
                "args": [
                    {"name": "label", "type": "string", "required": false},
                    {"name": "--path", "type": "path", "required": false, "default": "."},
                    {"name": "--output-file", "type": "path", "required": false, "description": "- writes the manifest to stdout"},
                    {"name": "--ignore", "type": "string[]", "required": false},
                    {"name": "--hidden", "type": "boolean", "required": false},
                    {"name": "--no-ignore", "type": "boolean", "required": false},
                    {"name": "--no-content", "type": "boolean", "required": false},
                    {"name": "--force", "type": "boolean", "required": false}
                ],
                "output_fields": [
                    {"name": "label", "type": "string | null"},
                    {"name": "tree_digest", "type": "string"},
                    {"name": "entry_count", "type": "integer"},
                    {"name": "created", "type": "boolean"}
                ]
            },
            {
                "name": "list",
                "mutating": false,
                "stability": "stable",
                "description": "List stored snapshots.",
                "args": [
                    {"name": "--limit", "type": "integer", "required": false},
                    {"name": "--offset", "type": "integer", "required": false},
                    {"name": "--fields", "type": "string[]", "required": false}
                ],
                "output_fields": [
                    {"name": "items", "type": "StoreItem[]"},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer | null"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "diff",
                "mutating": false,
                "stability": "stable",
                "description": "Diff two refs (label | manifest file | @ current tree).",
                "args": [
                    {"name": "a", "type": "string", "required": false},
                    {"name": "b", "type": "string", "required": false, "default": "@"},
                    {"name": "--content", "type": "boolean", "required": false},
                    {"name": "--only", "type": "string[]", "required": false, "enum": ["added", "modified", "deleted", "renamed"]},
                    {"name": "--limit", "type": "integer", "required": false},
                    {"name": "--offset", "type": "integer", "required": false},
                    {"name": "--fields", "type": "string[]", "required": false},
                    {"name": "--exit-code", "type": "boolean", "required": false}
                ],
                "output_fields": [
                    {"name": "changes", "type": "Change[]"},
                    {"name": "added", "type": "integer"},
                    {"name": "modified", "type": "integer"},
                    {"name": "deleted", "type": "integer"},
                    {"name": "renamed", "type": "integer"},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer | null"},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "show",
                "mutating": false,
                "stability": "stable",
                "description": "Show a manifest by ref.",
                "args": [{"name": "reference", "type": "string", "required": true}],
                "output_fields": [{"name": "manifest", "type": "Manifest"}]
            },
            {
                "name": "rm",
                "mutating": true,
                "stability": "stable",
                "description": "Remove stored snapshots.",
                "args": [
                    {"name": "labels", "type": "string[]", "required": true},
                    {"name": "--yes", "type": "boolean", "required": false}
                ],
                "output_fields": [{"name": "removed", "type": "string[]"}]
            },
            {
                "name": "init",
                "mutating": true,
                "stability": "stable",
                "description": "Create the snapshot store in the current directory (idempotent).",
                "args": [{"name": "--path", "type": "path", "required": false, "default": "."}],
                "output_fields": [
                    {"name": "initialized", "type": "boolean"},
                    {"name": "path", "type": "string"}
                ]
            },
            {
                "name": "schema",
                "mutating": false,
                "stability": "stable",
                "description": "Emit this schema.",
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
        assert_eq!(s["clispec"], "0.1");
        assert_eq!(s["name"], "kairn");
        assert!(s["version"].is_string());
        assert!(s["commands"].as_array().unwrap().len() >= 6);
    }

    #[test]
    fn errors_array_has_conflict_with_retryable() {
        let s = schema();
        let errors = s["errors"].as_array().expect("top-level errors array");
        // Every error entry carries kind + retryable.
        for e in errors {
            assert!(e["kind"].is_string(), "error missing kind: {e:?}");
            assert!(
                e["retryable"].is_boolean(),
                "error missing retryable: {e:?}"
            );
        }
        // The conflict kind must be declared (clispec idempotency requirement).
        assert!(
            errors.iter().any(|e| e["kind"] == "conflict"),
            "schema must declare the conflict error kind"
        );
    }

    #[test]
    fn error_kinds_are_snake_case() {
        let s = schema();
        for e in s["errors"].as_array().unwrap() {
            let kind = e["kind"].as_str().unwrap();
            assert!(
                kind.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "error kind {kind:?} must be snake_case"
            );
        }
    }

    #[test]
    fn commands_declare_mutating_and_output_fields() {
        let s = schema();
        for cmd in s["commands"].as_array().unwrap() {
            assert!(
                cmd["mutating"].is_boolean(),
                "command {} missing mutating marker",
                cmd["name"]
            );
            assert!(
                cmd.get("output_fields").is_some(),
                "command {} missing output_fields",
                cmd["name"]
            );
        }
        // At least one mutating and one non-mutating command exist.
        let muts: Vec<bool> = s["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["mutating"].as_bool().unwrap())
            .collect();
        assert!(muts.iter().any(|&m| m), "expected a mutating command");
        assert!(muts.iter().any(|&m| !m), "expected a non-mutating command");
    }
}
