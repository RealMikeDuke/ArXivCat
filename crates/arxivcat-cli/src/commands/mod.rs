pub mod chat;
pub mod paper;
pub mod token;
pub mod workspace;

use arxivcat_core::config;
use arxivcat_core::workspace::Workspace;
use crate::Cli;

pub fn resolve_workspace(cli: &Cli) -> Option<std::path::PathBuf> {
    if let Some(ref ws) = cli.workspace {
        if ws.exists() {
            return Some(ws.clone());
        }
    }
    if let Some(path) = config::load_workspace_path() {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn find_paper(workspace: &Workspace, query: &str) -> Option<arxivcat_core::workspace::Paper> {
    workspace.find_paper_by_id(query).cloned()
}
