mod app;

use std::io;

use app::{App, InputMode, ViewMode};
use arxivcat_core::config;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::*,
};

/// Layout rects from the last render, used for mouse hit-testing.
#[derive(Clone, Default)]
struct Rects {
    header: Rect,
    status: Rect,
    paper_list: Rect,
    preview: Rect,
    chat: Rect,
    preview_tabs: Rect,
    chat_input: Rect,
    three_pane: bool,
}

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
    let mut rects = Rects::default();
    loop {
        terminal.draw(|f| rects = ui(f, app))?;

        if app.quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        handle_key(app, key).await?;
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse(app, &mouse, &rects).await?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

async fn handle_mouse(app: &mut App, mouse: &MouseEvent, rects: &Rects) -> io::Result<()> {
    let (col, row) = (mouse.column, mouse.row);

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if in_rect(rects.preview, col, row) {
                app.preview_scroll = app.preview_scroll.saturating_add(3);
            } else if rects.three_pane && in_rect(rects.chat, col, row) {
                app.chat_scroll = app.chat_scroll.saturating_add(3);
            } else if in_rect(rects.paper_list, col, row) {
                app.paper_list_scroll = app.paper_list_scroll.saturating_add(1);
                if !app.papers.is_empty() {
                    let max = app.papers.len().saturating_sub(1);
                    app.paper_list_selected = (app.paper_list_selected + 1).min(max);
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if in_rect(rects.preview, col, row) {
                app.preview_scroll = app.preview_scroll.saturating_sub(3);
            } else if rects.three_pane && in_rect(rects.chat, col, row) {
                app.chat_scroll = app.chat_scroll.saturating_sub(3);
            } else if in_rect(rects.paper_list, col, row) {
                app.paper_list_scroll = app.paper_list_scroll.saturating_sub(1);
                app.paper_list_selected = app.paper_list_selected.saturating_sub(1);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if in_rect(rects.paper_list, col, row) {
                let rel_row = row.saturating_sub(rects.paper_list.y);
                let idx = app.paper_list_scroll + rel_row as usize;
                if idx < app.papers.len() {
                    app.paper_list_selected = idx;
                    app.load_paper(idx);
                }
            }
            if in_rect(rects.preview_tabs, col, row) {
                let rel_col = col.saturating_sub(rects.preview_tabs.x);
                let tab_idx = (rel_col / 12).min(3) as usize;
                let views = [ViewMode::Body, ViewMode::Appendix, ViewMode::Note, ViewMode::Description];
                app.view_mode = views[tab_idx];
            }
            if rects.three_pane && in_rect(rects.chat_input, col, row) {
                app.input_mode = InputMode::Chat;
                app.show_chat = true;
            }
            if rects.three_pane && in_rect(rects.chat, col, row) {
                app.input_mode = InputMode::Chat;
            }
            if in_rect(rects.preview, col, row) {
                if app.input_mode == InputMode::Chat {
                    app.input_mode = InputMode::Normal;
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if in_rect(rects.preview, col, row) {
                let dy = mouse.row as i32 - rects.preview.y as i32 - 2;
                if dy >= 0 {
                    app.preview_scroll = (dy as u16).max(0);
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn in_rect(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
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
                app.status.clear();
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(&mut app.status);
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
                            let _ =
                                open::that(format!("https://arxiv.org/pdf/{}", paper.arxiv_id));
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
                        let _ = config::save_model_preference(&app.chat_model);
                        app.add_log(&format!("model: {new}"));
                    }
                    "t" | "think" => {
                        app.deep_thinking = !app.deep_thinking;
                        app.add_log(&format!("deep thinking: {}", app.deep_thinking));
                    }
                    _ => app.add_log(&format!("unknown: {cmd}. try: open, pdf, scan, dl, model, think")),
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
                app.add_log("[?] scroll:select j↓/k↑/mouse enter:load 1-4:view c:chat e:edit s:scan d:dl o:open p:pdf ::cmd q:quit");
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
                    let _ = open::that(format!("https://arxiv.org/pdf/{}", paper.arxiv_id));
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

fn ui(f: &mut Frame, app: &mut App) -> Rects {
    let mut rects = Rects::default();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    rects.header = main_chunks[0];
    rects.status = main_chunks[2];

    render_header(f, app, rects.header);

    let body_area = main_chunks[1];
    let three_pane = app.show_chat && app.current_paper.is_some();
    rects.three_pane = three_pane;

    if three_pane {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(40),
                Constraint::Percentage(35),
            ])
            .split(body_area);
        rects.paper_list = h[0];
        rects.preview = h[1];
        rects.chat = h[2];

        let preview_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(rects.preview);
        rects.preview_tabs = preview_chunks[0];

        let chat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1), Constraint::Length(3)])
            .split(rects.chat);
        rects.chat_input = chat_chunks[2];
    } else {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
            .split(body_area);
        rects.paper_list = h[0];
        rects.preview = h[1];

        let preview_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(rects.preview);
        rects.preview_tabs = preview_chunks[0];
        rects.chat_input = Rect::default();
    }

    render_paper_list(f, app, rects.paper_list);
    render_preview(f, app, rects.preview, rects.preview_tabs);
    if three_pane {
        render_chat(f, app, rects.chat, rects.chat_input);
    }
    render_status(f, app, rects.status);

    rects
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.workspace_path_str.is_empty() {
        "ArxivCat TUI — no workspace".to_string()
    } else {
        format!(
            "ArxivCat TUI — {}  papers:{}",
            app.workspace_path_str,
            app.papers.len()
        )
    };
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Rgb(137, 180, 250)))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
        );
    f.render_widget(p, area);
}

fn render_paper_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .papers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let icon = if p.is_complete {
                "●"
            } else if p.has_body {
                "○"
            } else {
                "·"
            };
            let style = if i == app.paper_list_selected {
                Style::default()
                    .fg(Color::Rgb(205, 214, 244))
                    .bg(Color::Rgb(69, 71, 90))
            } else {
                Style::default().fg(Color::Rgb(166, 173, 200))
            };
            ListItem::new(format!("{icon} {:<14} {}", p.arxiv_id, p.title)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Papers ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
    );
    f.render_stateful_widget(
        list,
        area,
        &mut ratatui::widgets::ListState::default()
            .with_selected(Some(app.paper_list_selected))
            .with_offset(app.paper_list_scroll),
    );
}

