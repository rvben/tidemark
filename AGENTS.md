# tidemark for agents

`tidemark` is a deterministic filesystem snapshot/diff tool designed to be driven
by AI agents. This document is the operational contract.

## Discover capabilities

Run `tidemark schema` first. It returns a clispec v0.1 document listing every
command, its arguments (name, type, required, default), output fields, error
kinds (each with a `retryable` flag), exit-code semantics, and which commands
mutate state (`mutating: true`). The schema is the source of truth; prefer it
over parsing help text.

## Output contract

- Output is **JSON by default when stdout is not a TTY**. You never need a flag,
  but you may pass `--json` (or `--output json`) to be explicit.
- **stdout carries data only.** All diagnostics and the human summary line go to
  stderr. Piping stdout to a JSON parser is always safe.
- `diff` returns a flat object:

  ```json
  {
    "changes": [ { "kind": "modified", "path": "src/a.rs",
                   "old_hash": "blake3:...", "new_hash": "blake3:...",
                   "size_delta": 12 } ],
    "added": 0, "modified": 1, "deleted": 0, "renamed": 0,
    "total": 1, "limit": null, "offset": 0
  }
  ```

- `list` uses the envelope `{"items": [...], "total": N, "limit": L, "offset": O}`.
- Bound large results with `--limit` / `--offset`, and trim fields with
  `--fields path,kind`. These flags are global (accepted on any command).
- Running `tidemark` with no command lists stored snapshots.

## Recommended pattern

```
tidemark snap -o pre.tidemark        # portable manifest (or: tidemark snap before)
<run the operation under test>
tidemark diff pre.tidemark @ --json  # @ means "the current tree"
```

A ref (the `A`/`B` of `diff`) is one of: a store label, a path to a manifest
file, or `@` for the live tree. `tidemark diff before` is shorthand for
`tidemark diff before @`. A bare ref resolves to a store label first, falling
back to a same-named file only if no such label exists. `tidemark init` creates
the `.tidemark/` store explicitly (idempotent).

## Determinism and idempotency

- `tree_digest` is a BLAKE3 Merkle root over the sorted entries. Two snapshots of
  an unchanged tree are byte-for-byte identical in digest.
- `mtime` never affects change detection, so rebuilds and `touch` do not produce
  spurious diffs.
- `tidemark snap LABEL` on an unchanged tree is a success no-op (exit 0).
  Re-using a label for a *different* tree returns the `conflict` error kind
  (exit 2) unless you pass `--force`.
- A file that vanishes between the directory walk and its read is skipped, so a
  concurrent deletion mid-snapshot does not abort the whole snapshot.

## Exit codes

- Default: `0` success, `2` error.
- `diff --exit-code`: `0` no changes, `1` changes found, `2` error. Use this when
  you only need a yes/no "did anything change" signal.

## Errors

Always to stderr, shape:

```json
{"error": {"kind": "not_found", "message": "...", "retryable": false}}
```

Kinds: `not_found`, `conflict`, `invalid_input`, `unsupported`, and `io`
(the only `retryable: true` kind - safe to retry transient filesystem errors).
Argument-parse failures are also reported as `invalid_input` JSON on stderr.

## Content diffs

`diff --content` emits a real unified line diff in each modified change's
`content_preview` field, reconstructed from text content stored in both
manifests. Because every snapshot stores inline text content (UTF-8 files under
256 KiB), this works even between two stored manifests, not just against the live
tree. Binary, oversized, or `--no-content` files report that the content was
unavailable instead.

## Safety

- Destructive `rm` refuses to run without `--yes` when stdin is not a TTY.
- tidemark never records its own `.tidemark/` store directory in a snapshot.
- Labels are validated against path traversal and control characters.
