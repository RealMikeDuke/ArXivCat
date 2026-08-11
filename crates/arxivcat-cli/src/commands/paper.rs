use std::path::PathBuf;

use arxivcat_core::config;
use arxivcat_core::workspace::Workspace;
use crate::Cli;

use owo_colors::OwoColorize;

fn ok(s: &str) -> String { s.green().to_string() }
fn err(s: &str) -> String { s.red().to_string() }
fn warn(s: &str) -> String { s.yellow().to_string() }
fn gray(s: &str) -> String { s.dimmed().to_string() }

fn get_ws(cli: &Cli) -> PathBuf {
    match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no workspace configured");
        }
    }
}

fn open_ws(cli: &Cli) -> Workspace {
    let path = get_ws(cli);
    match Workspace::open(&path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    }
}

pub async fn cmd_list(cli: &Cli) {
    let ws = open_ws(cli);

    if cli.json {
        let json = serde_json::to_string_pretty(&ws.papers).unwrap_or_default();
        println!("{json}");
        return;
    }

    if ws.papers.is_empty() {
        println!("{}", gray("(empty)"));
        return;
    }

    for p in &ws.papers {
        let status = if p.is_complete {
            ok("[C]")
        } else {
            gray("[.]")
        };
        let desc = if p.description_ready {
            ok("desc")
        } else {
            gray("-")
        };
        println!("{} {:<20} {} [{}]", status, p.arxiv_id, p.title, desc);
    }
}

pub async fn cmd_download(cli: &Cli, id_or_url: &str) {
    let ws_path = get_ws(cli);
    let _ws = open_ws(cli);

    let arxiv_id = match arxivcat_core::extract::arxiv::extract_arxiv_id(id_or_url) {
        Some(id) => id,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", "could not extract arXiv ID from input");
        }
    };

    eprintln!("downloading {arxiv_id}...");

    let downloads_dir = config::get_downloads_dir();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };

    let (paper_dir_opt, folder_name_opt) =
        match arxivcat_core::extract::source::download_source(&http, &arxiv_id, &downloads_dir).await {
            Ok(r) => r,
            Err(e) => {
                crate::commands::die_err(cli, &e);
            }
        };

    let paper_dir = match paper_dir_opt {
        Some(d) => d,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "data", "source download returned None");
        }
    };

    let folder_name = folder_name_opt.unwrap_or_else(|| {
        arxiv_id.replace('.', "_")
    });

    let output_dir = ws_path.join(&folder_name);

    let output =
        match arxivcat_core::extract::tex::extract_body_from_dir(&paper_dir, &output_dir) {
            Ok(o) => o,
            Err(e) => {
                crate::commands::die_err(cli, &e);
            }
        };

    let _ = arxivcat_core::extract::source::download_pdf(&http, &arxiv_id, &output_dir).await;

    if let Err(e) = arxivcat_core::workspace::ensure_paper_meta_files(&output_dir) {
        eprintln!("{}: {e}", warn("warning creating meta files"));
    }

    if !cli.json {
        println!("{}", ok("extraction complete"));
        println!("arxiv ID: {}", &arxiv_id);
        println!("folder: {}", output_dir.display());
        println!("body: {} chars", output.body.len());
        if let Some(ref app) = output.appendix {
            println!("appendix: {} chars", app.len());
        }
    } else {
        let desc_exists = output_dir.join(".description_ready").exists();
        let json = serde_json::json!({
            "arxiv_id": arxiv_id,
            "folder": output_dir.to_string_lossy(),
            "body_length": output.body.len(),
            "appendix_length": output.appendix.as_ref().map(|a| a.len()),
            "description_ready": desc_exists,
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
}

pub async fn cmd_download_all(cli: &Cli) {
    let ws_path = get_ws(cli);
    let ws = open_ws(cli);

    let pending = ws.pending_papers();
    if pending.is_empty() {
        if cli.json {
            println!("{}", serde_json::json!({"status": "complete", "count": 0}));
        }
        return;
    }

    eprintln!("downloading {} pending papers...", pending.len());

    let downloads_dir = config::get_downloads_dir();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };
    let cancel_flag = std::sync::atomic::AtomicBool::new(false);
    let mut success = 0usize;
    let total = pending.len();

    for (i, paper) in pending.iter().enumerate() {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            println!("{}", warn("cancelled"));
            break;
        }

        eprint!(
            "\r[{}/{}] {} ...",
            i + 1,
            total,
            paper.arxiv_id
        );

        match arxivcat_core::workspace::process_pending_paper(
            &http,
            paper,
            &downloads_dir,
            &ws_path,
            &cancel_flag,
        )
        .await
        {
            Ok(true) => success += 1,
            Ok(false) => {
                eprintln!("\n{} failed: {}", warn("warn"), paper.arxiv_id);
            }
            Err(e) => {
                eprintln!("\n{} {}: {e}", err("error"), paper.arxiv_id);
        }
    }
}

    eprintln!();
    if cli.json {
        println!(
            "{}",
            serde_json::json!({"status": "done", "success": success, "total": total})
        );
    }
}

fn resolve_view_file(view: &str) -> Result<&'static str, String> {
    match view {
        "body" => Ok("body.tex"),
        "appendix" => Ok("appendix.tex"),
        "note" => Ok("note.txt"),
        "description" => Ok("description.md"),
        _ => Err(format!(
            "unknown view '{}'. options: body, appendix, note, description",
            view
        )),
    }
}

