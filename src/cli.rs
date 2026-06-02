//! CLI command tree and dispatch.

use clap::{Parser, Subcommand, ValueEnum};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use crate::builder::SnapOptions;
use crate::error::TidemarkError;
use crate::manifest::Manifest;
use crate::output::{self, Format};
use crate::store::Store;
use crate::walk::WalkOptions;

#[derive(Parser)]
#[command(
    name = "tidemark",
    version,
    about = "Snapshot a directory tree and diff what changed - no git required."
)]
struct Cli {
    /// Output format. Defaults to json when piped, table on a terminal.
    #[arg(long, global = true, value_enum)]
    output: Option<FormatArg>,
    /// Shorthand for `--output json`.
    #[arg(long, global = true)]
    json: bool,
    /// Suppress diagnostics on stderr (data on stdout is unaffected).
    #[arg(long, global = true)]
    quiet: bool,
    /// Assume yes for destructive prompts (required to delete non-interactively).
    #[arg(long, global = true)]
    yes: bool,
    /// Limit the number of items in list/diff output.
    #[arg(long, global = true)]
    limit: Option<usize>,
    /// Skip this many items in list/diff output.
    #[arg(long, global = true, default_value_t = 0)]
    offset: usize,
    /// Restrict output objects to these fields (comma-separated).
    #[arg(long, global = true, value_delimiter = ',')]
    fields: Vec<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Copy, Clone, ValueEnum)]
