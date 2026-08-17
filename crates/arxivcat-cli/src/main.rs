mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "arxivcat",
    about = "ArXiv paper extraction and chat tool",
    version
)]
pub struct Cli {
    #[arg(short = 'w', long, help = "Override workspace path")]
    pub workspace: Option<PathBuf>,

    #[arg(long, global = true, help = "Output as JSON (machine-readable)")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Manage workspace")]
    Workspace {
        #[command(subcommand)]
        cmd: WorkspaceCmd,
    },

    #[command(about = "Manage papers")]
    Paper {
        #[command(subcommand)]
        cmd: PaperCmd,
    },

    #[command(about = "Chat with papers")]
    Chat {
        #[command(subcommand)]
        cmd: ChatCmd,
    },

    #[command(about = "Manage API token")]
    Token {
        #[command(subcommand)]
        cmd: TokenCmd,
    },

    #[command(subcommand, hide = true)]
    Internal(InternalCmd),
}

#[derive(Subcommand)]
pub enum WorkspaceCmd {
    #[command(about = "Open a workspace folder")]
    Open { path: PathBuf },

    #[command(about = "Scan workspace root for PDFs and create paper folders")]
    Scan,
}

#[derive(Debug, clap::Subcommand)]
pub enum TagCmd {
    /// List all existing tags.
    #[command(about = "List all existing tags")]
    List,
    /// Add a paper to a tag (creates the tag directory if new).
    #[command(about = "Add a paper to a tag (creates the tag directory if new)")]
    Add { id_or_query: String, tag: String },
    /// Remove a paper from a tag.
    #[command(about = "Remove a paper from a tag")]
    Remove { id_or_query: String, tag: String },
    /// Reclassify: set the FULL tag list of a paper (comma-separated).
    /// Tags not listed are removed, new ones are added.
    #[command(about = "Reclassify: set the full tag list (comma-separated)")]
    Set { id_or_query: String, tags: String },
    /// Remove ALL tags from a paper.
    #[command(about = "Remove all tags from a paper")]
    Clear { id_or_query: String },
}

#[derive(Debug, clap::Subcommand)]
pub enum PaperCmd {
    #[command(about = "List all papers in workspace")]
    List,

    #[command(about = "Download and extract a single paper")]
    Download {
        id_or_url: String,

        /// Skip automatic brief generation after download (default: on).
        #[arg(long)]
        no_describe: bool,

        /// Skip automatic deep recap generation after download (default: on).
        #[arg(long)]
        no_deep: bool,
    },

    #[command(about = "Download all pending papers in workspace")]
    DownloadAll {
        /// Number of parallel downloads (1-32). DeepSeek v4 concurrency is
        /// 2500 (flash) / 500 (pro); 429s are handled by Retry-After backoff.
        #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=32))]
        jobs: u8,

        /// Ignore the 24h per-paper retry cooldown.
        #[arg(long)]
        force: bool,

        /// Skip automatic brief generation after download (default: on).
        #[arg(long)]
        no_describe: bool,

        /// Skip automatic deep recap generation after download (default: on;
        /// deep recaps are spawned as detached workers, not awaited).
        #[arg(long)]
        no_deep: bool,
    },

    #[command(about = "Manage tags (tag = directory of symlinks into raw/)")]
    Tag {
        #[command(subcommand)]
        cmd: TagCmd,
    },

    #[command(about = "Show paper preview")]
    Preview {
        id_or_query: String,

        #[arg(short = 'v', long, default_value = "body")]
        view: String,
    },

    #[command(about = "View or edit note for a paper")]
    Note {
        id_or_query: String,

        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        text: Vec<String>,

        #[arg(short, long, help = "Open in editor")]
        edit: bool,
    },

    #[command(about = "Strip LaTeX comments from body.tex")]
    Strip { id_or_query: String },

    #[command(about = "Open paper folder in file manager")]
    Open { id_or_query: String },

    #[command(about = "Open PDF in browser")]
    Pdf { id_or_query: String },

    #[command(about = "Show paper info and file status")]
    Info { id_or_query: String },

    #[command(about = "Generate AI description for a paper (requires DeepSeek API key)")]
    Describe { id_or_query: String },

    #[command(about = "Generate the deep recap for a paper (two-round LLM)")]
    DeepSummarize {
        id_or_query: String,

        /// Regenerate even if a deep recap already exists.
        #[arg(long)]
        force: bool,
    },

    #[command(about = "Remove a paper folder from the workspace")]
    Remove { id_or_query: String },

    #[command(about = "Re-download a paper (notes/descriptions are preserved)")]
    Redownload { id_or_query: String },
}

#[derive(Subcommand)]
pub enum ChatCmd {
    #[command(about = "Start side chat scoped to one paper")]
    Side { id_or_query: String },

    #[command(about = "Start global chat over all workspace descriptions")]
    Global,
}

#[derive(Subcommand)]
pub enum InternalCmd {
    /// Detached worker: generate the deep recap for a paper dir (spawned by
    /// download-all; reads arxiv_id/title from the manifest so the round-1
    /// prefix stays byte-identical to the brief that built the cache).
    DeepWorker { paper_dir: String },

