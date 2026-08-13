use std::path::PathBuf;

use crate::Cli;
use arxivcat_core::config;
use arxivcat_core::workspace::Workspace;

use owo_colors::OwoColorize;

fn ok(s: &str) -> String {
    s.green().to_string()
}
fn warn(s: &str) -> String {
    s.yellow().to_string()
}
fn gray(s: &str) -> String {
    s.dimmed().to_string()
}

fn get_ws(cli: &Cli) -> PathBuf {
    match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                "no workspace configured",
            );
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
            crate::commands::die(
                cli,
                crate::commands::EXIT_USAGE,
                "usage",
                "could not extract arXiv ID from input",
            );
        }
    };

    eprintln!("downloading {arxiv_id}...");

    let downloads_dir = config::get_downloads_dir();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };

    let (paper_dir_opt, folder_name_opt) =
        match arxivcat_core::extract::source::download_source(&http, &arxiv_id, &downloads_dir)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                crate::commands::die_err(cli, &e);
            }
        };

    let paper_dir = match paper_dir_opt {
        Some(d) => d,
        None => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_DATA,
                "data",
                "source download returned None",
            );
        }
    };

    let folder_name = folder_name_opt.unwrap_or_else(|| arxiv_id.replace('.', "_"));

    let output_dir = ws_path.join(&folder_name);

    let output = match arxivcat_core::extract::tex::extract_body_from_dir(&paper_dir, &output_dir) {
        Ok(o) => o,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    for w in &output.warnings {
        eprintln!("warning: {w}");
    }

    let _ = arxivcat_core::extract::source::download_pdf(&http, &arxiv_id, &output_dir).await;

    if let Err(e) = arxivcat_core::workspace::ensure_paper_meta_files(&output_dir) {
        eprintln!("{}: {e}", warn("warning creating meta files"));
    }

    // Single-download must also write the manifest (P1.1 write-path rule).
    // Backfill the title best-effort so `paper list` shows a real title
    // right after a fresh download (previously always empty). Use the
    // export API (P2-1) instead of an extra abs-page request — one batch
    // call, no 429 exposure.
    let title =
        arxivcat_core::extract::arxiv::fetch_titles_batch(&http, std::slice::from_ref(&arxiv_id))
            .await
            .get(&arxiv_id)
            .cloned()
            .unwrap_or_default();
    let _ = arxivcat_core::manifest::refresh_manifest(&output_dir, &arxiv_id, &title);

    if !cli.json {
        println!("{}", ok("extraction complete"));
        println!("arxiv ID: {arxiv_id}");
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

pub async fn cmd_download_all(cli: &Cli, jobs: u8, force: bool) {
    let ws_path = get_ws(cli);
    let ws = open_ws(cli);
    let downloads_dir = config::get_downloads_dir();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };

    // Defensive clamp — clap already validates 1..=8, but keep this as a
    // library-facing guard (P3-3 known-issue; restored after jury-review).
    let jobs = jobs.clamp(1, 8);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Split pending papers into actionable vs cooled-down (24h cooldown).
    let mut pending: Vec<arxivcat_core::workspace::Paper> =
        ws.papers.iter().filter(|p| !p.has_body).cloned().collect();
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    pending.retain(|p| {
        if let Ok(Some(m)) = arxivcat_core::manifest::PaperManifest::load(&p.folder) {
            if arxivcat_core::manifest::in_cooldown(&m, now_ms) && !force {
                skipped.push(serde_json::json!({
                    "id": p.arxiv_id,
                    "reason": "cooldown",
                    "last_error": m.last_error,
                }));
                return false;
            }
        }
        true
    });

    if pending.is_empty() {
        if cli.json {
            println!(
                "{}",
                serde_json::json!({
                    "status": "done",
                    "total": 0,
                    "success": 0,
                    "failed": 0,
                    "skipped": skipped.len(),
                    "failures": [],
                })
            );
        }
        return;
    }

    eprintln!(
        "downloading {} pending papers (jobs={jobs})...",
        pending.len()
    );

    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Real Ctrl-C: flip the flag; workers observe it and the command exits 130.
    {
        let cancel = cancel_flag.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    }

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(jobs as usize));
    let mut handles = Vec::new();
    let total = pending.len();

    for paper in pending {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            drop(permit);
            break;
        }

        let ws_path = ws_path.clone();
        let downloads_dir = downloads_dir.clone();
        let http = http.clone();
        let cancel = cancel_flag.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let res = arxivcat_core::workspace::process_pending_paper(
                &http,
                &paper,
                &downloads_dir,
                &ws_path,
                &cancel,
            )
            .await;
            (paper, res)
        }));
    }

    let mut success = 0usize;
    let mut failures: Vec<serde_json::Value> = Vec::new();
    for h in handles {
        if let Ok((paper, res)) = h.await {
            match res {
                Ok(true) => success += 1,
                Ok(false) => {
                    // cancelled mid-flight; not counted as failure
                }
                Err(e) => {
                    let code = crate::commands::exit_code_for(&e);
                    let kind = crate::commands::kind_for(&e);
                    let msg = e.to_string();
                    // Keep the paper's identity in the manifest first, then
                    // arm the 24h cooldown (C2: legacy folders must not get
                    // an empty-ID paper.json on failure).
                    let _ = arxivcat_core::manifest::refresh_manifest(
                        &paper.folder,
                        &paper.arxiv_id,
                        &paper.title,
                    );
                    let _ = arxivcat_core::manifest::mark_failure(&paper.folder, &msg);
                    eprintln!("{} failed: {msg}", paper.arxiv_id);
                    failures.push(serde_json::json!({
                        "id": paper.arxiv_id,
                        "code": code,
                        "kind": kind,
                        "message": msg,
                        "retryable": crate::commands::retryable_for(kind, &msg),
                    }));
                }
            }
        }
    }

    let cancelled = cancel_flag.load(std::sync::atomic::Ordering::Relaxed);

    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "status": if cancelled { "cancelled" } else if failures.is_empty() { "done" } else if success > 0 { "partial" } else { "failed" },
                "total": total,
                "success": success,
                "failed": failures.len(),
                "skipped": skipped.len(),
                "failures": failures,
            })
        );
    }

    // Exit contract: 0 all ok, 8 partial (some failed), 1 all failed, 130 SIGINT.
    if cancelled {
        std::process::exit(130);
    } else if !failures.is_empty() && success > 0 {
        std::process::exit(crate::commands::EXIT_PARTIAL);
    } else if !failures.is_empty() {
        std::process::exit(crate::commands::EXIT_OTHER);
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
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

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
                println!(
                    "{}",
                    serde_json::json!({
                        "arxiv_id": paper.arxiv_id,
                        "title": paper.title,
                        "view": view,
                        "content": content,
                    })
                );
            }
            Err(_) => {
                crate::commands::die(
                    cli,
                    crate::commands::EXIT_DATA,
                    "not_found",
                    &format!("file not found: {view} ({})", path.display()),
                );
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
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for paper note",
        );
    }
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    let note_path = paper.folder.join("note.txt");

    if edit {
        let default_editor = if cfg!(windows) { "notepad" } else { "vi" };
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| default_editor.to_string());
        let status = std::process::Command::new(&editor).arg(&note_path).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                crate::commands::die(
                    cli,
                    crate::commands::EXIT_OTHER,
                    "other",
                    &format!("editor exited with {s}"),
                );
            }
            Err(e) => {
                crate::commands::die(
                    cli,
                    crate::commands::EXIT_OTHER,
                    "other",
                    &format!("failed to launch editor '{editor}': {e}"),
                );
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
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for paper strip",
        );
    }
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

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
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    let _ = open::that(&paper.folder);
}

