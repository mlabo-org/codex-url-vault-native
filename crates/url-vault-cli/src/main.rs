use std::fs;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use url_vault_core::{
    AgentHost, ApplyImportInput, SaveSnapshotInput, SaveUrlInput, UpdateUrlInput, Vault,
    VaultError,
};

#[derive(Debug, Parser)]
#[command(name = "url-vault", version, about = "Native Codex URL Vault CLI")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init(JsonFlag),
    Add(AddArgs),
    ImportHtml(ImportArgs),
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    Search(QueryArgs),
    Suggest(QueryArgs),
    Open(OpenArgs),
    List(ListArgs),
    ExportHtml(ExportArgs),
    RefreshDb(JsonFlag),
    Edit(EditArgs),
    Move(MoveArgs),
    Delete(TargetArgs),
    Category {
        #[command(subcommand)]
        command: CategoryCommand,
    },
}

#[derive(Debug, Args)]
struct JsonFlag {
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AddArgs {
    url: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long)]
    tags: Option<String>,
    #[arg(long)]
    aliases: Option<String>,
    #[arg(long)]
    intents: Option<String>,
    /// Defaults to the resolved agent host's capture type (`codex_capture` or `claude_code_capture`).
    #[arg(long)]
    source_type: Option<String>,
    #[arg(long)]
    source_browser: Option<String>,
    #[arg(long)]
    source_profile: Option<String>,
    #[arg(long, alias = "category")]
    folder_path: Option<String>,
    #[arg(long)]
    preferred_browser: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long, default_value = "active")]
    status: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, ValueEnum)]
enum ImportMode {
    Add,
    Merge,
    Reset,
}

impl ImportMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Merge => "merge",
            Self::Reset => "reset",
        }
    }
}

#[derive(Debug, Args)]
struct ImportArgs {
    file: PathBuf,
    #[arg(long)]
    source_browser: String,
    #[arg(long, default_value = "")]
    source_profile: String,
    #[arg(long)]
    preferred_browser: Option<String>,
    #[arg(long)]
    strip_common_root: bool,
    #[arg(long, value_enum, default_value_t = ImportMode::Merge)]
    mode: ImportMode,
}

#[derive(Debug, Subcommand)]
enum SnapshotCommand {
    Add(SnapshotAddArgs),
    Show(TargetArgs),
    Verify(SnapshotVerifyArgs),
}

#[derive(Debug, Args)]
struct SnapshotAddArgs {
    target: String,
    input: PathBuf,
    #[arg(long, default_value = "semantic")]
    kind: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    captured_at: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SnapshotVerifyArgs {
    target: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct QueryArgs {
    query: String,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, ValueEnum)]
enum Browser {
    Default,
    Brave,
    Chrome,
    Firefox,
    Safari,
}

impl Browser {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Brave => "brave",
            Self::Chrome => "chrome",
            Self::Firefox => "firefox",
            Self::Safari => "safari",
        }
    }
}

#[derive(Debug, Args)]
struct OpenArgs {
    target: String,
    #[arg(long, value_enum, default_value_t = Browser::Default)]
    browser: Browser,
    /// Defaults to the resolved agent host (`codex` or `claude_code`).
    #[arg(long)]
    opened_by: Option<String>,
    #[arg(long)]
    context: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, ValueEnum)]
