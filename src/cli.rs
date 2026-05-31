//! CLI command tree and dispatch.

use clap::{Parser, Subcommand, ValueEnum};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use crate::builder::SnapOptions;
use crate::error::KairnError;
use crate::manifest::Manifest;
use crate::output::{self, Format};
use crate::store::Store;
use crate::walk::WalkOptions;

#[derive(Parser)]
#[command(
    name = "kairn",
    version,
    about = "Snapshot a directory tree and diff what changed - no git required."
)]
struct Cli {
    /// Output format. Defaults to json when piped, table on a terminal.
    #[arg(long, global = true, value_enum)]
    output: Option<FormatArg>,
    #[command(subcommand)]
    command: Command,
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
        limit: Option<usize>,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, value_delimiter = ',')]
        fields: Vec<String>,
        #[arg(long)]
        exit_code: bool,
    },
    /// List stored snapshots.
    List {
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, value_delimiter = ',')]
        fields: Vec<String>,
    },
    /// Show a manifest by ref.
    Show { reference: String },
    /// Remove stored snapshots.
    Rm {
        labels: Vec<String>,
        #[arg(long)]
        yes: bool,
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
    let cli = Cli::parse();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let fmt = output::resolve_format(cli.output.map(Into::into), stdout_is_tty);
    match dispatch(cli.command, fmt) {
        Ok(code) => code,
        Err(e) => {
            emit_error(&e);
            2
        }
    }
}

fn emit_error(e: &KairnError) {
    let payload = serde_json::json!({
        "error": { "kind": e.kind, "message": e.message, "retryable": e.kind.retryable() }
    });
    eprintln!("{payload}");
}

fn dispatch(cmd: Command, fmt: Format) -> Result<i32, KairnError> {
    match cmd {
        Command::Schema => {
            print_json(&crate::schema::schema());
            Ok(0)
        }
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
            fmt,
        ),
        Command::Diff {
            a,
            b,
            content,
            only,
            limit,
            offset,
            fields,
            exit_code,
        } => cmd_diff(a, b, content, only, limit, offset, fields, exit_code, fmt),
        Command::List {
            limit,
            offset,
            fields,
        } => cmd_list(limit, offset, fields, fmt),
        Command::Show { reference } => cmd_show(reference),
        Command::Rm { labels, yes } => cmd_rm(labels, yes),
    }
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
    fmt: Format,
) -> Result<i32, KairnError> {
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
        let json =
            serde_json::to_string_pretty(&manifest).map_err(|e| KairnError::io(e.to_string()))?;
        if out.as_os_str() == "-" {
            println!("{json}");
            wrote_manifest_to_stdout = true;
        } else {
            std::fs::write(out, json)?;
            created = true;
        }
    }

    // Default destination: no label and no -o, piped -> write manifest to stdout.
    if label.is_none() && output_file.is_none() && matches!(fmt, Format::Json) {
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
    match fmt {
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

#[allow(clippy::too_many_arguments)]
fn cmd_diff(
    a: Option<String>,
    b: Option<String>,
    content: bool,
    only: Vec<ChangeKindArg>,
    limit: Option<usize>,
    offset: usize,
    fields: Vec<String>,
    exit_code: bool,
    fmt: Format,
) -> Result<i32, KairnError> {
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
    }
    if content {
        attach_content(&mut report, &old, &new);
    }

    let all: Vec<serde_json::Value> = report
        .changes
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();
    let (page, total) = output::paginate(all, offset, limit);
    let page = output::select_fields(page, &fields);
    let changed = total > 0;
    let env = serde_json::json!({
        "changes": page,
        "added": report.added,
        "modified": report.modified,
        "deleted": report.deleted,
        "renamed": report.renamed,
        "total": total,
        "limit": limit,
        "offset": offset
    });
    match fmt {
        Format::Json => print_json(&env),
        Format::Table => print_diff_table(&report, &page),
    }
    if exit_code {
        Ok(if changed { 1 } else { 0 })
    } else {
        Ok(0)
    }
}

fn cmd_list(
    limit: Option<usize>,
    offset: usize,
    fields: Vec<String>,
    fmt: Format,
) -> Result<i32, KairnError> {
    let store = Store::at(&PathBuf::from("."));
    let items = store.list()?;
    let values: Vec<serde_json::Value> = items
        .iter()
        .map(|i| serde_json::to_value(i).unwrap())
        .collect();
    let (page, total) = output::paginate(values, offset, limit);
    let page = output::select_fields(page, &fields);
    match fmt {
        Format::Json => {
            let env = output::list_envelope(page, total, limit, offset);
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

fn cmd_show(reference: String) -> Result<i32, KairnError> {
    let base = PathBuf::from(".");
    let store = Store::at(&base);
    let m = crate::refs::resolve(&reference, &base, &store, &WalkOptions::default())?;
    print_json(&serde_json::to_value(&m).unwrap());
    Ok(0)
}

fn cmd_rm(labels: Vec<String>, yes: bool) -> Result<i32, KairnError> {
    if labels.is_empty() {
        return Err(KairnError::invalid("no labels given"));
    }
    if !yes && !std::io::stdin().is_terminal() {
        return Err(KairnError::invalid(
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
) -> Result<(String, String), KairnError> {
    match (a, b) {
        (None, _) => {
            let latest = store.latest()?.ok_or_else(|| {
                KairnError::not_found("no stored snapshots; run `kairn snap <label>` first")
            })?;
            Ok((latest, "@".to_string()))
        }
        (Some(a), None) => Ok((a, "@".to_string())),
        (Some(a), Some(b)) => Ok((a, b)),
    }
}

/// Attach a content preview for modified files when the NEW side is the current
/// tree (`@`). Manifests store hashes, not bytes, so the old side cannot be
/// reconstructed; we surface the current file body under a unified header.
fn attach_content(
    report: &mut crate::diff::DiffReport,
    base: &Path,
    bref: &str,
) -> Result<(), KairnError> {
    if bref != "@" {
        return Ok(());
    }
    for c in report.changes.iter_mut() {
        if !matches!(c.kind, crate::diff::ChangeKind::Modified) {
            continue;
        }
        let p = base.join(&c.path);
        if let Ok(bytes) = std::fs::read(&p) {
            match String::from_utf8(bytes) {
                Ok(text) => c.content_preview = Some(text),
                Err(_) => c.content_preview = Some("<binary changed>".to_string()),
            }
        }
    }
    Ok(())
}

fn print_json(v: &serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string_pretty(v).unwrap());
}

fn print_diff_table(report: &crate::diff::DiffReport, page: &[serde_json::Value]) {
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
    eprintln!(
        "{} added, {} modified, {} deleted, {} renamed",
        report.added, report.modified, report.deleted, report.renamed
    );
}
