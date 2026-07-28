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
        eprintln!("\u{1b}[31merror\u{1b}[39m: workspace not found: {}", ws.display());
        return None;
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
    if let Some(p) = workspace.find_paper_by_id(query) {
        return Some(p.clone());
    }
    if let Some(id) = arxivcat_core::extract::arxiv::extract_arxiv_id(query) {
        return workspace.find_paper_by_id(&id).cloned();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace_with_paper() -> arxivcat_core::workspace::Workspace {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test_Paper");
        std::fs::create_dir_all(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), r"\documentclass{article}\begin{document}test\end{document}").unwrap();
        std::fs::write(paper_dir.join("note.txt"), "").unwrap();
        arxivcat_core::workspace::Workspace::open(dir.path()).unwrap()
    }

    #[test]
    fn test_find_paper_direct_id() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "2501.12948");
        assert!(p.is_some());
    }

    #[test]
    fn test_find_paper_url_abs() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "https://arxiv.org/abs/2501.12948");
        assert!(p.is_some());
    }

    #[test]
    fn test_find_paper_url_pdf() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "https://arxiv.org/pdf/2501.12948.pdf");
        assert!(p.is_some());
    }

    #[test]
    fn test_find_paper_url_with_www() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "www.arxiv.org/abs/2501.12948");
        assert!(p.is_some());
    }

    #[test]
    fn test_find_paper_url_versioned() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "https://arxiv.org/abs/2501.12948v2");
        assert!(p.is_some());
    }

    #[test]
    fn test_find_paper_nonexistent() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "nonexistent");
        assert!(p.is_none());
    }

    #[test]
    fn test_find_paper_url_with_whitespace() {
        let ws = make_workspace_with_paper();
        let p = find_paper(&ws, "  https://arxiv.org/abs/2501.12948  ");
        assert!(p.is_some());
    }

    // ─── resolve_workspace ───

    fn make_cli(workspace: Option<std::path::PathBuf>) -> crate::Cli {
        crate::Cli {
            workspace,
            json: false,
            command: crate::Commands::Paper { cmd: crate::PaperCmd::List },
        }
    }

    #[test]
    fn test_resolve_workspace_w_flag_valid() {
        let dir = tempfile::tempdir().unwrap();
        let cli = make_cli(Some(dir.path().to_path_buf()));
        assert_eq!(resolve_workspace(&cli), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn test_resolve_workspace_w_flag_invalid() {
        let cli = make_cli(Some(std::path::PathBuf::from("/__nonexistent_xyz__")));
        assert_eq!(resolve_workspace(&cli), None);
    }

}
