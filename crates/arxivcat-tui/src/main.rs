mod app;

use std::io;

use app::{App, InputMode, ViewMode};
use arxivcat_core::config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::*,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut app = App::new();

    if let Some(ref wp) = config::load_workspace_path() {
        let p = std::path::PathBuf::from(wp);
        if p.exists() {
            match arxivcat_core::workspace::Workspace::open(&p) {
                Ok(ws) => {
                    app.workspace_path_str = ws.path.to_string_lossy().to_string();
                    app.papers = ws.papers.clone();
                    app.workspace = Some(ws);
                }
                Err(e) => app.add_log(&format!("workspace error: {e}")),
            }
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("error: {e}");
    }
    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(app, key).await?;
                }
            }
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, key: event::KeyEvent) -> io::Result<()> {
    match app.input_mode {
        InputMode::Chat => match key.code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.show_chat = false;
            }
            KeyCode::Enter => {
                let msg = app.chat_input.clone();
                app.chat_input.clear();
                if !msg.is_empty() {
                    app.chat_messages.push(app::ChatMsg {
                        speaker: "user".to_string(),
                        content: msg.clone(),
                    });
                    app.add_log("sending...");
                    // TODO: async chat send
                }
            }
            KeyCode::Backspace => {
                app.chat_input.pop();
            }
            KeyCode::Char(c) => {
                app.chat_input.push(c);
            }
            _ => {}
        },
        InputMode::NoteEdit => match key.code {
            KeyCode::Esc => {
                if let Some(ref paper) = app.current_paper {
                    let path = paper.folder.join("note.txt");
                    let _ = std::fs::write(&path, &app.note_content);
                    app.add_log("note saved");
                }
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                app.current_text_mut().pop();
            }
            KeyCode::Enter => {
                app.current_text_mut().push('\n');
            }
            KeyCode::Char(c) => {
                app.current_text_mut().push(c);
            }
            _ => {}
        },
        InputMode::Command => match key.code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.status = String::new();
            }
            KeyCode::Enter => {
                let cmd = app.status.clone();
                app.status.clear();
                app.input_mode = InputMode::Normal;
                match cmd.as_str() {
                    "o" | "open" => {
                        if let Some(ref paper) = app.current_paper {
                            let _ = open::that(&paper.folder);
                            app.add_log(&format!("opened {}", paper.folder.display()));
                        }
                    }
                    "p" | "pdf" => {
                        if let Some(ref paper) = app.current_paper {
                            let _ = open::that(format!(
                                "https://arxiv.org/pdf/{}",
                                paper.arxiv_id
                            ));
                            app.add_log(&format!("opened PDF for {}", paper.arxiv_id));
                        }
                    }
                    "scan" => {
                        app.scan_pdfs().await;
                    }
                    "dl" | "download" => {
                        app.download_all().await;
                    }
                    "m" | "model" => {
                        let new = if app.chat_model == "Flash" { "Pro" } else { "Flash" };
                        app.chat_model = new.to_string();
                        app.add_log(&format!("model: {new}"));
                    }
                    "t" | "think" => {
                        app.deep_thinking = !app.deep_thinking;
                        app.add_log(&format!("deep thinking: {}", app.deep_thinking));
                    }
                    _ => app.add_log(&format!("unknown command: {cmd}. try: open, pdf, scan, dl, model, think")),
                }
            }
            KeyCode::Char(c) => {
                app.status.push(c);
            }
            KeyCode::Backspace => {
                app.status.pop();
            }
            _ => {}
        },
        InputMode::Normal => match key.code {
            KeyCode::Char('q') => app.quit = true,
            KeyCode::Char('?') => {
                app.add_log("[?] j/k:nav 1-4:view c:chat o:open p:pdf s:scan d:download :cmd q:quit");
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !app.papers.is_empty() && app.paper_list_selected + 1 < app.papers.len() {
                    app.paper_list_selected += 1;
                    if app.paper_list_selected >= app.paper_list_scroll + 20 {
                        app.paper_list_scroll += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.paper_list_selected > 0 {
                    app.paper_list_selected -= 1;
                    if app.paper_list_selected < app.paper_list_scroll {
                        app.paper_list_scroll = app.paper_list_selected;
                    }
                }
            }
            KeyCode::Enter => {
                if !app.papers.is_empty() {
                    let idx = app.paper_list_selected;
                    app.load_paper(idx);
                }
            }
            KeyCode::Char('1') => app.view_mode = ViewMode::Body,
            KeyCode::Char('2') => app.view_mode = ViewMode::Appendix,
            KeyCode::Char('3') => app.view_mode = ViewMode::Note,
            KeyCode::Char('4') => app.view_mode = ViewMode::Description,
            KeyCode::Char('e') => {
                if app.view_mode == ViewMode::Note && app.current_paper.is_some() {
                    app.input_mode = InputMode::NoteEdit;
                    app.add_log("editing note (Esc to save)");
                }
            }
            KeyCode::Char('c') => {
                if app.current_paper.is_some() {
                    app.show_chat = !app.show_chat;
                    if app.show_chat {
                        app.input_mode = InputMode::Chat;
                    } else {
                        app.input_mode = InputMode::Normal;
                    }
                }
            }
            KeyCode::Char('s') => {
                app.scan_pdfs().await;
            }
            KeyCode::Char('d') => {
                app.download_all().await;
            }
            KeyCode::Char('o') => {
                if let Some(ref paper) = app.current_paper {
                    let _ = open::that(&paper.folder);
                    app.add_log(&format!("opened {}", paper.folder.display()));
                }
            }
            KeyCode::Char('p') => {
                if let Some(ref paper) = app.current_paper {
                    let _ =
                        open::that(format!("https://arxiv.org/pdf/{}", paper.arxiv_id));
                    app.add_log(&format!("opened PDF for {}", paper.arxiv_id));
                }
            }
            KeyCode::Char(':') => {
                app.input_mode = InputMode::Command;
                app.status = String::from(":");
            }
            _ => {}
        },
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    render_header(f, app, main_chunks[0]);

    let body = if app.show_chat && app.current_paper.is_some() {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(40), Constraint::Percentage(30)])
            .split(main_chunks[1]);
        render_paper_list(f, app, h[0]);
        render_preview(f, app, h[1]);
        render_chat(f, app, h[2]);
    } else {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
            .split(main_chunks[1]);
        render_paper_list(f, app, h[0]);
        render_preview(f, app, h[1]);
    };
    let _ = body;

    render_status(f, app, main_chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.workspace_path_str.is_empty() {
        "ArxivCat TUI — no workspace".to_string()
    } else {
        format!(
            "ArxivCat TUI — {} ({} papers)",
            app.workspace_path_str,
            app.papers.len()
        )
    };
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Rgb(137, 180, 250)))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(49, 50, 68))));
    f.render_widget(p, area);
}

