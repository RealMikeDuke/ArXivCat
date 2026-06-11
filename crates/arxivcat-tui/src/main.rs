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

        if app.just_selected {
            app.just_selected = false;
        }

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
    app.mouse_col = col;
    app.mouse_row = row;

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if in_rect(rects.preview, col, row) {
                app.preview_scroll = app.preview_scroll.saturating_add(3);
            } else if rects.three_pane && in_rect(rects.chat, col, row) {
                app.chat_scroll = app.chat_scroll.saturating_add(3);
            } else if in_rect(rects.paper_list, col, row) {
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
                app.paper_list_selected = app.paper_list_selected.saturating_sub(1);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.just_selected = false;
            if in_rect(rects.paper_list, col, row) {
                let rel_row = row.saturating_sub(rects.paper_list.y + 1);
                let idx = rel_row as usize;
                if idx < app.papers.len() {
                    app.paper_list_selected = idx;
                    app.load_paper(idx);
                    app.show_chat = true;
                }
            } else if in_rect(rects.preview_tabs, col, row) {
                let rel_col = col.saturating_sub(rects.preview_tabs.x);
                let tab_idx = (rel_col / 12).min(3) as usize;
                let views = [
                    ViewMode::Body,
                    ViewMode::Appendix,
                    ViewMode::Note,
                    ViewMode::Description,
                ];
                app.view_mode = views[tab_idx];
            } else if in_rect(rects.chat_input, col, row) && rects.three_pane {
                app.show_chat = true;
                app.input_mode = InputMode::Chat;
                let prefix = 2u16;
                app.chat_cursor = col
                    .saturating_sub(rects.chat_input.x + prefix)
                    .min(app.chat_input.len() as u16) as usize;
            } else if in_rect(rects.chat, col, row) && rects.three_pane {
                app.show_chat = true;
                app.input_mode = InputMode::Chat;
            } else if in_rect(rects.preview, col, row) && app.input_mode == InputMode::Chat {
                app.input_mode = InputMode::Normal;
            } else if in_rect(rects.preview, col, row) {
                app.selecting = true;
                let rel_y = row.saturating_sub(rects.preview.y + 2);
                let rel_x = col.saturating_sub(rects.preview.x + 1);
                app.sel_start = Some((rel_x, rel_y));
                app.sel_end = None;
                app.preview_scroll = 0;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.selecting && in_rect(rects.preview, col, row) {
                let rel_y = row.saturating_sub(rects.preview.y + 2);
                let rel_x = col.saturating_sub(rects.preview.x + 1);
                app.sel_end = Some((rel_x, rel_y));
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.selecting {
                app.selecting = false;
                if let (Some(start), Some(end)) = (app.sel_start, app.sel_end) {
                    let text = app.current_text();
                    let (sx, sy) = start;
                    let (ex, ey) = end;
                    let (x1, y1, x2, y2) = if (sy, sx) <= (ey, ex) {
                        (sx, sy, ex, ey)
                    } else {
                        (ex, ey, sx, sy)
                    };
                    let content = get_selected_text(text, x1 as usize, y1 as usize, x2 as usize, y2 as usize);
                    if !content.is_empty() {
                        copy_to_clipboard(&content);
                        app.just_selected = true;
                    }
                }
                app.sel_start = None;
                app.sel_end = None;
            }
        }
        _ => {}
    }

    Ok(())
}

fn get_selected_text(
    text: &str,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if y1 >= lines.len() {
        return String::new();
    }
    if y1 == y2 {
        let line = lines[y1];
        let start = x1.min(line.len());
        let end = x2.min(line.len()).max(start);
        return line[start..end].to_string();
    }
    let mut result = String::new();
    for yi in y1..=y2.min(lines.len().saturating_sub(1)) {
        if !result.is_empty() {
            result.push('\n');
        }
        let line = lines[yi];
        if yi == y1 {
            let s = x1.min(line.len());
            result.push_str(&line[s..]);
        } else if yi == y2 {
            let e = x2.min(line.len());
            result.push_str(&line[..e]);
        } else {
            result.push_str(line);
        }
    }
    result
}

