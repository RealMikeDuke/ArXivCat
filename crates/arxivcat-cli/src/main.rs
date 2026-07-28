mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "arxivcat", about = "ArXiv paper extraction and chat tool", version)]
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
}

#[derive(Subcommand)]
pub enum WorkspaceCmd {
    #[command(about = "Open a workspace folder")]
    Open { path: PathBuf },

    #[command(about = "Scan workspace root for PDFs and create paper folders")]
    Scan,
}

#[derive(Subcommand)]
pub enum PaperCmd {
    #[command(about = "List all papers in workspace")]
    List,

    #[command(about = "Download and extract a single paper")]
    Download { id_or_url: String },

    #[command(about = "Download all pending papers in workspace")]
    DownloadAll,

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
}

#[derive(Subcommand)]
pub enum ChatCmd {
    #[command(about = "Start side chat scoped to one paper")]
    Side { id_or_query: String },

    #[command(about = "Start global chat over all workspace descriptions")]
    Global,
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
    let cli = Cli::parse();

    match &cli.command {
        Commands::Workspace { cmd } => match cmd {
            WorkspaceCmd::Open { path } => commands::workspace::cmd_open(&cli, path).await,
            WorkspaceCmd::Scan => commands::workspace::cmd_scan(&cli).await,
        },
        Commands::Paper { cmd } => match cmd {
            PaperCmd::List => commands::paper::cmd_list(&cli).await,
            PaperCmd::Download { id_or_url } => {
                commands::paper::cmd_download(&cli, id_or_url).await
            }
            PaperCmd::DownloadAll => commands::paper::cmd_download_all(&cli).await,
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
            PaperCmd::Strip { id_or_query } => {
                commands::paper::cmd_strip(&cli, id_or_query).await
            }
            PaperCmd::Open { id_or_query } => {
                commands::paper::cmd_open(&cli, id_or_query).await
            }
            PaperCmd::Pdf { id_or_query } => {
                commands::paper::cmd_pdf(&cli, id_or_query).await
            }
            PaperCmd::Info { id_or_query } => {
                commands::paper::cmd_info(&cli, id_or_query).await
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
