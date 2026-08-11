use arxivcat_core::config;
use crate::Cli;

pub async fn cmd_open(cli: &Cli, path: &std::path::Path) {
    if !path.exists() {
        crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", &format!("path does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", &format!("path is not a directory: {}", path.display()));
    }

    match config::save_workspace_path(path) {
        Ok(()) => println!("workspace set to: {}", path.display()),
        Err(e) => {
            crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
        }
    }
}

pub async fn cmd_scan(cli: &Cli) {
    let ws_path = crate::commands::resolve_workspace(cli);
    let ws_path = match ws_path {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no workspace configured. use 'arxivcat workspace open <path>'");
        }
    };

    let mut ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    match arxivcat_core::workspace::scan_workspace_pdfs(&mut ws).await {
        Ok(count) => {
            if cli.json {
                println!("{}", serde_json::json!({"scanned": count}));
            } else {
                println!("scanned: {count} new paper folders created");
            }
        }
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    }
}