fn render_preview(f: &mut Frame, app: &App, area: Rect, tab_area: Rect) {
    let mode_label = match app.view_mode {
        ViewMode::Body => " Body ",
        ViewMode::Appendix => " Appendix ",
        ViewMode::Note => " Note ",
        ViewMode::Description => " Description ",
    };
    let tab_widget = Paragraph::new(mode_label).style(
        Style::default()
            .fg(Color::Rgb(137, 180, 250))
            .bg(Color::Rgb(49, 50, 68)),
    );
    f.render_widget(tab_widget, tab_area);

    let text = if app.current_paper.is_some() {
        app.current_text()
    } else {
        "← Click a paper or press Enter to load"
    };

    let content = if text.is_empty() { "(empty)" } else { text };

    let char_count = content.chars().count();
    let lines = content.lines().count();
    let p = Paragraph::new(content)
        .block(
            Block::default()
                .title(format!(" {} chars · {} lines ", char_count, lines))
                .title_bottom(" scroll ↑↓/wheel ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(p, area);
}

fn render_chat(f: &mut Frame, app: &App, area: Rect, input_area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);
    let messages_area = inner[0];

    let model_line = format!(
        " Chat [{}] {} ",
        app.chat_model,
        if app.deep_thinking { "🧠" } else { "" }
    );

    let msgs: Vec<ListItem> = app
        .chat_messages
        .iter()
        .map(|m| {
            let style = if m.speaker == "user" {
                Style::default().fg(Color::Rgb(137, 180, 250))
            } else {
                Style::default().fg(Color::Rgb(205, 214, 244))
            };
            let prefix = if m.speaker == "user" { "You" } else { "AI" };
            ListItem::new(format!("<{prefix}> {}", m.content)).style(style)
        })
        .collect();
    let chat_list = List::new(msgs).block(
        Block::default()
            .title(model_line)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
    );
    f.render_widget(chat_list, messages_area);

    let input_text = format!(
        "> {}",
        if app.chat_input.is_empty() {
            "type here..."
        } else {
            &app.chat_input
        }
    );
    let input_style = if app.input_mode == InputMode::Chat {
        Style::default()
            .fg(Color::Rgb(205, 214, 244))
            .bg(Color::Rgb(49, 50, 68))
    } else {
        Style::default().fg(Color::Rgb(108, 112, 134))
    };
    let input = Paragraph::new(input_text)
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if app.input_mode == InputMode::Chat {
                    Color::Rgb(137, 180, 250)
                } else {
                    Color::Rgb(49, 50, 68)
                })),
        );
    f.render_widget(input, input_area);
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
        let chat_hint = if app.current_paper.is_some() {
            " | c:chat"
        } else {
            ""
        };
        format!(
            "[{mode}] ↑↓/scroll/jk:nav 1-4:view{chat_hint} e:edit s:scan d:dl o:open p:pdf ::cmd q:quit  ?:help",
        )
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Rgb(166, 173, 200)))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
        );
    f.render_widget(status, area);
}