pub async fn cmd_pdf(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

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
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

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
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

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
            // Lazy migration (P1.1): describe is a write-path command; refresh
            // the manifest so description_ready is durable.
            let _ = arxivcat_core::manifest::refresh_manifest(
                &paper.folder,
                &paper.arxiv_id,
                &paper.title,
            );
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

pub async fn cmd_remove(cli: &Cli, id_or_query: &str) {
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    match std::fs::remove_dir_all(&paper.folder) {
        Ok(()) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({"removed": paper.arxiv_id, "folder": paper.folder_name})
                );
            } else {
                println!("removed {} ({})", paper.arxiv_id, paper.folder_name);
            }
        }
        Err(e) => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_IO,
                "io",
                &format!("failed to remove {}: {e}", paper.folder.display()),
            );
        }
    }
}

pub async fn cmd_redownload(cli: &Cli, id_or_query: &str) {
    let _ws_path = get_ws(cli); // validates workspace config
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    let folder = paper.folder.clone();
    // Preserve user metadata across the re-download (P1.5). Timestamped so
    // a stale /tmp dir from a crashed run can never collide and silently
    // lose the backup (rename-to-existing fails).
    let backup = std::env::temp_dir().join(format!(
        "arxivcat_redl_{}_{}_{}",
        std::process::id(),
        paper.folder_name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let _ = std::fs::create_dir_all(&backup);
    // .description_ready must be preserved too, else redownload silently
    // loses the description_ready state (P1.5 regression).
    let mut backup_failed = false;
    for name in [
        "note.txt",
        "description.md",
        "arxiv_chats",
        ".description_ready",
    ] {
        let src = folder.join(name);
        if src.exists() && std::fs::rename(&src, backup.join(name)).is_err() {
            backup_failed = true;
        }
    }
    if backup_failed {
        crate::commands::die(
            cli,
            crate::commands::EXIT_IO,
            "io",
            "metadata backup failed — aborting re-download to protect data",
        );
    }

    match std::fs::remove_dir_all(&folder) {
        Ok(()) => {}
        Err(e) => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_IO,
                "io",
                &format!("failed to remove {}: {e}", folder.display()),
            );
        }
    }

    let downloads_dir = config::get_downloads_dir();
    // "redownload" must actually hit the network — the plain download path
    // is cache-first and would otherwise just re-extract the local copy,
    // never picking up a newer arXiv version (T0 P2).
    let cache_dir = downloads_dir.join(&paper.folder_name);
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };

    let result = async {
        let (dir_opt, _) =
            arxivcat_core::extract::source::download_source(&http, &paper.arxiv_id, &downloads_dir)
                .await?;
        let dir = dir_opt.ok_or_else(|| {
            arxivcat_core::error::ArxivError::Other("source download returned nothing".into())
        })?;
        arxivcat_core::extract::tex::extract_body_from_dir(&dir, &folder)?;
        let _ = arxivcat_core::extract::source::download_pdf(&http, &paper.arxiv_id, &folder).await;
        arxivcat_core::workspace::ensure_paper_meta_files(&folder)?;
        arxivcat_core::manifest::refresh_manifest(&folder, &paper.arxiv_id, &paper.title)?;
        Ok::<_, arxivcat_core::error::ArxivError>(())
    }
    .await;

    // Restore metadata even on failure. Recreate the folder first so a
    // failed download (folder removed) still gets its meta back; remove
    // freshly-created stubs before restoring real content.
    let _ = std::fs::create_dir_all(&folder);
    for name in [
        "note.txt",
        "description.md",
        "arxiv_chats",
        ".description_ready",
    ] {
        let src = backup.join(name);
        if src.exists() {
            let dst = folder.join(name);
            let _ = std::fs::remove_dir_all(&dst); // drop stub created by ensure_*
            let _ = std::fs::rename(&src, dst);
        }
    }
    // Refresh AFTER the meta restore so description_ready reflects the
    // restored flag file (previously it stayed stale until the next scan).
    let _ = arxivcat_core::manifest::refresh_manifest(&folder, &paper.arxiv_id, &paper.title);
    let _ = std::fs::remove_dir_all(&backup);

    match result {
        Ok(()) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({"redownloaded": paper.arxiv_id, "folder": paper.folder_name})
                );
            } else {
                println!("re-downloaded {} ({})", paper.arxiv_id, paper.folder_name);
            }
        }
        Err(e) => crate::commands::die_err(cli, &e),
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