fn copy_to_clipboard(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
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
                let msg = std::mem::take(&mut app.chat_input);
                if !msg.is_empty() {
                    app.chat_messages.push(app::ChatMsg {
                        speaker: "user".to_string(),
                        content: msg.clone(),
                    });
                    app.add_log("sending...");
                }
                app.chat_cursor = 0;
            }
            KeyCode::Backspace => {
                if app.chat_cursor > 0 {
                    app.chat_cursor -= 1;
                    app.chat_input.remove(app.chat_cursor);
                }
            }
            KeyCode::Delete => {
                if app.chat_cursor < app.chat_input.len() {
                    app.chat_input.remove(app.chat_cursor);
                }
            }
            KeyCode::Left => {
                app.chat_cursor = app.chat_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                app.chat_cursor = (app.chat_cursor + 1).min(app.chat_input.len());
            }
            KeyCode::Home => app.chat_cursor = 0,
            KeyCode::End => app.chat_cursor = app.chat_input.len(),
            KeyCode::Char(c) => {
                app.chat_input.insert(app.chat_cursor, c);
                app.chat_cursor += 1;
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
                app.cmd_cursor = 1;
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(&mut app.status);
                let cmd_body = cmd.strip_prefix(':').unwrap_or("").to_string();
                app.cmd_cursor = 1;
                app.input_mode = InputMode::Normal;
                match cmd_body.as_str() {
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
                    _ => app.add_log(&format!("unknown: {cmd_body}. try: open, pdf, scan, dl, model, think")),
                }
            }
            KeyCode::Backspace => {
                if app.cmd_cursor > 1 {
                    app.cmd_cursor -= 1;
                    app.status.remove(app.cmd_cursor);
                }
            }
            KeyCode::Delete => {
                if app.cmd_cursor < app.status.len() {
                    app.status.remove(app.cmd_cursor);
                }
            }
            KeyCode::Left => {
                app.cmd_cursor = (app.cmd_cursor.saturating_sub(1)).max(1);
            }
            KeyCode::Right => {
                app.cmd_cursor = (app.cmd_cursor + 1).min(app.status.len());
            }
            KeyCode::Home => app.cmd_cursor = 1,
            KeyCode::End => app.cmd_cursor = app.status.len(),
            KeyCode::Char(c) => {
                app.status.insert(app.cmd_cursor, c);
                app.cmd_cursor += 1;
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
                    app.show_chat = true;
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
    let hovered = in_rect(area, app.mouse_col, app.mouse_row);
    let border_style = if hovered {
        Style::default().fg(Color::Rgb(108, 112, 134))
    } else {
        Style::default().fg(Color::Rgb(49, 50, 68))
    };

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
            let is_selected = i == app.paper_list_selected;
            let is_hovered = app.mouse_row as usize == area.y as usize + 1 + i
                && app.mouse_col >= area.x
                && app.mouse_col < area.x + area.width;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Rgb(205, 214, 244))
                    .bg(Color::Rgb(69, 71, 90))
            } else if is_hovered {
                Style::default()
                    .fg(Color::Rgb(137, 180, 250))
                    .bg(Color::Rgb(49, 50, 68))
                    .add_modifier(ratatui::style::Modifier::BOLD)
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
            .border_style(border_style),
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
    use ratatui::text::{Line, Span};

    let text = if app.current_paper.is_some() {
        app.current_text()
    } else {
        "← Click a paper or press Enter to load"
    };

    let content = if text.is_empty() { "(empty)" } else { text };

    let styled_text: ratatui::text::Text = if app.selecting
        && app.sel_start.is_some()
        && app.current_paper.is_some()
        && !text.is_empty()
    {
        let (sx, sy) = app.sel_start.unwrap();
        let (ex, ey) = app.sel_end.unwrap_or((sx, sy));
        let (x1, y1, x2, y2) = if (sy, sx) <= (ey, ex) {
            (sx as usize, sy as usize, ex as usize, ey as usize)
        } else {
            (ex as usize, ey as usize, sx as usize, sy as usize)
        };

        let raw_lines: Vec<&str> = content.lines().collect();
        let normal_style = Style::default().fg(Color::Rgb(205, 214, 244));
        let sel_style = Style::default()
            .fg(Color::Rgb(205, 214, 244))
            .bg(Color::Rgb(69, 71, 90));

        let styled_lines: Vec<Line> = raw_lines
            .iter()
            .enumerate()
            .map(|(li, line)| {
                if li < y1 || li > y2 {
                    return Line::from(Span::styled(line.to_string(), normal_style));
                }
                let sc = if li == y1 { x1 } else { 0 };
                let ec = if li == y2 { x2 } else { line.len() };
                let s = sc.min(line.len());
                let e = ec.min(line.len());
                let mut spans: Vec<Span> = Vec::new();
                if s > 0 {
                    spans.push(Span::styled(line[..s].to_string(), normal_style));
                }
                if e > s {
                    spans.push(Span::styled(line[s..e].to_string(), sel_style));
                }
                if e < line.len() {
                    spans.push(Span::styled(line[e..].to_string(), normal_style));
                }
                Line::from(spans)
            })
            .collect();
        ratatui::text::Text::from(styled_lines)
    } else {
        ratatui::text::Text::from(content)
    };

    let char_count = content.chars().count();
    let total_lines = content.lines().count();
    let hovered_preview = in_rect(area, app.mouse_col, app.mouse_row);
    let border_style = if hovered_preview {
        Style::default().fg(Color::Rgb(108, 112, 134))
    } else {
        Style::default().fg(Color::Rgb(49, 50, 68))
    };
    let p = Paragraph::new(styled_text)
        .block(
            Block::default()
                .title(format!(" {} chars · {} lines ", char_count, total_lines))
                .title_bottom(" scroll ↑↓/wheel ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(p, area);

    let tab_labels = [" Body ", " Appendix ", " Note ", " Description "];
    let tab_widths = [7u16, 11u16, 7u16, 14u16];

    let mut tab_x = tab_area.x;
    for i in 0..4 {
        let w = tab_widths[i];
        let tab_rect = Rect::new(tab_x, tab_area.y, w, 1);
        let is_active = i == app.view_mode as usize;
        let is_hovered = app.mouse_col >= tab_rect.x
            && app.mouse_col < tab_rect.x + tab_rect.width
            && app.mouse_row == tab_rect.y;

        let style = if is_active {
            Style::default()
                .fg(Color::Rgb(30, 30, 46))
                .bg(Color::Rgb(137, 180, 250))
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else if is_hovered {
            Style::default()
                .fg(Color::Rgb(205, 214, 244))
                .bg(Color::Rgb(69, 71, 90))
        } else {
            Style::default()
                .fg(Color::Rgb(108, 112, 134))
                .bg(Color::Rgb(30, 30, 46))
        };

        let label = if is_hovered && !is_active {
            format!(" {} ", tab_labels[i].trim())
        } else {
            tab_labels[i].to_string()
        };

        let tab = Paragraph::new(label).style(style);
        f.render_widget(tab, tab_rect);
        tab_x += w;
    }
}

fn render_chat(f: &mut Frame, app: &App, area: Rect, input_area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);
    let messages_area = inner[0];

    let hovered_chat = in_rect(area, app.mouse_col, app.mouse_row);
    let chat_border = if hovered_chat {
        Style::default().fg(Color::Rgb(108, 112, 134))
    } else {
        Style::default().fg(Color::Rgb(49, 50, 68))
    };

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
            .border_style(chat_border),
    );
    f.render_widget(chat_list, messages_area);

    let input_display = build_cursor_text(
        &app.chat_input,
        app.chat_cursor,
        "> ",
        app.input_mode == InputMode::Chat,
    );
    let input_hovered = app.mouse_col >= input_area.x + 1
        && app.mouse_col < input_area.x + input_area.width
        && app.mouse_row >= input_area.y
        && app.mouse_row < input_area.y + input_area.height;
    let input_style = if app.input_mode == InputMode::Chat {
        Style::default()
            .fg(Color::Rgb(205, 214, 244))
            .bg(Color::Rgb(49, 50, 68))
    } else if input_hovered {
        Style::default()
            .fg(Color::Rgb(166, 173, 200))
            .bg(Color::Rgb(40, 40, 56))
    } else {
        Style::default().fg(Color::Rgb(108, 112, 134))
    };

    let input_border_color = if app.input_mode == InputMode::Chat {
        Color::Rgb(137, 180, 250)
    } else if input_hovered {
        Color::Rgb(108, 112, 134)
    } else {
        Color::Rgb(49, 50, 68)
    };
    let input = Paragraph::new(input_display)
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(input_border_color)),
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
    let status_text: ratatui::text::Text = if app.input_mode == InputMode::Command {
        build_cursor_text(&app.status, app.cmd_cursor, "", true)
    } else if app.just_selected {
        ratatui::text::Text::from("[copied to clipboard]")
    } else if !app.status.is_empty() {
        ratatui::text::Text::from(app.status.as_str())
    } else {
        let chat_hint = if app.current_paper.is_some() {
            " | c:chat"
        } else {
            ""
        };
        ratatui::text::Text::from(format!(
            "[{mode}] ↑↓/scroll/jk:nav 1-4:view{chat_hint} e:edit s:scan d:dl o:open p:pdf ::cmd q:quit  ?:help",
        ))
    };
    let hovered_status = in_rect(area, app.mouse_col, app.mouse_row);
    let status = Paragraph::new(status_text)
        .style(if hovered_status {
            Style::default().fg(Color::Rgb(205, 214, 244))
        } else {
            Style::default().fg(Color::Rgb(166, 173, 200))
        })
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(49, 50, 68))),
        );
    f.render_widget(status, area);
}

