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

/// Best-effort automatic brief generation (round 1): missing key or any
/// generation failure is logged to stderr and ignored — the download result
/// and exit code never depend on it.
async fn auto_brief(
    http: &arxivcat_core::net::HttpConfig,
    paper_dir: &std::path::Path,
    arxiv_id: &str,
    title: &str,
) {
    if let Err(e) = arxivcat_core::chat::description::build_description(
        http, paper_dir, arxiv_id, title, None, None,
    )
    .await
    {
        eprintln!("warning: brief generation failed for {arxiv_id}: {e}");
    }
}

/// Best-effort automatic deep recap (round 2): same best-effort contract as
/// the brief — failure never affects the download result or exit code.
async fn auto_deep(
    http: &arxivcat_core::net::HttpConfig,
    paper_dir: &std::path::Path,
    arxiv_id: &str,
    title: &str,
) {
    if let Err(e) =
        arxivcat_core::chat::summary::generate_deep(http, paper_dir, arxiv_id, title).await
    {
        eprintln!("warning: deep summary generation failed for {arxiv_id}: {e}");
    }
}

pub async fn cmd_download(cli: &Cli, id_or_url: &str, no_describe: bool, no_deep: bool) {
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
            // Map keys are normalized to base id — lookup must be too, or a
            // versioned input (2501.12948v2) silently misses (jury-burst R2).
            .get(&arxivcat_core::manifest::strip_version(&arxiv_id))
            .cloned()
            .unwrap_or_default();
    let _ = arxivcat_core::manifest::refresh_manifest(&output_dir, &arxiv_id, &title);

    // Default-on automatic brief (disable with --no-describe).
    if !no_describe {
        auto_brief(&http, &output_dir, &arxiv_id, &title).await;
    }

    // Default-on automatic deep recap (disable with --no-deep). Runs after
    // the brief so round 2 hits the prefix cache; single downloads wait.
    if !no_deep {
        auto_deep(&http, &output_dir, &arxiv_id, &title).await;
    }

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
        let deep_exists = output_dir.join(".deep_ready").exists();
        let json = serde_json::json!({
            "arxiv_id": arxiv_id,
            "folder": output_dir.to_string_lossy(),
            "body_length": output.body.len(),
            "appendix_length": output.appendix.as_ref().map(|a| a.len()),
            "description_ready": desc_exists,
            "deep_ready": deep_exists,
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
}

/// Full download pipeline as an independent process (spawned by
/// download-all). Emits line-delimited JSON events on stdout:
/// downloading / downloaded / brief_done / deep_spawned / done / failed.
/// Exit code 0 = paper ready, non-zero = failed (contract codes).
pub async fn cmd_download_worker(_cli: &Cli, paper_dir: &str, no_describe: bool, no_deep: bool) {
    use arxivcat_core::manifest::PaperManifest;
    let dir = std::path::Path::new(paper_dir);

    // Manifest is the single source of truth, but legacy/raw pending folders
    // may not have one yet — fall back to folder-name parsing so the worker
    // can bootstrap the first download (refresh_manifest writes it after).
    let (arxiv_id, title) = match PaperManifest::load(dir) {
        Ok(Some(m)) => (m.arxiv_id, m.title),
        _ => {
            let folder_name = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            arxivcat_core::manifest::parse_legacy_folder(&folder_name)
        }
    };

    let ws_path = dir.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let downloads_dir = config::get_downloads_dir();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("download-worker: {e}");
            std::process::exit(4);
        }
    };

    let emit = |name: &str| {
        println!(
            "{}",
            serde_json::json!({
                "event": name,
                "arxiv_id": arxiv_id,
            })
        );
    };

    let folder_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let paper = arxivcat_core::workspace::Paper {
        arxiv_id: arxiv_id.clone(),
        title: title.clone(),
        folder_name: folder_name.clone(),
        folder: dir.to_path_buf(),
        has_body: dir.join("body.tex").exists(),
        description_ready: false,
        deep_ready: false,
        is_complete: false,
    };

    let cancel = std::sync::atomic::AtomicBool::new(false);
    let ev = |name: &str| emit(name);

    let result = arxivcat_core::workspace::process_pending_paper(
        &http,
        &paper,
        &downloads_dir,
        &ws_path,
        &cancel,
        Some(&ev),
    )
    .await;

    match result {
        Ok(true) | Ok(false) => {
            if !no_describe {
                auto_brief(&http, dir, &arxiv_id, &title).await;
                emit("brief_done");
            }
            if !no_deep {
                spawn_deep_worker(dir);
                emit("deep_spawned");
            }
            emit("done");
        }
        Err(e) => {
            let msg = e.to_string();
            // Keep identity in manifest, arm the 24h cooldown (C2).
            let _ = arxivcat_core::manifest::refresh_manifest(dir, &arxiv_id, &title);
            let _ = arxivcat_core::manifest::mark_failure(dir, &msg);
            let code = crate::commands::exit_code_for(&e);
            let kind = crate::commands::kind_for(&e);
            println!(
                "{}",
                serde_json::json!({
                    "event": "failed",
                    "arxiv_id": arxiv_id,
                    "code": code,
                    "kind": kind,
                    "message": msg,
                    "retryable": crate::commands::retryable_for(kind, &msg),
                })
            );
            std::process::exit(code);
        }
    }
}