enum FormatArg {
    Json,
    Table,
}
impl From<FormatArg> for Format {
    fn from(f: FormatArg) -> Self {
        match f {
            FormatArg::Json => Format::Json,
            FormatArg::Table => Format::Table,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Snapshot a directory into a manifest.
    Snap {
        label: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long = "output-file", short = 'o')]
        output_file: Option<PathBuf>,
        #[arg(long)]
        ignore: Vec<String>,
        #[arg(long)]
        hidden: bool,
        #[arg(long)]
        no_ignore: bool,
        /// Do not store inline text content (smaller manifest, no content diffs).
        #[arg(long)]
        no_content: bool,
        #[arg(long)]
        force: bool,
    },
    /// Diff two refs (label | manifest file | @ current tree).
    Diff {
        a: Option<String>,
        b: Option<String>,
        #[arg(long)]
        content: bool,
        #[arg(long, value_delimiter = ',')]
        only: Vec<ChangeKindArg>,
        #[arg(long)]
        exit_code: bool,
    },
    /// List stored snapshots.
    List,
    /// Show a manifest by ref.
    Show { reference: String },
    /// Remove stored snapshots.
    Rm { labels: Vec<String> },
    /// Create the snapshot store in the current directory (idempotent).
    Init {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Emit the clispec schema.
    Schema,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ChangeKindArg {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Cross-cutting options shared by every command.
struct Ctx {
    fmt: Format,
    quiet: bool,
    yes: bool,
    limit: Option<usize>,
    offset: usize,
    fields: Vec<String>,
}

fn snap_opts(
    hidden: bool,
    no_ignore: bool,
    ignore: Vec<String>,
    store_content: bool,
) -> SnapOptions {
    SnapOptions {
        walk: WalkOptions {
            hidden,
            use_ignore: !no_ignore,
            extra_ignores: ignore,
        },
        store_content,
    }
}

/// Entry point used by `main`. Returns a process exit code.
pub fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => return handle_parse_error(e),
    };
    let stdout_is_tty = std::io::stdout().is_terminal();
    let fmt = if cli.json {
        Format::Json
    } else {
        output::resolve_format(cli.output.map(Into::into), stdout_is_tty)
    };
    let ctx = Ctx {
        fmt,
        quiet: cli.quiet,
        yes: cli.yes,
        limit: cli.limit,
        offset: cli.offset,
        fields: cli.fields,
    };
    // No subcommand defaults to listing stored snapshots.
    let command = cli.command.unwrap_or(Command::List);
    match dispatch(command, &ctx) {
        Ok(code) => code,
        Err(e) => {
            emit_error(&e);
            2
        }
    }
}

/// Map a clap parse failure to either help/version output (exit 0) or a
/// structured JSON error on stderr (exit 2), per clispec.
fn handle_parse_error(e: clap::Error) -> i32 {
    use clap::error::ErrorKind as CK;
    match e.kind() {
        CK::DisplayHelp | CK::DisplayVersion | CK::DisplayHelpOnMissingArgumentOrSubcommand => {
            print!("{e}");
            0
        }
        _ => {
            let message = e
                .to_string()
                .lines()
                .next()
                .unwrap_or("invalid arguments")
                .trim_start_matches("error: ")
                .to_string();
            let payload = serde_json::json!({
                "error": { "kind": "invalid_input", "message": message, "retryable": false }
            });
            eprintln!("{payload}");
            2
        }
    }
}

fn emit_error(e: &TidemarkError) {
    let payload = serde_json::json!({
        "error": { "kind": e.kind, "message": e.message, "retryable": e.kind.retryable() }
    });
    eprintln!("{payload}");
}

fn dispatch(cmd: Command, ctx: &Ctx) -> Result<i32, TidemarkError> {
    match cmd {
        Command::Schema => {
            print_json(&crate::schema::schema());
            Ok(0)
        }
        Command::Init { path } => cmd_init(&path, ctx),
        Command::Snap {
            label,
            path,
            output_file,
            ignore,
            hidden,
            no_ignore,
            no_content,
            force,
        } => cmd_snap(
            label,
            path,
            output_file,
            ignore,
            hidden,
            no_ignore,
            no_content,
            force,
            ctx,
        ),
        Command::Diff {
            a,
            b,
            content,
            only,
            exit_code,
        } => cmd_diff(a, b, content, only, exit_code, ctx),
        Command::List => cmd_list(ctx),
        Command::Show { reference } => cmd_show(reference),
        Command::Rm { labels } => cmd_rm(labels, ctx),
    }
}

fn cmd_init(path: &std::path::Path, ctx: &Ctx) -> Result<i32, TidemarkError> {
    let store = Store::at(path);
    store.init()?;
    let payload = serde_json::json!({ "initialized": true, "path": ".tidemark" });
    match ctx.fmt {
        Format::Json => print_json(&payload),
        Format::Table => println!("initialized store at .tidemark"),
    }
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
fn cmd_snap(
    label: Option<String>,
    path: PathBuf,
    output_file: Option<PathBuf>,
    ignore: Vec<String>,
    hidden: bool,
    no_ignore: bool,
    no_content: bool,
    force: bool,
    ctx: &Ctx,
) -> Result<i32, TidemarkError> {
    let opts = snap_opts(hidden, no_ignore, ignore, !no_content);
    let manifest = crate::builder::build_manifest(&path, &opts)?;
    let mut created = false;

    if let Some(l) = &label {
        let store = Store::at(&path);
        store.save(l, &manifest, force)?;
        created = true;
    }

    let mut wrote_manifest_to_stdout = false;
    if let Some(out) = &output_file {
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| TidemarkError::io(e.to_string()))?;
        if out.as_os_str() == "-" {
            println!("{json}");
            wrote_manifest_to_stdout = true;
        } else {
            std::fs::write(out, json)?;
            created = true;
        }
    }

    // Default destination: no label and no -o, piped -> write manifest to stdout.
    if label.is_none() && output_file.is_none() && matches!(ctx.fmt, Format::Json) {
        print_json(&serde_json::to_value(&manifest).unwrap());
        return Ok(0);
    }

    if wrote_manifest_to_stdout {
        return Ok(0);
    }

    let summary = serde_json::json!({
        "label": label,
        "tree_digest": manifest.tree_digest,
        "entry_count": manifest.entry_count,
        "created": created
    });
    match ctx.fmt {
        Format::Json => print_json(&summary),
        Format::Table => {
            let label_suffix = match &label {
                Some(l) => format!("  [{l}]"),
                None => String::new(),
            };
            println!(
                "snapped {} entries  {}{}",
                manifest.entry_count, manifest.tree_digest, label_suffix
            );
        }
    }
    Ok(0)
}

fn cmd_diff(
    a: Option<String>,
    b: Option<String>,
    content: bool,
    only: Vec<ChangeKindArg>,
    exit_code: bool,
    ctx: &Ctx,
) -> Result<i32, TidemarkError> {
    let base = PathBuf::from(".");
    let store = Store::at(&base);
    let sopts = SnapOptions::default();
    let (aref, bref) = resolve_diff_refs(a, b, &store)?;
    let old = crate::refs::resolve(&aref, &base, &store, &sopts)?;
    let new = crate::refs::resolve(&bref, &base, &store, &sopts)?;
    let mut report = crate::diff::diff(&old, &new);

    if !only.is_empty() {
        report
            .changes
            .retain(|c| only.iter().any(|k| matches_kind(k, &c.kind)));
        report.recount();
    }
    if content {
        attach_content(&mut report, &old, &new);
    }

    let all: Vec<serde_json::Value> = report
        .changes
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();
    let (page, total) = output::paginate(all, ctx.offset, ctx.limit);
    let page = output::select_fields(page, &ctx.fields);
    let changed = total > 0;
    let env = serde_json::json!({
        "changes": page,
        "added": report.added,
        "modified": report.modified,
        "deleted": report.deleted,
        "renamed": report.renamed,
        "total": total,
        "limit": ctx.limit,
        "offset": ctx.offset
    });
    match ctx.fmt {
        Format::Json => print_json(&env),
        Format::Table => print_diff_table(&report, &page, ctx.quiet),
    }
    if exit_code {
        Ok(if changed { 1 } else { 0 })
    } else {
        Ok(0)
    }
}

fn cmd_list(ctx: &Ctx) -> Result<i32, TidemarkError> {
    let store = Store::at(&PathBuf::from("."));
    let items = store.list()?;
    let values: Vec<serde_json::Value> = items
        .iter()
        .map(|i| serde_json::to_value(i).unwrap())
        .collect();
    let (page, total) = output::paginate(values, ctx.offset, ctx.limit);
    let page = output::select_fields(page, &ctx.fields);
    match ctx.fmt {
        Format::Json => {
            let env = output::list_envelope(page, total, ctx.limit, ctx.offset);
            print_json(&env);
        }
        Format::Table => {
            for it in &page {
                println!(
                    "{}\t{}\t{} entries",
                    it.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                    it.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
                    it.get("entry_count").and_then(|v| v.as_u64()).unwrap_or(0)
                );
            }
        }
    }
    Ok(0)
}

fn cmd_show(reference: String) -> Result<i32, TidemarkError> {
    let base = PathBuf::from(".");
    let store = Store::at(&base);
    let m = crate::refs::resolve(&reference, &base, &store, &SnapOptions::default())?;
    print_json(&serde_json::to_value(&m).unwrap());
    Ok(0)
}

fn cmd_rm(labels: Vec<String>, ctx: &Ctx) -> Result<i32, TidemarkError> {
    if labels.is_empty() {
        return Err(TidemarkError::invalid("no labels given"));
    }
    if !ctx.yes && !std::io::stdin().is_terminal() {
        return Err(TidemarkError::invalid(
            "refusing to delete without --yes in non-interactive mode",
        ));
    }
    let store = Store::at(&PathBuf::from("."));
    let mut removed = Vec::new();
    for l in &labels {
        store.remove(l)?;
        removed.push(l.clone());
    }
    print_json(&serde_json::json!({ "removed": removed }));
    Ok(0)
}

fn matches_kind(arg: &ChangeKindArg, kind: &crate::diff::ChangeKind) -> bool {
    use crate::diff::ChangeKind as K;
    matches!(
        (arg, kind),
        (ChangeKindArg::Added, K::Added)
            | (ChangeKindArg::Modified, K::Modified)
            | (ChangeKindArg::Deleted, K::Deleted)
            | (ChangeKindArg::Renamed, K::Renamed)
    )
}

fn resolve_diff_refs(
    a: Option<String>,
    b: Option<String>,
    store: &Store,
) -> Result<(String, String), TidemarkError> {
    match (a, b) {
        (None, _) => {
            let latest = store.latest()?.ok_or_else(|| {
                TidemarkError::not_found("no stored snapshots; run `tidemark snap <label>` first")
            })?;
            Ok((latest, "@".to_string()))
        }
        (Some(a), None) => Ok((a, "@".to_string())),
        (Some(a), Some(b)) => Ok((a, b)),
    }
}

/// Attach a real unified content diff for modified files using the inline text
/// content stored in both manifests. When either side lacks stored content
/// (binary, oversized, or snapped with `--no-content`), the preview reports that
/// instead of a line diff.
fn attach_content(report: &mut crate::diff::DiffReport, old: &Manifest, new: &Manifest) {
    for c in report.changes.iter_mut() {
        if !matches!(c.kind, crate::diff::ChangeKind::Modified) {
            continue;
        }
        c.content_preview = Some(match (content_of(old, &c.path), content_of(new, &c.path)) {
            (Some(o), Some(n)) => crate::diff::unified_diff(o, n, &c.path),
            _ => "<content unavailable (binary, too large, or --no-content)>".to_string(),
        });
    }
}

/// Look up the inline text content for `path` in a manifest, if stored.
fn content_of<'a>(m: &'a Manifest, path: &str) -> Option<&'a str> {
    m.entries
        .iter()
        .find(|e| e.path == path)
        .and_then(|e| e.content.as_deref())
}

fn print_json(v: &serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string_pretty(v).unwrap());
    let _ = out.flush();
}

fn print_diff_table(report: &crate::diff::DiffReport, page: &[serde_json::Value], quiet: bool) {
    use owo_colors::OwoColorize;
    let color = std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
    for c in page {
        let kind = c.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let path = c.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let sym = match kind {
            "added" => "+",
            "deleted" => "-",
            "modified" => "~",
            "renamed" => "\u{2192}",
            _ => "?",
        };
        if color {
            let line = match kind {
                "added" => format!("{sym} {path}").green().to_string(),
                "deleted" => format!("{sym} {path}").red().to_string(),
                "modified" => format!("{sym} {path}").yellow().to_string(),
                "renamed" => format!("{sym} {path}").blue().to_string(),
                _ => format!("{sym} {path}"),
            };
            println!("{line}");
        } else {
            println!("{sym} {path}");
        }
        if let Some(prev) = c.get("content_preview").and_then(|v| v.as_str()) {
            for line in prev.lines() {
                println!("    {line}");
            }
        }
    }
    if !quiet {
        eprintln!(
            "{} added, {} modified, {} deleted, {} renamed",
            report.added, report.modified, report.deleted, report.renamed
        );
    }
}