    /// Full download pipeline as an independent process (spawned by
    /// download-all). Emits line-delimited JSON events on stdout:
    /// downloading/downloaded/brief_done/deep_spawned/done/failed.
    DownloadWorker {
        paper_dir: String,

        #[arg(long)]
        no_describe: bool,

        #[arg(long)]
        no_deep: bool,
    },
}

#[derive(Subcommand)]
pub enum TokenCmd {
    #[command(about = "Show token status (masked)")]
    Status,

    #[command(about = "Set DeepSeek API token")]
    Set,

    #[command(about = "Validate cached token")]
    Validate,
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    e.print().ok();
                    std::process::exit(0);
                }
                _ => {
                    // Usage errors exit 2 (clap-aligned, POSIX convention).
                    // With --json, emit the error envelope on stdout so stdout
                    // stays a single JSON document for machine consumers.
                    let argv_has_json = std::env::args().any(|a| a == "--json");
                    if argv_has_json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "error": {
                                    "code": 2,
                                    "kind": "usage",
                                    "message": e.to_string(),
                                    "retryable": false,
                                }
                            })
                        );
                    } else {
                        e.print().ok();
                    }
                    std::process::exit(2);
                }
            }
        }
    };

    match &cli.command {
        Commands::Internal(cmd) => match cmd {
            InternalCmd::DeepWorker { paper_dir } => {
                commands::paper::cmd_deep_worker(&cli, paper_dir).await
            }
            InternalCmd::DownloadWorker {
                paper_dir,
                no_describe,
                no_deep,
            } => {
                commands::paper::cmd_download_worker(&cli, paper_dir, *no_describe, *no_deep).await
            }
        },
        Commands::Workspace { cmd } => match cmd {
            WorkspaceCmd::Open { path } => commands::workspace::cmd_open(&cli, path).await,
            WorkspaceCmd::Scan => commands::workspace::cmd_scan(&cli).await,
        },
        Commands::Paper { cmd } => match cmd {
            PaperCmd::List => commands::paper::cmd_list(&cli).await,
            PaperCmd::Download {
                id_or_url,
                no_describe,
                no_deep,
            } => commands::paper::cmd_download(&cli, id_or_url, *no_describe, *no_deep).await,
            PaperCmd::DownloadAll {
                jobs,
                force,
                no_describe,
                no_deep,
            } => {
                commands::paper::cmd_download_all(&cli, *jobs, *force, *no_describe, *no_deep).await
            }
            PaperCmd::Tag { cmd } => match cmd {
                TagCmd::List => commands::paper::cmd_tag_list(&cli).await,
                TagCmd::Add { id_or_query, tag } => {
                    commands::paper::cmd_tag_add(&cli, id_or_query, tag).await
                }
                TagCmd::Remove { id_or_query, tag } => {
                    commands::paper::cmd_tag_remove(&cli, id_or_query, tag).await
                }
                TagCmd::Set { id_or_query, tags } => {
                    commands::paper::cmd_tag_set(&cli, id_or_query, &tags).await
                }
                TagCmd::Clear { id_or_query } => {
                    commands::paper::cmd_tag_clear(&cli, id_or_query).await
                }
            },
            PaperCmd::Preview { id_or_query, view } => {
                commands::paper::cmd_preview(&cli, id_or_query, view).await
            }
            PaperCmd::Note {
                id_or_query,
                text,
                edit,
            } => {
                let text_str = text.join(" ");
                commands::paper::cmd_note(&cli, id_or_query, &text_str, *edit).await
            }
            PaperCmd::Strip { id_or_query } => commands::paper::cmd_strip(&cli, id_or_query).await,
            PaperCmd::Open { id_or_query } => commands::paper::cmd_open(&cli, id_or_query).await,
            PaperCmd::Pdf { id_or_query } => commands::paper::cmd_pdf(&cli, id_or_query).await,
            PaperCmd::Info { id_or_query } => commands::paper::cmd_info(&cli, id_or_query).await,
            PaperCmd::Describe { id_or_query } => {
                commands::paper::cmd_describe(&cli, id_or_query).await
            }
            PaperCmd::DeepSummarize { id_or_query, force } => {
                commands::paper::cmd_deep_summarize(&cli, id_or_query, *force).await
            }
            PaperCmd::Remove { id_or_query } => {
                commands::paper::cmd_remove(&cli, id_or_query).await
            }
            PaperCmd::Redownload { id_or_query } => {
                commands::paper::cmd_redownload(&cli, id_or_query).await
            }
        },
        Commands::Chat { cmd } => match cmd {
            ChatCmd::Side { id_or_query } => commands::chat::cmd_side(&cli, id_or_query).await,
            ChatCmd::Global => commands::chat::cmd_global(&cli).await,
        },
        Commands::Token { cmd } => match cmd {
            TokenCmd::Status => commands::token::cmd_status(&cli).await,
            TokenCmd::Set => commands::token::cmd_set(&cli).await,
            TokenCmd::Validate => commands::token::cmd_validate(&cli).await,
        },
    }
}
