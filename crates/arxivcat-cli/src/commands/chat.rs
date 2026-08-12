use crate::Cli;
use arxivcat_core::chat::{self, ContextSelection};
use arxivcat_core::config;

use owo_colors::OwoColorize;

fn gray(s: &str) -> String {
    s.dimmed().to_string()
}

/// Parameters distinguishing the side (per-paper) and global REPL loops.
/// The loop body itself is shared — previously ~400 duplicated lines that
/// had already started to drift (e.g. the side loop's "sent => locked"
/// /context rule was missing from the global loop).
struct ReplConfig<'a> {
    /// System-prompt prefix; the rendered context is appended.
    system_prompt_prefix: String,
    /// Builds the context block from the current selection.
    build_context: Box<dyn Fn(&ContextSelection) -> String + 'a>,
    /// Where sessions are stored.
    chat_dir: std::path::PathBuf,
    /// ChatSession kind ("paper" | "global") and ref (arxiv id or "").
    session_kind: &'a str,
    session_ref: String,
    initial_selection: ContextSelection,
    /// Side chat locks fields that were already sent to the model; the
    /// global chat does not.
    lock_after_send: bool,
    welcome_lines: Vec<String>,
}

async fn run_repl(cli: &Cli, cfg: ReplConfig<'_>) {
    let mut selection = cfg.initial_selection;
    let mut last_sent = ContextSelection {
        body: false,
        appendix: false,
        description: false,
        note: false,
    };
    let mut model = config::load_model_preference();
    let mut deep_thinking = true;
    let mut history: Vec<chat::session::ChatMessage> = Vec::new();

    for line in &cfg.welcome_lines {
        println!("{}", gray(line));
    }
    println!(
        "{}",
        gray("Commands: /quit /model <Flash|Pro> /thinking /context [field] /save /load /history /clear /help")
    );
    println!();

    loop {
        use std::io::{self, Write};
        print!("You: ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let line = line.trim().to_string();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('/') {
            let parts: Vec<&str> = line
                .strip_prefix('/')
                .unwrap_or("")
                .split_whitespace()
                .collect();
            match parts.first().copied().unwrap_or("") {
                "quit" | "exit" | "q" => break,
                "model" => {
                    if let Some(m) = parts.get(1) {
                        let m = match *m {
                            "flash" | "Flash" => "Flash",
                            "pro" | "Pro" => "Pro",
                            _ => {
                                println!("unknown model: {m}. options: Flash, Pro");
                                continue;
                            }
                        };
                        model = m.to_string();
                        let _ = config::save_model_preference(m);
                        println!("{} model: {model}", gray("switched to"));
                    }
                    continue;
                }
                "thinking" => {
                    deep_thinking = !deep_thinking;
                    println!(
                        "{} reasoning effort: {}",
                        gray("toggled"),
                        if deep_thinking { "high" } else { "off" }
                    );
                    continue;
                }
                "context" => {
                    if let Some(field) = parts.get(1) {
                        if cfg.lock_after_send {
                            let locked = match *field {
                                "body" => last_sent.body && selection.body,
                                "appendix" => last_sent.appendix && selection.appendix,
                                "description" => last_sent.description && selection.description,
                                "note" => last_sent.note && selection.note,
                                _ => {
                                    println!(
                                        "unknown field: {field}. options: body, appendix, description, note"
                                    );
                                    continue;
                                }
                            };
                            if locked {
                                println!("({field} is locked: already sent to model)");
                                continue;
                            }
                        }
                        match *field {
                            "body" => selection.body = !selection.body,
                            "appendix" => selection.appendix = !selection.appendix,
                            "description" => selection.description = !selection.description,
                            "note" => selection.note = !selection.note,
                            _ => {
                                println!(
                                    "unknown field: {field}. options: body, appendix, description, note"
                                );
                                continue;
                            }
                        }
                    }
                    println!(
                        "context: body={} appendix={} description={} note={}",
                        selection.body, selection.appendix, selection.description, selection.note
                    );
                    continue;
                }
                "save" => {
                    let mut session =
                        chat::session::ChatSession::new(cfg.session_kind, &cfg.session_ref);
                    session.messages = history.clone();
                    session.model = model.clone();
                    session.reasoning_effort = if deep_thinking {
                        "high".to_string()
                    } else {
                        "off".to_string()
                    };
                    session.context_selection = selection.clone();
                    if let Err(e) = chat::session::save_session(&mut session, Some(&cfg.chat_dir)) {
                        eprintln!("error saving session: {e}");
                    } else {
                        println!("{}", gray("session saved"));
                    }
                    continue;
                }
                "load" => {
                    match chat::session::list_sessions(&cfg.chat_dir) {
                        Ok(sessions) => {
                            for (i, s) in sessions.iter().enumerate() {
                                println!("  [{}] {} ({})", i, s.title, s.updated_at);
                            }
                            print!("load session #: ");
                            io::stdout().flush().ok();
                            let mut num = String::new();
                            if io::stdin().read_line(&mut num).is_err() {
                                break;
                            }
                            if let Ok(idx) = num.trim().parse::<usize>() {
                                if idx < sessions.len() {
                                    let s = &sessions[idx];
                                    history = s.messages.clone();
                                    model = s.model.clone();
                                    deep_thinking = s.reasoning_effort == "high";
                                    selection = s.context_selection.clone();
                                    println!("{}", gray("session loaded"));
                                }
                            }
                        }
                        Err(e) => eprintln!("error listing sessions: {e}"),
                    }
                    continue;
                }
                "history" => {
                    for (i, msg) in history.iter().enumerate() {
                        let preview: String = msg.content.chars().take(200).collect();
                        println!("[{}] {}: {preview}", i, msg.speaker);
                    }
                    continue;
                }
                "clear" => {
                    history.clear();
                    last_sent = ContextSelection {
                        body: false,
                        appendix: false,
                        description: false,
                        note: false,
                    };
                    println!("{}", gray("history cleared"));
                    continue;
                }
                "help" => {
                    println!("/quit     — exit chat");
                    println!("/model    — Flash or Pro");
                    println!("/thinking — toggle deep thinking");
                    println!("/context  — toggle body/appendix/description/note");
                    println!("/save     — save session");
                    println!("/load     — load session");
                    println!("/history  — show history");
                    println!("/clear    — clear history");
                    continue;
                }
                _ => {}
            }
        }

        let ctx = (cfg.build_context)(&selection);

        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("{}{}", cfg.system_prompt_prefix, ctx),
        }));

        for msg in history.iter().rev().take(12).rev() {
            messages.push(serde_json::json!({
                "role": if msg.speaker == "user" { "user" } else { "assistant" },
                "content": msg.content,
            }));
        }

        messages.push(serde_json::json!({
            "role": "user",
            "content": line.clone(),
        }));

        history.push(chat::session::ChatMessage {
            speaker: "user".to_string(),
            content: line.clone(),
        });

        last_sent = selection.clone();

        let cancel_flag = std::sync::atomic::AtomicBool::new(false);

        print!("Assistant: ");
        io::stdout().flush().ok();

        let http = match arxivcat_core::net::HttpConfig::new() {
            Ok(c) => c,
            Err(e) => crate::commands::die_err(cli, &e),
        };
        let result = chat::deepseek::stream_chat(
            &http,
            &messages,
            &model,
            if deep_thinking { "high" } else { "off" },
            chat::deepseek::StreamCallbacks {
                on_token: |token, _first| {
                    print!("{token}");
                    io::stdout().flush().ok();
                },
                on_status: |status| {
                    println!("\n[{}]", gray(status));
                },
                on_complete: |_content| {},
            },
            &cancel_flag,
        )
        .await;

        if let Err(e) = result {
            eprintln!("\nerror: {e}");
        }

        println!();
    }
}