/// Batch download as a PROCESS SCHEDULER: spawns one independent
/// `internal download-worker` process per pending paper (from the first
/// download step), up to `jobs` at a time, reads line-delimited JSON events
/// from each worker's stdout pipe for live progress, then aggregates.
pub async fn cmd_download_all(cli: &Cli, jobs: u8, force: bool, no_describe: bool, no_deep: bool) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command as TokioCommand;

    let ws = open_ws(cli);

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

    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let children: std::sync::Arc<std::sync::Mutex<Vec<tokio::process::Child>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    // Real Ctrl-C: kill every worker process, then exit 130.
    {
        let cancelled = cancelled.clone();
        let children = children.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            for c in children.lock().unwrap().iter_mut() {
                let _ = c.start_kill();
            }
        });
    }

    let exe = std::env::current_exe().expect("cannot locate own executable");
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(jobs as usize));
    let mut handles = Vec::new();
    let total = pending.len();

    for paper in pending {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            drop(permit);
            break;
        }

        let exe = exe.clone();
        let children = children.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let mut cmd = TokioCommand::new(&exe);
            cmd.arg("internal")
                .arg("download-worker")
                .arg(&paper.folder);
            if no_describe {
                cmd.arg("--no-describe");
            }
            if no_deep {
                cmd.arg("--no-deep");
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::inherit());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let aid = paper.arxiv_id.clone();
                    return (
                        paper,
                        false,
                        Some(serde_json::json!({
                            "id": aid,
                            "code": crate::commands::EXIT_IO,
                            "kind": "io",
                            "message": format!("cannot spawn download-worker: {e}"),
                            "retryable": false,
                        })),
                    );
                }
            };

            let stdout = match child.stdout.take() {
                Some(o) => o,
                None => {
                    let _ = child.start_kill();
                    let aid = paper.arxiv_id.clone();
                    return (
                        paper,
                        false,
                        Some(serde_json::json!({
                            "id": aid,
                            "code": crate::commands::EXIT_IO,
                            "kind": "io",
                            "message": "no stdout pipe from worker".to_string(),
                            "retryable": false,
                        })),
                    );
                }
            };

            children.lock().unwrap().push(child);

            let mut lines = BufReader::new(stdout).lines();
            let mut done = false;
            let mut failed: Option<serde_json::Value> = None;
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    match v["event"].as_str() {
                        Some("done") => done = true,
                        Some("failed") => failed = Some(v),
                        Some("downloaded") => {
                            eprintln!("  [{}] downloaded", v["arxiv_id"].as_str().unwrap_or("?"))
                        }
                        _ => {}
                    }
                }
            }
            (paper, done, failed)
        }));
    }

    let mut success = 0usize;
    let mut failures: Vec<serde_json::Value> = Vec::new();
    for h in handles {
        if let Ok((paper, done, failed)) = h.await {
            match failed {
                Some(f) => failures.push(f),
                None if done => success += 1,
                None => {
                    // Worker exited without a terminal event (crash/kill).
                    failures.push(serde_json::json!({
                        "id": paper.arxiv_id,
                        "code": crate::commands::EXIT_OTHER,
                        "kind": "other",
                        "message": "download-worker exited without a result event".to_string(),
                        "retryable": false,
                    }));
                }
            }
        }
    }

    let cancelled = cancelled.load(std::sync::atomic::Ordering::Relaxed);

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
    } else {
        eprintln!(
            "done: {} ok, {} failed, {} skipped",
            success,
            failures.len(),
            skipped.len()
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
        // `description` is kept as an alias for brief_summary.md (contract
        // compatibility); `brief` is the canonical name.
        "description" | "brief" => Ok("brief_summary.md"),
        "deep" => Ok("deep_summary.md"),
        _ => Err(format!(
            "unknown view '{}'. options: body, appendix, note, description|brief, deep",
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
        ("brief_summary.md", "brief_summary.md"),
        ("deep_summary.md", "deep_summary.md"),
        ("note.txt", "note.txt"),
    ] {
        let path = paper.folder.join(file);
        if path.exists() {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  {:<18} {:>8} bytes", label, size);
        }
    }
}

/// Spawn a DETACHED worker to generate the deep recap for a paper dir.
/// Never awaited: the caller moves on, the worker finishes in its own
/// process group (survives Ctrl-C of the parent) and writes
/// deep_summary.md + .deep_ready. stdout/stderr go to `.deep.log`.
fn spawn_deep_worker(paper_dir: &std::path::Path) {
    // Already done or already queued — never double-spawn.
    if paper_dir.join(".deep_ready").exists() {
        return;
    }
    if paper_dir.join(".deep.lock").exists() {
        return;
    }
    let _ = std::fs::write(
        paper_dir.join(".deep.lock"),
        format!("{}\n", std::process::id()),
    );

    let Some(exe) = std::env::current_exe().ok() else {
        eprintln!("warning: cannot locate own executable, skipping deep worker");
        return;
    };
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("internal").arg("deep-worker").arg(paper_dir);

    // Own process group so parent Ctrl-C (SIGINT to the foreground group)
    // does not kill in-flight workers.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    // Never inherit the parent's stdout/stderr: a held pipe would block the
    // caller until EOF (contract pollution / apparent hang). Log instead.
    cmd.stdin(std::process::Stdio::null());
    let log_path = paper_dir.join(".deep.log");
    match std::fs::File::create(&log_path) {
        Ok(f) => {
            if let Ok(dup) = f.try_clone() {
                cmd.stdout(std::process::Stdio::from(f));
                cmd.stderr(std::process::Stdio::from(dup));
            } else {
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::null());
            }
        }
        Err(_) => {
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
        }
    }

    if let Err(e) = cmd.spawn() {
        eprintln!(
            "warning: failed to spawn deep worker for {}: {e}",
            paper_dir.display()
        );
    }
}

/// Detached worker entry (`arxivcat internal deep-worker <paper_dir>`).
/// Reads arxiv_id/title from the manifest so user1 stays byte-identical to
/// the brief that built the prefix cache; writes deep_summary.md + tables,
/// sets .deep_ready, refreshes the manifest. Exit code 0 = done, non-zero =
/// failure (see .deep.log).
pub async fn cmd_deep_worker(cli: &Cli, paper_dir: &str) {
    use arxivcat_core::manifest::PaperManifest;
    let dir = std::path::Path::new(paper_dir);
    let m = match PaperManifest::load(dir) {
        Ok(Some(m)) => m,
        Ok(None) => {
            eprintln!("deep-worker: no manifest at {paper_dir}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("deep-worker: {e}");
            std::process::exit(1);
        }
    };

    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("deep-worker: {e}");
            std::process::exit(4);
        }
    };

    if let Err(e) =
        arxivcat_core::chat::summary::generate_deep(&http, dir, &m.arxiv_id, &m.title).await
    {
        eprintln!("deep-worker: {e}");
        std::process::exit(7); // chat upstream
    }

    let _ = arxivcat_core::manifest::refresh_manifest(dir, &m.arxiv_id, &m.title);
    let _ = std::fs::remove_file(dir.join(".deep.lock"));
    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "arxiv_id": m.arxiv_id,
                "deep_ready": true,
            })
        );
    }
}

