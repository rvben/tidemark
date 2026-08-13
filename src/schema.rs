//! The clispec v0.3 schema document for tidemark.

use serde_json::{Value, json};

/// Build the clispec v0.3 contract describing tidemark's commands, effects,
/// cardinality, arguments, output, and errors.
pub fn schema() -> Value {
    json!({
        "clispec": "0.3",
        "name": "tidemark",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Snapshot a directory tree and diff what changed - no git required.",
        "output": {"tty": "text", "piped": "json"},
        "global_args": [
            {
                "name": "--output",
                "short": "-o",
                "type": "string",
                "enum": ["auto", "json", "table", "text"],
                "default": "auto",
                "description": "Output format. auto detects the TTY; an explicit value always wins."
            },
            {
                "name": "--json",
                "type": "boolean",
                "default": false,
                "description": "Shorthand for --output json."
            },
            {
                "name": "--quiet",
                "type": "boolean",
                "default": false,
                "description": "Suppress diagnostics on stderr."
            },
            {
                "name": "--yes",
                "type": "boolean",
                "default": false,
                "description": "Assume yes for destructive prompts (required to delete non-interactively)."
            },
            {
                "name": "--limit",
                "type": "integer",
                "required": false,
                "description": "Limit the number of items in list/diff output."
            },
            {
                "name": "--offset",
                "type": "integer",
                "default": 0,
                "description": "Skip this many items in list/diff output."
            },
            {
                "name": "--fields",
                "type": "string[]",
                "required": false,
                "description": "Restrict output objects to these fields (comma-separated)."
            }
        ],
        "errors": [
            {"kind": "not_found", "exit_code": 4, "retryable": false, "description": "A label, manifest file, or path does not exist."},
            {"kind": "conflict", "exit_code": 5, "retryable": false, "description": "A label exists with a different tree (use --force to overwrite)."},
            {"kind": "invalid_input", "exit_code": 2, "retryable": false, "description": "A ref, glob, label, or manifest was malformed."},
            {"kind": "io", "exit_code": 3, "retryable": true, "description": "A filesystem operation failed; retrying may succeed."},
            {"kind": "unsupported", "exit_code": 6, "retryable": false, "description": "An input could not be represented (e.g. a non-UTF-8 path)."}
        ],
        "exit_codes": {
            "default": {"0": "success", "2": "error"},
            "with_exit_code_flag": {"0": "no changes", "1": "changes found", "2": "error"}
        },
        "commands": [
            {
                "name": "snap",
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "single",
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
                    {"name": "label", "type": "string", "description": "Stored label; omitted when the manifest was only written to a file or stdout."},
                    {"name": "tree_digest", "type": "string"},
                    {"name": "entry_count", "type": "integer"},
                    {"name": "created", "type": "boolean"}
                ]
            },
            {
                "name": "list",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "unbounded",
                "pagination": {"style": "offset", "limit_arg": "--limit", "offset_arg": "--offset"},
                "fields_arg": "--fields",
                "stability": "stable",
                "description": "List stored snapshots.",
                "args": [
                    {"name": "--limit", "type": "integer", "required": false},
                    {"name": "--offset", "type": "integer", "required": false},
                    {"name": "--fields", "type": "string[]", "required": false}
                ],
                "output_fields": [
                    {"name": "items", "type": "array", "items": {"type": "object"}},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer", "description": "Requested limit; omitted when no limit was supplied."},
                    {"name": "offset", "type": "integer"}
                ]
            },
            {
                "name": "diff",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "unbounded",
                "pagination": {"style": "offset", "limit_arg": "--limit", "offset_arg": "--offset"},
                "fields_arg": "--fields",
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
                    {"name": "changes", "type": "array", "items": {"type": "object"}},
                    {"name": "added", "type": "integer"},
                    {"name": "modified", "type": "integer"},
                    {"name": "deleted", "type": "integer"},
                    {"name": "renamed", "type": "integer"},
                    {"name": "total", "type": "integer"},
                    {"name": "limit", "type": "integer", "description": "Requested limit; omitted when no limit was supplied."},
                    {"name": "offset", "type": "integer"}
                ],
                "example": {"args": ["diff", "@", "@"]}
            },
            {
                "name": "show",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "single",
                "stability": "stable",
                "description": "Show a manifest by ref.",
                "args": [{"name": "reference", "type": "string", "required": true}],
                "output_fields": [{"name": "manifest", "type": "object"}]
            },
            {
                "name": "rm",
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "bounded",
                "stability": "stable",
                "description": "Remove stored snapshots.",
                "args": [
                    {"name": "labels", "type": "string[]", "required": true},
                    {"name": "--yes", "type": "boolean", "required": false}
                ],
                "output_fields": [{"name": "removed", "type": "array", "items": {"type": "string"}}]
            },
            {
                "name": "init",
                "effects": "idempotent",
                "mutating": true,
                "cardinality": "single",
                "idempotency": {"key_args": ["--path"], "conflict_error": "conflict"},
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
                "effects": "read_only",
                "mutating": false,
                "cardinality": "single",
                "stability": "stable",
                "description": "Emit this schema.",
                "args": [],
                "stdout_schema": {"$ref": "https://clispec.dev/schema/v0.3.json"}
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
        assert_eq!(s["clispec"], "0.3");
        assert_eq!(s["name"], "tidemark");
        assert!(s["version"].is_string());
        assert!(s["commands"].as_array().unwrap().len() >= 6);
    }

    #[test]
    fn schema_declares_global_args() {
        let s = schema();
        let global_args = s["global_args"]
            .as_array()
            .expect("global_args must be present");
        assert!(!global_args.is_empty(), "global_args must not be empty");
        // --output must be declared with auto default
        let output_arg = global_args
            .iter()
            .find(|a| a["name"] == "--output")
            .expect("--output must be in global_args");
        assert_eq!(
            output_arg["default"], "auto",
            "--output default must be auto"
        );
        let enums = output_arg["enum"]
            .as_array()
            .expect("--output must have enum");
        assert!(enums.iter().any(|e| e == "auto"), "enum must include auto");
        assert!(enums.iter().any(|e| e == "json"), "enum must include json");
        assert!(enums.iter().any(|e| e == "text"), "enum must include text");
    }

    #[test]
    fn error_kinds_have_exit_code() {
        let s = schema();
        let errors = s["errors"].as_array().expect("top-level errors array");
        for e in errors {
            let kind = e["kind"].as_str().unwrap();
            assert!(
                e["exit_code"].is_number(),
                "error kind {kind:?} missing exit_code"
            );
            let code = e["exit_code"].as_u64().unwrap();
            assert!(
                (1..=255).contains(&code),
                "exit_code for {kind:?} must be 1-255"
            );
        }
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
    fn commands_declare_effects_and_output_contracts() {
        let s = schema();
        for cmd in s["commands"].as_array().unwrap() {
            assert!(
                cmd["mutating"].is_boolean(),
                "command {} missing mutating marker",
                cmd["name"]
            );
            assert!(
                cmd["effects"].is_string(),
                "command {} missing effects",
                cmd["name"]
            );
            assert!(
                cmd.get("output_fields").is_some() || cmd.get("stdout_schema").is_some(),
                "command {} missing an output contract",
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