pub async fn cmd_preview(cli: &Cli, id_or_query: &str, view: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let file = match resolve_view_file(view) {
        Ok(f) => f,
        Err(e) => {
            crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", &e);
        }
    };

    let path = paper.folder.join(file);

    if cli.json {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                println!("{}", serde_json::json!({
                    "arxiv_id": paper.arxiv_id,
                    "title": paper.title,
                    "view": view,
                    "content": content,
                }));
            }
            Err(_) => {
                println!("{}", serde_json::json!({
                    "arxiv_id": paper.arxiv_id,
                    "title": paper.title,
                    "view": view,
                    "error": "file not found"
                }));
            }
        }
        return;
    }

    if !path.exists() {
        println!("{}", gray("(file not found)"));
        return;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            println!("{} {} {}{}", ok("==="), paper.arxiv_id, file, ok(" ==="));
            println!("{content}");
        }
        Err(e) => {
            crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
        }
    }
}

pub async fn cmd_note(cli: &Cli, id_or_query: &str, text: &str, edit: bool) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let note_path = paper.folder.join("note.txt");

    if edit {
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "notepad".to_string());
        let status = std::process::Command::new(&editor)
            .arg(&note_path)
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                crate::commands::die(cli, crate::commands::EXIT_OTHER, "other", &format!("editor exited with {s}"));
            }
            Err(e) => {
                crate::commands::die(cli, crate::commands::EXIT_OTHER, "other", &format!("failed to launch editor '{editor}': {e}"));
            }
        }
    } else if !text.is_empty() {
        match std::fs::write(&note_path, text) {
            Ok(()) => {
                if !cli.json {
                    println!("{}", ok("note saved"));
                }
            }
            Err(e) => {
                crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
            }
        }
    } else {
        match std::fs::read_to_string(&note_path) {
            Ok(content) => print!("{content}"),
            Err(_) => println!("{}", gray("(no note)")),
        }
    }
}

pub async fn cmd_strip(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let path = paper.folder.join("body.tex");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
        }
    };

    let stripped = arxivcat_core::extract::tex::strip_latex_comments(&content);
    let re = regex::Regex::new(r"\n{3,}").unwrap();
    let cleaned = re.replace_all(&stripped, "\n\n").to_string();
    print!("{cleaned}");
}

pub async fn cmd_open(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let _ = open::that(&paper.folder);
}

pub async fn cmd_pdf(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let glob_pattern = format!("{}/*.pdf", paper.folder.display());
    if let Ok(entries) = glob::glob(&glob_pattern) {
        if let Some(entry) = entries.flatten().next() {
            let _ = open::that(&entry);
            return;
        }
    }

    let _ = open::that(format!("https://arxiv.org/pdf/{}", paper.arxiv_id));
}

pub async fn cmd_info(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    if cli.json {
        let body_size = std::fs::metadata(paper.folder.join("body.tex"))
            .map(|m| m.len())
            .unwrap_or(0);
        let appendix_size = std::fs::metadata(paper.folder.join("appendix.tex"))
            .map(|m| m.len())
            .unwrap_or(0);
        let desc_size = std::fs::metadata(paper.folder.join("description.md"))
            .map(|m| m.len())
            .unwrap_or(0);
        let note_size = std::fs::metadata(paper.folder.join("note.txt"))
            .map(|m| m.len())
            .unwrap_or(0);

        println!(
            "{}",
            serde_json::json!({
                "arxiv_id": paper.arxiv_id,
                "title": paper.title,
                "folder": paper.folder.to_string_lossy(),
                "has_body": paper.has_body,
                "description_ready": paper.description_ready,
                "is_complete": paper.is_complete,
                "files": {
                    "body.tex": body_size,
                    "appendix.tex": appendix_size,
                    "description.md": desc_size,
                    "note.txt": note_size,
                }
            })
        );
        return;
    }

    println!("arXiv ID:       {}", paper.arxiv_id);
    println!("Title:          {}", paper.title);
    println!("Folder:         {}", paper.folder.display());
    println!(
        "Status:         {}",
        if paper.is_complete {
            ok("complete")
        } else {
            gray("incomplete")
        }
    );

    for (file, label) in &[
        ("body.tex", "body.tex"),
        ("appendix.tex", "appendix.tex"),
        ("description.md", "description.md"),
        ("note.txt", "note.txt"),
    ] {
        let path = paper.folder.join(file);
        if path.exists() {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  {:<18} {:>8} bytes", label, size);
        }
    }
}

pub async fn cmd_describe(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = match crate::commands::find_paper(&ws, id_or_query) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_DATA, "not_found", &format!("paper not found: {id_or_query}"));
        }
    };

    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };
    match arxivcat_core::chat::description::build_description(
        &http,
        &paper.folder,
        &paper.arxiv_id,
        &paper.title,
        None,
        None,
    )
    .await
    {
        Ok(()) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "arxiv_id": paper.arxiv_id,
                        "description_ready": true,
                    })
                );
            } else {
                println!("{}", ok("description generated"));
            }
        }
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_view_file_valid() {
        assert_eq!(resolve_view_file("body").unwrap(), "body.tex");
        assert_eq!(resolve_view_file("appendix").unwrap(), "appendix.tex");
        assert_eq!(resolve_view_file("note").unwrap(), "note.txt");
        assert_eq!(resolve_view_file("description").unwrap(), "description.md");
    }

    #[test]
    fn test_resolve_view_file_invalid() {
        assert!(resolve_view_file("invalid").is_err());
        assert!(resolve_view_file("").is_err());
        assert!(resolve_view_file("BODY").is_err());
    }
}
