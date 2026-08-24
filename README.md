# tidemark

Snapshot a directory tree and diff exactly what changed - no git required.

A *tidemark* is the line a receding tide leaves behind, recording the level the
water reached so you can see how things stood before. `tidemark` does the same
for a directory: leave a mark, do anything, then see precisely what was added,
modified, deleted, or renamed.

It scores **100/100** against [The CLI Spec](https://clispec.dev): structured
output, schema introspection, clean stdout/stderr separation, non-interactive by
default, idempotent operations, and bounded output. Excellent for humans at a
terminal and for AI agents in a pipeline.

## Why

"What did that command actually do to my files?" is a question without a good
answer today. `git` needs a repo and pollutes the tree; `fsdiff` was archived;
copy-based tools need two full copies. `tidemark` is a small, deterministic
(BLAKE3) witness that answers it in one bounded call.

## Install

```
cargo install tidemark
brew install rvben/tap/tidemark
```

## Quick start

Human workflow with a labeled checkpoint:

```
tidemark snap before        # checkpoint the current tree
make install                # do the risky thing
tidemark diff before        # colored table of what changed
```

Agent / CI workflow with a portable manifest:

```
tidemark snap -o pre.tidemark        # write a manifest file (use - for stdout)
./run-some-tool
tidemark diff pre.tidemark @ --json  # bounded JSON delta in one call
```

## Commands

| Command | Description |
| --- | --- |
| `tidemark snap [LABEL]` | Snapshot the tree. `LABEL` stores it under `.tidemark/`; `-o FILE` writes a portable manifest (`-` = stdout). |
| `tidemark diff [A] [B]` | Diff two refs. A ref is a label, a manifest file, or `@` (current tree). `tidemark diff before` means `before` vs `@`. |
| `tidemark list` | List stored snapshots. Also the default when no command is given. |
| `tidemark show REF` | Print a manifest. |
| `tidemark rm LABEL... --yes` | Delete stored snapshots. |
| `tidemark init` | Create the `.tidemark/` store in the current directory (idempotent). |
| `tidemark schema` | Emit the clispec schema describing every command and output field. |

### Useful flags

- `snap`: `--path DIR`, `--ignore GLOB` (repeatable), `--hidden`, `--no-ignore`,
  `--no-content` (smaller manifest, disables content diffs), `--force` (overwrite
  a label whose tree differs).
- `diff`: `--content` (real unified line diffs for text files), `--only
  added,modified,deleted,renamed`, `--exit-code`.
- Global (accepted on any command): `--output json|table` (defaults to JSON when
  piped, a table on a TTY), `--json` (shorthand for `--output json`), `--quiet`
  (suppress the stderr summary), `--yes` (confirm destructive ops), and the
  bounded-output flags `--limit N`, `--offset N`, `--fields a,b` for `list` and
  `diff`.

## What counts as a change

Each entry records its BLAKE3 content hash, size, Unix mode, and symlink target.
A file is **modified** when its content, mode, or symlink target changes. A
**rename** is detected when a deletion and an addition share the same content
hash and mode (and the pairing is unambiguous - duplicate-content files stay as
plain add/delete). `mtime` is recorded for information but never triggers a
change, so `touch` and rebuilds do not create false positives.

Small UTF-8 files (up to 256 KiB) also store their text inline so `diff
--content` can show real before/after line diffs, even between two stored
manifests. Pass `--no-content` to skip this.

## Output

Snapshots of an unchanged tree produce an identical `tree_digest` - that is the
idempotency signal. `diff` JSON is a flat object with summary counts and a
bounded `changes` array; `list` uses the standard envelope:

```json
{ "items": [ ... ], "total": 3, "limit": null, "offset": 0 }
```

## Exit codes

- Default: `0` success, `2` error.
- With `--exit-code` (the diff(1) convention): `0` no changes, `1` changes
  found, `2` error.

Errors are written to stderr as
`{"error": {"kind": "...", "message": "...", "retryable": <bool>}}`. Kinds:
`not_found`, `conflict`, `invalid_input`, `io` (retryable), `unsupported`.

See [AGENTS.md](AGENTS.md) for the agent-focused contract.

## Development

All CI steps are make targets, so the pipeline only runs make:

```
make check   # fmt-check + clippy (-D warnings) + tests + doctests
make score   # build release and score against clispec
make ci      # check + score
```

## License

MIT

## Releasing

Vership owns versioning, changelog generation, release commits, and tags. See
[the release runbook](docs/releases.md) for the verified workflow and recovery policy.
