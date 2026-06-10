use arxivcat_core::config;
use crate::Cli;

pub async fn cmd_open(_cli: &Cli, path: &std::path::Path) {
    if !path.exists() {
        eprintln!("error: path does not exist: {}", path.display());
        std::process::exit(1);
    }
    if !path.is_dir() {
        eprintln!("error: path is not a directory: {}", path.display());
        std::process::exit(1);
    }

    match config::save_workspace_path(path) {
        Ok(()) => println!("workspace set to: {}", path.display()),
        Err(e) => {
            eprintln!("error saving workspace path: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn cmd_scan(cli: &Cli) {
    let ws_path = crate::commands::resolve_workspace(cli);
    let ws_path = match ws_path {
        Some(p) => p,
        None => {
            eprintln!("error: no workspace configured. use 'arxivcat workspace open <path>'");
            std::process::exit(1);
        }
    };

    let mut ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error opening workspace: {e}");
            std::process::exit(1);
        }
    };

    match arxivcat_core::workspace::scan_workspace_pdfs(&mut ws).await {
        Ok(count) => println!("scanned: {} new paper folders created", count),
        Err(e) => {
            eprintln!("error scanning PDFs: {e}");
            std::process::exit(1);
        }
    }
}