enum ListKind {
    Bookmarks,
    Categories,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long, value_enum, default_value_t = ListKind::Bookmarks)]
    kind: ListKind,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    limit: Option<usize>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ExportArgs {
    output: Option<PathBuf>,
    #[arg(long, default_value = "Codex URL Vault")]
    top_folder: String,
    #[arg(long)]
    no_top_folder: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct EditArgs {
    target: String,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long)]
    tags: Option<String>,
    #[arg(long)]
    aliases: Option<String>,
    #[arg(long)]
    intents: Option<String>,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    preferred_browser: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct MoveArgs {
    target: String,
    category: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct TargetArgs {
    target: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum CategoryCommand {
    List(JsonFlag),
    Add(CategoryAddArgs),
    Rename(CategoryRenameArgs),
    Delete(CategoryDeleteArgs),
}

#[derive(Debug, Args)]
struct CategoryAddArgs {
    path: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CategoryRenameArgs {
    old_path: String,
    new_path: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CategoryDeleteArgs {
    path: String,
    #[arg(long, default_value = "")]
    move_to: String,
    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("url-vault: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), VaultError> {
    let cli = Cli::parse();
    let vault = if let Some(home) = cli.home {
        Vault::at(home)?
    } else {
        Vault::from_env()?
    };
    match cli.command {
        Command::Init(flag) => output(&vault.init()?, flag.json),
        Command::Add(args) => {
            let result = vault.save_url(SaveUrlInput {
                url: args.url,
                title: args.title,
                description: args.description,
                note: args.note,
                tags: parse_list(args.tags.as_deref()),
                aliases: parse_list(args.aliases.as_deref()),
                intents: parse_list(args.intents.as_deref()),
                source_type: Some(args.source_type.unwrap_or_else(|| {
                    AgentHost::from_env().capture_source_type().to_owned()
                })),
                source_browser: args.source_browser,
                source_profile: args.source_profile,
                folder_path: args.folder_path,
                preferred_browser: args.preferred_browser,
                project: args.project,
                status: Some(args.status),
            })?;
            output(&result, args.json)
        }
        Command::ImportHtml(args) => {
            let html = fs::read_to_string(&args.file).map_err(|error| {
                VaultError::InvalidInput(format!(
                    "bookmark HTML not readable at {}: {error}",
                    args.file.display()
                ))
            })?;
            let preview = vault.preview_bookmark_import(
                &html,
                &args.file.display().to_string(),
                args.strip_common_root,
            )?;
            let result = vault.apply_bookmark_import(ApplyImportInput {
                html,
                file_name: args.file.display().to_string(),
                source_browser: args.source_browser,
                source_profile: args.source_profile,
                preferred_browser: args.preferred_browser,
                mode: args.mode.as_str().to_owned(),
                strip_common_root: args.strip_common_root,
                expected_sha256: preview.content_sha256,
                confirm_reset: matches!(args.mode, ImportMode::Reset),
            })?;
            output(&result, true)
        }
        Command::Snapshot { command } => match command {
            SnapshotCommand::Add(args) => {
                let content = fs::read_to_string(&args.input).map_err(|error| {
                    VaultError::InvalidInput(format!(
                        "snapshot input not readable at {}: {error}",
                        args.input.display()
                    ))
                })?;
                let result = vault.save_snapshot(SaveSnapshotInput {
                    target: args.target,
                    content,
                    kind: args.kind,
                    title: args.title,
                    captured_at: args.captured_at,
                })?;
                output(&result, args.json)
            }
            SnapshotCommand::Show(args) => output(&vault.get_snapshot(&args.target)?, args.json),
            SnapshotCommand::Verify(args) => {
                let result = vault.verify_snapshots(args.target.as_deref())?;
                let failed = result.invalid > 0;
                output(&result, args.json)?;
                if failed {
                    std::process::exit(1);
                }
                Ok(())
            }
        },
        Command::Search(args) => output(&vault.search_urls(&args.query, args.limit)?, args.json),
        Command::Suggest(args) => output(&vault.suggest_urls(&args.query, args.limit)?, args.json),
        Command::Open(args) => output(
            &vault.open_url(
                &args.target,
                Some(args.browser.as_str()),
                args.dry_run,
                args.opened_by
                    .as_deref()
                    .unwrap_or(AgentHost::from_env().as_str()),
                args.context.as_deref(),
            )?,
            args.json,
        ),
        Command::List(args) => match args.kind {
            ListKind::Bookmarks => output(
                &vault.list_urls(args.category.as_deref(), args.limit)?,
                args.json,
            ),
            ListKind::Categories => output(&vault.list_categories()?, args.json),
        },
        Command::ExportHtml(args) => {
            let output_path = args.output.unwrap_or_else(|| vault.canonical_html_path());
            let top_folder = if args.no_top_folder {
                ""
            } else {
                &args.top_folder
            };
            let result = vault.export_html(&output_path, top_folder)?;
            output(&serde_json::json!({ "output": result }), args.json)
        }
        Command::RefreshDb(flag) => output(&vault.refresh_db()?, flag.json),
        Command::Edit(args) => output(
            &vault.update_url(
                &args.target,
                UpdateUrlInput {
                    url: args.url,
                    title: args.title,
                    description: args.description,
                    note: args.note,
                    tags: args.tags.as_deref().map(|value| parse_list(Some(value))),
                    aliases: args.aliases.as_deref().map(|value| parse_list(Some(value))),
                    intents: args.intents.as_deref().map(|value| parse_list(Some(value))),
                    folder_path: args.category,
                    preferred_browser: args.preferred_browser,
                    project: args.project,
                },
            )?,
            args.json,
        ),
        Command::Move(args) => output(&vault.move_url(&args.target, &args.category)?, args.json),
        Command::Delete(args) => output(&vault.archive_url(&args.target)?, args.json),
        Command::Category { command } => match command {
            CategoryCommand::List(flag) => output(&vault.list_categories()?, flag.json),
            CategoryCommand::Add(args) => output(
                &vault.create_category(&args.path, args.label.as_deref(), args.note.as_deref())?,
                args.json,
            ),
            CategoryCommand::Rename(args) => output(
                &vault.rename_category(&args.old_path, &args.new_path)?,
                args.json,
            ),
            CategoryCommand::Delete(args) => output(
                &vault.archive_category(&args.path, &args.move_to)?,
                args.json,
            ),
        },
    }
}

fn output(value: &impl Serialize, _json: bool) -> Result<(), VaultError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_list(value: Option<&str>) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(value) = value {
        for item in value.replace(';', ",").split(',') {
            let item = item.trim();
            if !item.is_empty() && !result.iter().any(|existing| existing == item) {
                result.push(item.to_owned());
            }
        }
    }
    result
}
