use arxivcat_core::chat::{self, ContextSelection};
use arxivcat_core::config;
use crate::Cli;

use owo_colors::OwoColorize;

fn gray(s: &str) -> String { s.dimmed().to_string() }

pub async fn cmd_side(cli: &Cli, id_or_query: &str) {
    if cli.json {
        crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", "--json is not supported for chat commands");
    }
    let ws_path = match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no workspace configured");
        }
    };

    let ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    let paper = crate::commands::find_paper_or_die(cli, &ws, id_or_query);

    let token = config::load_cached_token();
    if token.is_none() {
        crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no API token configured. use 'arxivcat token set'");
    }

    println!("{} {}", gray("Side chat — paper:"), paper.arxiv_id);
    println!("{}", gray("Commands: /quit /model <Flash|Pro> /thinking /context [field] /save /load /history /clear /help"));
    println!();

    let chat_dir = paper.folder.join("arxiv_chats");
    let mut selection = ContextSelection::default();
    let mut last_sent = ContextSelection {
        body: false,
        appendix: false,
        description: false,
        note: false,
    };
    let mut model = config::load_model_preference();
    let mut deep_thinking = true;
    let mut history: Vec<chat::session::ChatMessage> = Vec::new();

    loop {
        print!("You: ");
        use std::io::{self, Write};
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
                    println!("{} reasoning effort: {}", gray("toggled"), if deep_thinking { "high" } else { "off" });
                    continue;
                }
                "context" => {
                    if let Some(field) = parts.get(1) {
                        let locked = match *field {
                            "body" => last_sent.body && selection.body,
                            "appendix" => last_sent.appendix && selection.appendix,
                            "description" => last_sent.description && selection.description,
                            "note" => last_sent.note && selection.note,
                            _ => {
                                println!("unknown field: {field}. options: body, appendix, description, note");
                                continue;
                            }
                        };
                        if locked {
                            println!("({field} is locked: already sent to model)");
                            continue;
                        }
                        match *field {
                            "body" => selection.body = !selection.body,
                            "appendix" => selection.appendix = !selection.appendix,
                            "description" => selection.description = !selection.description,
                            "note" => selection.note = !selection.note,
                            _ => unreachable!(),
                        }
                    }
                    println!(
                        "context: body={} appendix={} description={} note={}",
                        selection.body, selection.appendix, selection.description, selection.note
                    );
                    continue;
                }
                "save" => {
                    let mut session = chat::session::ChatSession::new("paper", &paper.arxiv_id);
                    session.messages = history.clone();
                    session.model = model.clone();
                    session.reasoning_effort = if deep_thinking { "high".to_string() } else { "off".to_string() };
                    session.context_selection = selection.clone();
                    if let Err(e) = chat::session::save_session(&mut session, Some(&chat_dir)) {
                        eprintln!("error saving session: {e}");
                    } else {
                        println!("{}", gray("session saved"));
                    }
                    continue;
                }
                "load" => {
                    match chat::session::list_sessions(&chat_dir) {
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

        let ctx = chat::build_side_chat_context(&paper.folder, &selection);

        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("You are a compact in-app chat assistant inside an arXiv paper extraction tool. Maintain conversation continuity. If the user asks a general question, answer it normally. If useful, use the paper preview as extra context. IMPORTANT: When using any content from the paper context, you MUST explicitly include the paper's complete arXiv ID in your response.\n\nPaper content:\n{ctx}")
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

        match result {
            Ok(()) => {}
            Err(e) => {
                eprintln!("\nerror: {e}");
            }
        }

        println!();
    }
}

pub async fn cmd_global(cli: &Cli) {
    if cli.json {
        crate::commands::die(cli, crate::commands::EXIT_USAGE, "usage", "--json is not supported for chat commands");
    }
    let ws_path = match crate::commands::resolve_workspace(cli) {
        Some(p) => p,
        None => {
            crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no workspace configured");
        }
    };

    let ws = match arxivcat_core::workspace::Workspace::open(&ws_path) {
        Ok(w) => w,
        Err(e) => {
            crate::commands::die_err(cli, &e);
        }
    };

    let token = config::load_cached_token();
    if token.is_none() {
        crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", "no API token configured. use 'arxivcat token set'");
    }

    let chat_dir = ws_path.join("arxivcat_global_chats");
    let mut model = config::load_model_preference();
    let mut deep_thinking = true;
    let mut history: Vec<chat::session::ChatMessage> = Vec::new();
    let mut selection = ContextSelection {
        body: false,
        appendix: false,
        description: true,
        note: false,
    };

    println!("{}", gray("Global Chat — over workspace papers"));
    println!(
        "context: body={} appendix={} description={} note={}",
        selection.body, selection.appendix, selection.description, selection.note
    );
    println!("{}", gray("Commands: /quit /model <Flash|Pro> /thinking /context [field] /save /load /history /clear /help"));
    println!();

    loop {
        print!("You: ");
        use std::io::{self, Write};
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
                    println!("{} reasoning effort: {}", gray("toggled"), if deep_thinking { "high" } else { "off" });
                    continue;
                }
                "context" => {
                    if let Some(field) = parts.get(1) {
                        match *field {
                            "body" => selection.body = !selection.body,
                            "appendix" => selection.appendix = !selection.appendix,
                            "description" => selection.description = !selection.description,
                            "note" => selection.note = !selection.note,
                            _ => {
                                println!("unknown field: {field}. options: body, appendix, description, note");
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
                    let mut session = chat::session::ChatSession::new("global", "");
                    session.messages = history.clone();
                    session.model = model.clone();
                    session.reasoning_effort = if deep_thinking { "high".to_string() } else { "off".to_string() };
                    session.context_selection = selection.clone();
                    if let Err(e) = chat::session::save_session(&mut session, Some(&chat_dir)) {
                        eprintln!("error saving session: {e}");
                    } else {
                        println!("{}", gray("session saved"));
                    }
                    continue;
                }
                "load" => {
                    match chat::session::list_sessions(&chat_dir) {
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

        let ctx = chat::build_global_chat_context(&ws.papers, &selection);

        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("You are a compact global chat assistant inside an arXiv paper workspace. The user may ask questions about any of the numbered papers below. When referencing a paper, clearly state its arXiv ID and explain your answer based on the provided description.\n\nWorkspace papers:\n{ctx}")
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