fn reject_json(cli: &Cli) {
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for chat commands",
        );
    }
}

pub async fn cmd_side(cli: &Cli, id_or_query: &str) {
    reject_json(cli);
    let ws_path = match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                "no workspace configured",
            );
        }
    };

    let ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    if config::load_cached_token().is_none() {
        crate::commands::die(
            cli,
            crate::commands::EXIT_CONFIG,
            "config",
            "no API token configured. use 'arxivcat token set'",
        );
    }

    let folder = paper.folder.clone();
    let chat_dir = folder.join("arxiv_chats");
    let arxiv_id = paper.arxiv_id.clone();
    let cfg = ReplConfig {
        system_prompt_prefix: "You are a compact in-app chat assistant inside an arXiv paper extraction tool. Maintain conversation continuity. If the user asks a general question, answer it normally. If useful, use the paper preview as extra context. IMPORTANT: When using any content from the paper context, you MUST explicitly include the paper's complete arXiv ID in your response.\n\nPaper content:\n".to_string(),
        build_context: Box::new(move |sel| chat::build_side_chat_context(&folder, sel)),
        chat_dir,
        session_kind: "paper",
        session_ref: arxiv_id.clone(),
        initial_selection: ContextSelection::default(),
        lock_after_send: true,
        welcome_lines: vec![format!("Side chat — paper: {arxiv_id}")],
    };
    run_repl(cli, cfg).await;
}

pub async fn cmd_global(cli: &Cli) {
    reject_json(cli);
    let ws_path = match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                "no workspace configured",
            );
        }
    };

    let ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    if config::load_cached_token().is_none() {
        crate::commands::die(
            cli,
            crate::commands::EXIT_CONFIG,
            "config",
            "no API token configured. use 'arxivcat token set'",
        );
    }

    let chat_dir = ws_path.join("arxivcat_global_chats");
    let papers = ws.papers.clone();
    let cfg = ReplConfig {
        system_prompt_prefix: "You are a compact global chat assistant inside an arXiv paper workspace. The user may ask questions about any of the numbered papers below. When referencing a paper, clearly state its arXiv ID and explain your answer based on the provided description.\n\nWorkspace papers:\n".to_string(),
        build_context: Box::new(move |sel| chat::build_global_chat_context(&papers, sel)),
        chat_dir,
        session_kind: "global",
        session_ref: String::new(),
        initial_selection: ContextSelection {
            body: false,
            appendix: false,
            description: true,
            note: false,
        },
        lock_after_send: false,
        welcome_lines: vec![
            "Global Chat — over workspace papers".to_string(),
            "context: body=false appendix=false description=true note=false".to_string(),
        ],
    };
    run_repl(cli, cfg).await;
}
