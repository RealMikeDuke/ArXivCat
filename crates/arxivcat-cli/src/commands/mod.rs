pub mod chat;
pub mod paper;
pub mod token;
pub mod workspace;

use crate::Cli;
use arxivcat_core::config;
use arxivcat_core::error::ArxivError;
use arxivcat_core::workspace::Workspace;

// ─── Error contract (P0.4, frozen at P0 gate — do not renumber) ───
// 0 success | 1 other | 2 usage | 3 network | 4 config | 5 data | 6 io | 7 chat | 8 partial | 130 SIGINT
#[allow(dead_code)] // success; implicit in command completion
pub const EXIT_OK: i32 = 0;
pub const EXIT_OTHER: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_NETWORK: i32 = 3;
pub const EXIT_CONFIG: i32 = 4;
pub const EXIT_DATA: i32 = 5;
pub const EXIT_IO: i32 = 6;
pub const EXIT_CHAT: i32 = 7;
#[allow(dead_code)] // reserved: download-all partial aggregation lands in P1.4
pub const EXIT_PARTIAL: i32 = 8;

pub fn exit_code_for(err: &ArxivError) -> i32 {
    use ArxivError::*;
    match err {
        Io(_) => EXIT_IO,
        Http(_) => EXIT_NETWORK,
        Parse(_) | Extraction(_) | NotFound(_) | Json(_) => EXIT_DATA,
        Chat(_) => EXIT_CHAT,
        Config(_) => EXIT_CONFIG,
        Other(_) => EXIT_OTHER,
    }
}

pub fn kind_for(err: &ArxivError) -> &'static str {
    use ArxivError::*;
    match err {
        Io(_) => "io",
        Http(_) => "http",
        Parse(_) => "parse",
        Extraction(_) => "extraction",
        Chat(_) => "chat",
        Config(_) => "config",
        NotFound(_) => "not_found",
        Json(_) => "json",
        Other(_) => "other",
    }
}

pub fn retryable_for(kind: &str, message: &str) -> bool {
    match kind {
        "http" => true,
        "chat" => !message.contains("401") && !message.contains("403"),
        _ => false,
    }
}

/// Uniform error exit. In --json mode the envelope goes to stdout so stdout
/// is always a single JSON document (payload or error); otherwise human text
/// goes to stderr.
pub fn die(cli: &Cli, code: i32, kind: &str, message: &str) -> ! {
    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "error": {
                    "code": code,
                    "kind": kind,
                    "message": message,
                    "retryable": retryable_for(kind, message),
                }
            })
        );
    } else {
        eprintln!("error[{code}]: {message}");
    }
    std::process::exit(code);
}

pub fn die_err(cli: &Cli, err: &ArxivError) -> ! {
    let code = exit_code_for(err);
    let kind = kind_for(err);
    die(cli, code, kind, &err.to_string());
}

pub fn resolve_workspace(cli: &Cli) -> Option<std::path::PathBuf> {
    if let Some(ref ws) = cli.workspace {
        if ws.exists() {
            return Some(ws.clone());
        }
        // Caller (get_ws) reports via the uniform error envelope.
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

pub fn find_papers(workspace: &Workspace, query: &str) -> Vec<arxivcat_core::workspace::Paper> {
    let mut found: Vec<arxivcat_core::workspace::Paper> = Vec::new();
    for p in workspace.find_papers_by_id(query) {
        if !found.iter().any(|f| f.folder_name == p.folder_name) {
            found.push(p.clone());
        }
    }
    if let Some(id) = arxivcat_core::extract::arxiv::extract_arxiv_id(query) {
        for p in workspace.find_papers_by_id(&id) {
            if !found.iter().any(|f| f.folder_name == p.folder_name) {
                found.push(p.clone());
            }
        }
    }
    found
}

/// Resolve a paper, dying with a clear error on not-found or ambiguity.
/// Silent wrong-paper selection (P0.14) is never allowed.
pub fn find_paper_or_die(
    cli: &Cli,
    workspace: &Workspace,
    query: &str,
) -> arxivcat_core::workspace::Paper {
    let matches = find_papers(workspace, query);
    match matches.len() {
        0 => die(
            cli,
            EXIT_DATA,
            "not_found",
            &format!("paper not found: {query}"),
        ),
        1 => matches.into_iter().next().unwrap(),
        n => {
            let ids: Vec<String> = matches.iter().map(|p| p.arxiv_id.clone()).collect();
            die(
                cli,
                EXIT_DATA,
                "ambiguous",
                &format!(
                    "query '{query}' matches {n} papers: {}. Use the full arXiv ID.",
                    ids.join(", ")
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace_with_paper() -> arxivcat_core::workspace::Workspace {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test_Paper");
        std::fs::create_dir_all(&paper_dir).unwrap();
        std::fs::write(
            paper_dir.join("body.tex"),
            r"\documentclass{article}\begin{document}test\end{document}",
        )
        .unwrap();
        std::fs::write(paper_dir.join("note.txt"), "").unwrap();
        arxivcat_core::workspace::Workspace::open(dir.path()).unwrap()
    }

    #[test]
    fn test_find_paper_direct_id() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "2501.12948");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_find_paper_url_abs() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "https://arxiv.org/abs/2501.12948");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_find_paper_url_pdf() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "https://arxiv.org/pdf/2501.12948.pdf");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_find_paper_url_with_www() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "www.arxiv.org/abs/2501.12948");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_find_paper_url_versioned() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "https://arxiv.org/abs/2501.12948v2");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_find_paper_nonexistent() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "nonexistent");
        assert!(p.is_empty());
    }

    #[test]
    fn test_find_paper_url_with_whitespace() {
        let ws = make_workspace_with_paper();
        let p = find_papers(&ws, "  https://arxiv.org/abs/2501.12948  ");
        assert_eq!(p.len(), 1);
    }

    // ─── resolve_workspace ───

    fn make_cli(workspace: Option<std::path::PathBuf>) -> crate::Cli {
        crate::Cli {
            workspace,
            json: false,
            command: crate::Commands::Paper {
                cmd: crate::PaperCmd::List,
            },
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