fn build_cursor_text(
    text: &str,
    cursor: usize,
    prefix: &str,
    active: bool,
) -> ratatui::text::Text<'static> {
    use ratatui::text::Span;

    if !active {
        return ratatui::text::Text::from(format!("{prefix}{text}"));
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(prefix.to_string()));

    let before = &text[..cursor.min(text.len())];
    let cursor_char = if cursor < text.len() {
        text[cursor..].chars().next().unwrap_or(' ')
    } else {
        ' '
    };
    let after = if cursor < text.len() {
        let rest = &text[cursor..];
        let first_char_len = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
        &rest[first_char_len..]
    } else {
        ""
    };

    spans.push(Span::raw(before.to_string()));

    let cursor_span = if cursor < text.len() {
        Span::styled(
            cursor_char.to_string(),
            Style::default()
                .fg(Color::Rgb(30, 30, 46))
                .bg(Color::Rgb(137, 180, 250)),
        )
    } else {
        Span::styled(
            " ".to_string(),
            Style::default()
                .fg(Color::Rgb(30, 30, 46))
                .bg(Color::Rgb(137, 180, 250)),
        )
    };
    spans.push(cursor_span);

    if !after.is_empty() {
        spans.push(Span::raw(after.to_string()));
    }

    ratatui::text::Text::from(ratatui::text::Line::from(spans))
}
