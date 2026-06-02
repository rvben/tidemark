# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
make check          # lint + test (CI runs this)
make build          # cargo build --release
make test           # cargo nextest run + cargo test --doc
make lint           # cargo fmt --check + cargo clippy -D warnings
make fmt            # auto-format
make score          # build release + clispec score ./target/release/tidemark
make ci             # check + score
make install        # cargo install --path .
cargo nextest run <name>   # run a single test by name
```

## Architecture

tidemark is a Rust CLI that snapshots a directory tree into a deterministic
BLAKE3 manifest and diffs two snapshots (added/modified/deleted/renamed) with no
git required. Binary name: `tidemark`. Also published to PyPI (`uvx tidemark`)
and Homebrew (`brew install rvben/tap/tidemark`). Scores 100/100 against
[The CLI Spec](https://clispec.dev).

### Pure-core + thin-shell layout

- **Pure (no I/O, unit-tested directly):** `manifest` (`Manifest`/`Entry`, Merkle
  `tree_digest`), `diff` (classification, rename detection, `unified_diff`).
- **I/O wrappers:** `walk` (directory traversal via the `ignore` crate,
  `require_git(false)`), `hash` (`hash_bytes`), `builder` (`build_manifest` +
  `SnapOptions`), `store` (`.tidemark/` labeled snapshot store), `refs` (resolve a
  ref: store label | manifest file | `@` current tree).
- **Shell:** `output` (JSON/table, pagination, field selection), `schema` (the
  clispec v0.1 document), `cli` (clap tree, dispatch, exit codes), `error`
  (`TidemarkError` + clispec error kinds).

### Key patterns

- **clispec scorer probes the first non-mutating command** for structured-output
  and stream checks, so `list` MUST stay ordered before `diff` in `schema.rs`
  (`tidemark diff` errors with no snapshot; `list` always exits 0 with an
  envelope). Reordering drops the score.
- **Schema matches `clispec.dev/schema/v0.1.json`:** top-level
  `name`/`version`/`commands`/`errors`; per-command `mutating` boolean +
  `output_fields`; `errors` is a top-level array of `{kind, retryable}`.
- **`mtime` is informational only** - excluded from `tree_digest` and change
  detection, so `touch`/rebuilds never create false positives.
- **Rename detection only on unambiguous `(hash, mode, kind)` signatures**
  (exactly one deleted + one added share it); duplicate-content files stay as
  add/delete.
- **Inline content** stored for UTF-8 files <= 256 KiB (`CONTENT_CAP_BYTES`)
  enables real two-sided `diff --content`, even between two stored manifests.
- **stdout/stderr flushed before `process::exit`** in `main.rs` (exit skips
  destructors, so a buffered pipe could otherwise truncate).
- Destructive `rm` requires `--yes` when stdin is not a TTY. tidemark never
  records its own `.tidemark/` store in a snapshot. Labels are validated against
  path traversal and control characters.

### CI / release

All CI steps are make targets; the pipeline only runs make. `ci.yml` runs
lint/test/coverage on push and PR. `release.yml` triggers on a `v*` tag (or
manual dispatch, dry-run by default) and publishes to crates.io + PyPI + a
GitHub release, then updates the Homebrew tap formula. Release is driven by
`make release-patch|minor|major` (vership).

## Documentation

- `README.md` - human-facing usage.
- `AGENTS.md` - the agent-facing operational contract.
- `docs/superpowers/` - design spec and implementation plan (gitignored).