fn render_paper_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .papers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let icon = if p.is_complete { "●" } else if p.has_body { "○" } else { "·" };
            let style = if i == app.paper_list_selected {
                Style::default().fg(Color::Rgb(205, 214, 244)).bg(Color::Rgb(69, 71, 90))
            } else {
                Style::default().fg(Color::Rgb(166, 173, 200))
            };
            ListItem::new(format!("{icon} {:<14} {}", p.arxiv_id, p.title)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Papers")
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
        )
        .scroll_padding(1);

    f.render_widget(list, area);
}

fn render_preview(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let mode_label = match app.view_mode {
        ViewMode::Body => " Body ",
        ViewMode::Appendix => " Appendix ",
        ViewMode::Note => " Note ",
        ViewMode::Description => " Description ",
    };
    let tab_widget = Paragraph::new(mode_label)
        .style(Style::default().fg(Color::Rgb(137, 180, 250)).bg(Color::Rgb(49, 50, 68)));
    f.render_widget(tab_widget, chunks[0]);

    let text = if app.current_paper.is_some() {
        app.current_text()
    } else {
        "Select a paper from the list"
    };

    let content = if text.is_empty() {
        "(empty)".to_string()
    } else {
        text.to_string()
    };

    let p = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::NONE),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(p, chunks[1]);
}

fn render_chat(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let model_str = format!("Chat [{}]", app.chat_model);
    let header = Paragraph::new(model_str)
        .style(Style::default().fg(Color::Rgb(137, 180, 250)))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(49, 50, 68))));
    f.render_widget(header, chunks[0]);

    let msgs: Vec<ListItem> = app
        .chat_messages
        .iter()
        .map(|m| {
            let style = if m.speaker == "user" {
                Style::default().fg(Color::Rgb(137, 180, 250))
            } else {
                Style::default().fg(Color::Rgb(205, 214, 244))
            };
            ListItem::new(format!("<{}> {}", m.speaker, m.content)).style(style)
        })
        .collect();
    let chat_list = List::new(msgs)
        .block(Block::default().borders(Borders::NONE))
        .scroll_padding(1);
    f.render_widget(chat_list, chunks[1]);

    let input = Paragraph::new(format!("> {}", app.chat_input))
        .style(Style::default().fg(Color::Rgb(205, 214, 244)))
        .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Rgb(49, 50, 68))));
    f.render_widget(input, chunks[2]);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let mode = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Chat => "CHAT",
        InputMode::NoteEdit => "EDIT",
        InputMode::Command => "CMD",
    };
    let status_text = if !app.status.is_empty() {
        app.status.clone()
    } else {
        format!(
            "[{}] j↓ k↑ Enter:load 1-4:view c:chat e:edit :cmd q:quit | {} papers",
            mode,
            app.papers.len()
        )
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Rgb(166, 173, 200)))
        .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Rgb(49, 50, 68))));
    f.render_widget(status, area);
}