/// `paper deep-summarize <id> [--force]`: generate the deep recap in the
/// foreground (explicit command — waiting is correct here). Rebuilds the
/// brief first if missing (generate_deep does that internally).
pub async fn cmd_deep_summarize(cli: &Cli, id_or_query: &str, force: bool) {
    let ws = open_ws(cli);
    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    if force {
        let _ = std::fs::remove_file(paper.folder.join(".deep_ready"));
        let _ = std::fs::remove_file(paper.folder.join("deep_summary.md"));
    }

    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => crate::commands::die_err(cli, &e),
    };
    match arxivcat_core::chat::summary::generate_deep(
        &http,
        &paper.folder,
        &paper.arxiv_id,
        &paper.title,
    )
    .await
    {
        Ok(()) => {
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
                        "deep_ready": true,
                    })
                );
            } else {
                println!(
                    "deep summary generated: {}",
                    paper.folder.join("deep_summary.md").display()
                );
            }
        }
        Err(e) => crate::commands::die_err(cli, &e),
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
        "brief_summary.md",
        "deep_summary.md",
        "arxiv_chats",
        ".description_ready",
        ".deep_ready",
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
        "brief_summary.md",
        "deep_summary.md",
        "arxiv_chats",
        ".description_ready",
        ".deep_ready",
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
        assert_eq!(
            resolve_view_file("description").unwrap(),
            "brief_summary.md"
        );
        assert_eq!(resolve_view_file("brief").unwrap(), "brief_summary.md");
        assert_eq!(resolve_view_file("deep").unwrap(), "deep_summary.md");
    }

    #[test]
    fn test_resolve_view_file_invalid() {
        assert!(resolve_view_file("invalid").is_err());
        assert!(resolve_view_file("").is_err());
        assert!(resolve_view_file("BODY").is_err());
    }
}
