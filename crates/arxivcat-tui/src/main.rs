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
use ratatui::{prelude::*, widgets::*, text::{Line as TLine, Span}};

#[derive(Clone, Default)]
struct Rects {
    header: Rect,
    status: Rect,
    paper_list: Rect,
    preview_text: Rect,
    preview_tabs: Rect,
    chat: Rect,
    chat_input: Rect,
    three_pane: bool,
    body: Rect,
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(e) = res { eprintln!("error: {e}"); }
    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let mut rects = Rects::default();
    loop {
        terminal.draw(|f| rects = ui(f, app))?;

        if app.just_selected { app.just_selected = false; }
        if app.quit { break; }

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => if key.kind == KeyEventKind::Press { handle_key(app, key).await?; }
                Event::Mouse(mouse) => handle_mouse(app, &mouse, &rects).await?,
                _ => {}
            }
        }
    }
    Ok(())
}

fn panel_seam(panel: Rect) -> (u16, u16) {
    (panel.x + panel.width - 1, panel.x + panel.width)
}

async fn handle_mouse(app: &mut App, mouse: &MouseEvent, rects: &Rects) -> io::Result<()> {
    let (col, row) = (mouse.column, mouse.row);
    app.mouse_col = col;
    app.mouse_row = row;

    let left_seam = panel_seam(rects.paper_list);
    let on_left_seam = row >= rects.body.y && row < rects.body.y + rects.body.height
        && col >= left_seam.0 && col <= left_seam.1 + 1;
    let right_seam = if rects.three_pane { Some(panel_seam(rects.preview_text)) } else { None };
    let on_right_seam = right_seam.map(|(s, e)| {
        row >= rects.body.y && row < rects.body.y + rects.body.height && col >= s.saturating_sub(1) && col <= e + 1
    }).unwrap_or(false);

    app.hover_left_border = on_left_seam;
    app.hover_right_border = on_right_seam;

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if in_rect(rects.preview_text, col, row) {
                app.preview_scroll = app.preview_scroll.saturating_add(3);
            } else if rects.three_pane && in_rect(rects.chat, col, row) {
                app.chat_scroll = app.chat_scroll.saturating_add(3);
            } else if in_rect(rects.paper_list, col, row) && !app.papers.is_empty() {
                app.paper_list_selected = (app.paper_list_selected + 1).min(app.papers.len().saturating_sub(1));
            }
        }
        MouseEventKind::ScrollUp => {
            if in_rect(rects.preview_text, col, row) {
                app.preview_scroll = app.preview_scroll.saturating_sub(3);
            } else if rects.three_pane && in_rect(rects.chat, col, row) {
                app.chat_scroll = app.chat_scroll.saturating_sub(3);
            } else if in_rect(rects.paper_list, col, row) {
                app.paper_list_selected = app.paper_list_selected.saturating_sub(1);
            }
        }

        MouseEventKind::Down(MouseButton::Left) => {
            app.just_selected = false;

            if on_left_seam {
                app.dragging_border = Some(0);
                return Ok(());
            }
            if on_right_seam {
                app.dragging_border = Some(1);
                return Ok(());
            }

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
                let tab_idx = (rel_col / 8).clamp(0, 3) as usize;
                app.view_mode = [ViewMode::Body, ViewMode::Appendix, ViewMode::Note, ViewMode::Description][tab_idx];
            } else if in_rect(rects.chat_input, col, row) {
                app.show_chat = true;
                app.input_mode = InputMode::Chat;
                app.chat_cursor = col.saturating_sub(rects.chat_input.x + 2).min(app.chat_input.len() as u16) as usize;
            } else if in_rect(rects.chat, col, row) {
                app.show_chat = true;
                app.input_mode = InputMode::Chat;
            } else if in_rect(rects.preview_text, col, row) {
                if app.input_mode == InputMode::Chat { app.input_mode = InputMode::Normal; }
                else {
                    // set cursor at click position
                    let rel_x = col.saturating_sub(rects.preview_text.x + 1);
                    let rel_y = row.saturating_sub(rects.preview_text.y + 1);
                    let text = app.current_text();
                    let lines: Vec<&str> = text.lines().collect();
                    let byte_pos = if (rel_y as usize) < lines.len() {
                        let line_start: usize = lines.iter().take(rel_y as usize).map(|l| l.len() + 1).sum();
                        line_start + rel_x.min(lines[rel_y as usize].len() as u16).saturating_sub(1) as usize
                    } else { 0 };
                    let cb = |s: &str, idx: usize| { let i = idx.min(s.len()); let mut j = i; while j > 0 && !s.is_char_boundary(j) { j -= 1; } j };
                    app.preview_cursor = cb(text, byte_pos);
                    app.selecting = true;
                    app.sel_start = Some((rel_x, rel_y));
                    app.sel_end = None;
                }
            }
        }

        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(border_idx) = app.dragging_border {
                let body_w = rects.body.width;
                if body_w == 0 { return Ok(()); }
                if border_idx == 0 {
                    let pct = ((col.saturating_sub(rects.body.x) as u32 * 100) / body_w as u32).clamp(15, 55) as u16;
                    app.left_width_pct = pct;
                } else {
                    let pct = ((rects.body.x + body_w).saturating_sub(col) as u32 * 100 / body_w as u32).clamp(20, 50) as u16;
                    app.right_width_pct = pct;
                }
            } else if app.selecting && in_rect(rects.preview_text, col, row) {
                app.sel_end = Some((col.saturating_sub(rects.preview_text.x + 1), row.saturating_sub(rects.preview_text.y + 1)));
            }
        }

        MouseEventKind::Up(MouseButton::Left) => {
            app.dragging_border = None;
            app.selecting = false;
        }

        _ => {}
    }
    Ok(())
}

fn copy_to_clipboard(text: &str) {
    if let Ok(mut c) = arboard::Clipboard::new() { let _ = c.set_text(text); }
}

fn in_rect(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

async fn handle_key(app: &mut App, key: event::KeyEvent) -> io::Result<()> {
    match app.input_mode {
        InputMode::Chat => {
            // Ctrl+V paste
            if key.code == KeyCode::Char('v') && is_ctrl(&key) {
                if let Ok(mut c) = arboard::Clipboard::new() {
                    if let Ok(t) = c.get_text() {
                        let lines = t.lines().count();
                        if lines > 1 {
                            app.chat_input.insert_str(app.chat_cursor, &format!("[Pasted ~{lines} lines]"));
                            app.chat_cursor += format!("[Pasted ~{lines} lines]").len();
                        } else {
                            app.chat_input.insert_str(app.chat_cursor, &t);
                            app.chat_cursor += t.len();
                        }
                    }
                }
                return Ok(());
            }
            match key.code {
            KeyCode::Esc => { app.input_mode = InputMode::Normal; app.show_chat = false; }
            KeyCode::Enter => {
                let msg = std::mem::take(&mut app.chat_input);
                if !msg.is_empty() {
                    app.chat_messages.push(app::ChatMsg { speaker: "user".into(), content: msg });
                    app.add_log("sending...");
                }
                app.chat_cursor = 0;
            }
            KeyCode::Backspace => { if app.chat_cursor > 0 { app.chat_cursor -= 1; app.chat_input.remove(app.chat_cursor); } }
            KeyCode::Delete => { if app.chat_cursor < app.chat_input.len() { app.chat_input.remove(app.chat_cursor); } }
            KeyCode::Left => app.chat_cursor = app.chat_cursor.saturating_sub(1),
            KeyCode::Right => app.chat_cursor = (app.chat_cursor + 1).min(app.chat_input.len()),
            KeyCode::Home => app.chat_cursor = 0,
            KeyCode::End => app.chat_cursor = app.chat_input.len(),
            KeyCode::Char(c) => { app.chat_input.insert(app.chat_cursor, c); app.chat_cursor += 1; }
            _ => {}
        }},
        InputMode::NoteEdit => {
            if key.code == KeyCode::Char('v') && is_ctrl(&key) {
                if let Ok(mut c) = arboard::Clipboard::new() {
                    if let Ok(t) = c.get_text() {
                        del_sel(app);
                        let text = app.current_text_mut();
                        let lines = t.lines().count();
                        if lines > 1 { text.push_str(&format!("[Pasted ~{lines} lines]")); }
                        else { text.push_str(&t); }
                    }
                }
                return Ok(());
            }
            match key.code {
            KeyCode::Esc => {
                if let Some(ref paper) = app.current_paper {
                    let _ = std::fs::write(paper.folder.join("note.txt"), &app.note_content);
                    app.add_log("note saved");
                }
                app.input_mode = InputMode::Normal;
                app.sel_start = None; app.sel_end = None;
            }
            KeyCode::Backspace => {
                if has_sel(app) { del_sel(app); }
                else { app.current_text_mut().pop(); }
            }
            KeyCode::Enter => {
                if has_sel(app) { del_sel(app); }
                app.current_text_mut().push('\n');
            }
            KeyCode::Char(c) => {
                if has_sel(app) { del_sel(app); }
                app.current_text_mut().push(c);
            }
            _ => {}
        }},
        InputMode::Command => {
            if key.code == KeyCode::Char('v') && is_ctrl(&key) {
                if let Ok(mut c) = arboard::Clipboard::new() {
                    if let Ok(t) = c.get_text() {
                        let lines = t.lines().count();
                        if lines > 1 { app.status = format!("[Pasted ~{lines} lines]"); }
                        else { app.status = t; }
                        app.cmd_cursor = app.status.len();
                    }
                }
                return Ok(());
            }
            match key.code {
            KeyCode::Esc => { app.input_mode = InputMode::Normal; app.status.clear(); app.cmd_cursor = 1; }
            KeyCode::Enter => {
                let body = std::mem::take(&mut app.status).strip_prefix(':').unwrap_or("").to_string();
                app.cmd_cursor = 1; app.input_mode = InputMode::Normal;
                match body.as_str() {
                    "o"|"open" => if let Some(ref p) = app.current_paper { let _ = open::that(&p.folder); }
                    "p"|"pdf" => if let Some(ref p) = app.current_paper { let _ = open::that(format!("https://arxiv.org/pdf/{}", p.arxiv_id)); }
                    "scan" => app.scan_pdfs().await,
                    "dl"|"download" => app.download_all().await,
                    _ => app.add_log(&format!("unknown: {body}")),
                }
            }
            KeyCode::Backspace => { if app.cmd_cursor > 1 { app.cmd_cursor -= 1; app.status.remove(app.cmd_cursor); } }
            KeyCode::Delete => { if app.cmd_cursor < app.status.len() { app.status.remove(app.cmd_cursor); } }
            KeyCode::Left => app.cmd_cursor = app.cmd_cursor.saturating_sub(1).max(1),
            KeyCode::Right => app.cmd_cursor = (app.cmd_cursor + 1).min(app.status.len()),
            KeyCode::Home => app.cmd_cursor = 1,
            KeyCode::End => app.cmd_cursor = app.status.len(),
            KeyCode::Char(c) => { app.status.insert(app.cmd_cursor, c); app.cmd_cursor += 1; }
            _ => {}
        }},
        InputMode::Normal => {
            // Ctrl+Alt+Q toggles hotkey lock
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                && key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                && key.code == KeyCode::Char('q')
            {
                app.hotkeys_locked = !app.hotkeys_locked;
                app.add_log(if app.hotkeys_locked { "hotkeys locked" } else { "hotkeys unlocked" });
                return Ok(());
            }

            // Ctrl+C copy selection
            if key.code == KeyCode::Char('c') && is_ctrl(&key) {
                if let (Some(start), Some(end)) = (app.sel_start, app.sel_end) {
                    let (sx, sy) = start;
                    let (ex, ey) = end;
                    let (x1, y1, x2, y2) = if (sy, sx) <= (ey, ex) { (sx, sy, ex, ey) } else { (ex, ey, sx, sy) };
                    let content = app.get_sel_text(x1, y1, x2, y2);
                    if !content.is_empty() { copy_to_clipboard(&content); app.just_selected = true; }
                }
                app.sel_start = None;
                app.sel_end = None;
                return Ok(());
            }

            match key.code {
            KeyCode::Char('q') if !app.hotkeys_locked => app.quit = true,
            KeyCode::Char('q') => {} // blocked when locked
            KeyCode::Char('?') => app.add_log("j↓/k↑ enter:load 1-4:view c:chat e:edit s:scan d:dl :cmd q:quit Ctrl+Alt+Q:unlock"),
            KeyCode::Char('j') => if !app.papers.is_empty() && app.paper_list_selected + 1 < app.papers.len() { app.paper_list_selected += 1; }
            KeyCode::Char('k') => app.paper_list_selected = app.paper_list_selected.saturating_sub(1),
            KeyCode::Enter => if !app.papers.is_empty() { let idx = app.paper_list_selected; app.load_paper(idx); app.show_chat = true; }
            KeyCode::Char('1') if !app.hotkeys_locked => app.view_mode = ViewMode::Body,
            KeyCode::Char('2') if !app.hotkeys_locked => app.view_mode = ViewMode::Appendix,
            KeyCode::Char('3') if !app.hotkeys_locked => app.view_mode = ViewMode::Note,
            KeyCode::Char('4') if !app.hotkeys_locked => app.view_mode = ViewMode::Description,
            KeyCode::Char('e') if !app.hotkeys_locked => if app.view_mode == ViewMode::Note && app.current_paper.is_some() { app.input_mode = InputMode::NoteEdit; app.add_log("editing note (Esc to save)"); }
            KeyCode::Char('c') if !app.hotkeys_locked => if app.current_paper.is_some() { app.show_chat = !app.show_chat; app.input_mode = if app.show_chat { InputMode::Chat } else { InputMode::Normal }; }
            KeyCode::Char('s') if !app.hotkeys_locked => app.scan_pdfs().await,
            KeyCode::Char('d') if !app.hotkeys_locked => app.download_all().await,
            KeyCode::Char('o') if !app.hotkeys_locked => if let Some(ref p) = app.current_paper { let _ = open::that(&p.folder); app.add_log(&format!("opened {}", p.folder.display())); }
            KeyCode::Char('p') if !app.hotkeys_locked => if let Some(ref p) = app.current_paper { let _ = open::that(format!("https://arxiv.org/pdf/{}", p.arxiv_id)); }
            KeyCode::Char(':') if !app.hotkeys_locked => { app.input_mode = InputMode::Command; app.status = ":".into(); }
            // Arrow keys in preview move cursor
            KeyCode::Left => {
                let t = app.current_text().to_string();
                let mut c = app.preview_cursor;
                if c > 0 { c -= 1; while c > 0 && !t.is_char_boundary(c) { c -= 1; } }
                app.preview_cursor = c;
            }
            KeyCode::Right => {
                let t = app.current_text().to_string();
                let mut c = app.preview_cursor;
                if c < t.len() { c += 1; while c < t.len() && !t.is_char_boundary(c) { c += 1; } }
                app.preview_cursor = c;
            }
            KeyCode::Up => {
                let t = app.current_text().to_string();
                let c = app.preview_cursor;
                let line_start = t[..c].rfind('\n').map(|i| i + 1).unwrap_or(0);
                if line_start > 0 {
                    let prev_start = t[..line_start-1].rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let col = c - line_start;
                    app.preview_cursor = (prev_start + col).min(line_start - 1);
                }
            }
            KeyCode::Down => {
                let t = app.current_text().to_string();
                let c = app.preview_cursor;
                let line_end = t[c..].find('\n').map(|i| c + i + 1).unwrap_or(t.len());
                if line_end < t.len() {
                    let next_end = t[line_end..].find('\n').map(|i| line_end + i + 1).unwrap_or(t.len());
                    let col = c.saturating_sub(t[..c].rfind('\n').map(|i| i + 1).unwrap_or(0));
                    app.preview_cursor = (line_end + col).min(next_end - 1);
                }
            }
            KeyCode::Home => {
                let t = app.current_text().to_string();
                let c = app.preview_cursor;
                app.preview_cursor = t[..c].rfind('\n').map(|i| i + 1).unwrap_or(0);
            }
            KeyCode::End => {
                let t = app.current_text().to_string();
                let c = app.preview_cursor;
                app.preview_cursor = t[c..].find('\n').map(|i| c + i).unwrap_or(t.len());
            }
            _ => {}
            }
        },
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) -> Rects {
    let mut rects = Rects::default();
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());
    rects.header = main[0];
    rects.status = main[2];
    render_header(f, app, rects.header);

    let body = main[1];
    rects.body = body;
    let three_pane = app.show_chat && app.current_paper.is_some();
    rects.three_pane = three_pane;

    let left_pct = app.left_width_pct;
    let right_pct = app.right_width_pct;
    let mid_pct = if three_pane {
        100u16.saturating_sub(left_pct).saturating_sub(right_pct)
    } else {
        100u16.saturating_sub(left_pct)
    };

    if three_pane {
        let h = Layout::default().direction(Direction::Horizontal).constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(mid_pct),
            Constraint::Percentage(right_pct),
        ]).split(body);
        rects.paper_list = h[0];
        let prev_area = h[1];
        rects.chat = h[2];

        rects.preview_tabs = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(prev_area)[0];
        rects.preview_text = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(prev_area)[1];
        let cc = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(rects.chat);
        rects.chat_input = cc[1];
    } else {
        let h = Layout::default().direction(Direction::Horizontal).constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(mid_pct),
        ]).split(body);
        rects.paper_list = h[0];
        let prev_area = h[1];

        rects.preview_tabs = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(prev_area)[0];
        rects.preview_text = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(prev_area)[1];
    }

    render_paper_list(f, app, rects.paper_list);
    render_preview(f, app, rects.preview_text, rects.preview_tabs);
    if three_pane { render_chat(f, app, rects.chat, rects.chat_input); }
    render_status(f, app, rects.status);
    rects
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.workspace_path_str.is_empty() { "ArxivCat TUI — no workspace".into() }
    else { format!("ArxivCat TUI — {}  papers:{}", app.workspace_path_str, app.papers.len()) };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::Rgb(137, 180, 250)))
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(49, 50, 68)))),
        area,
    );
}

fn render_paper_list(f: &mut Frame, app: &App, area: Rect) {
    let hovered = in_rect(area, app.mouse_col, app.mouse_row);
    let items: Vec<ListItem> = app.papers.iter().enumerate().map(|(i, p)| {
        let icon = if p.is_complete { "●" } else if p.has_body { "○" } else { "·" };
        let is_sel = i == app.paper_list_selected;
        let is_hov = app.mouse_row as usize == area.y as usize + 1 + i && app.mouse_col >= area.x && app.mouse_col < area.x + area.width;
        let style = if is_sel {
            Style::default().fg(Color::Rgb(205, 214, 244)).bg(Color::Rgb(69, 71, 90))
        } else if is_hov {
            Style::default().fg(Color::Rgb(137, 180, 250)).bg(Color::Rgb(49, 50, 68)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(166, 173, 200))
        };
        ListItem::new(format!("{icon} {:<14} {}", p.arxiv_id, p.title)).style(style)
    }).collect();

    let border = if hovered { Style::default().fg(Color::Rgb(108, 112, 134)) } else { Style::default().fg(Color::Rgb(49, 50, 68)) };
    f.render_stateful_widget(
        List::new(items).block(Block::default().title(" Papers ").borders(Borders::ALL).border_style(border)),
        area,
        &mut ListState::default().with_selected(Some(app.paper_list_selected)).with_offset(app.paper_list_scroll),
    );
}

fn render_preview(f: &mut Frame, app: &mut App, text_area: Rect, tab_area: Rect) {
    app.text_line_width = text_area.width.saturating_sub(2);
    let text = if app.current_paper.is_some() { app.current_text() } else { "← Click a paper or press Enter to load" };
    let content = if text.is_empty() { "(empty)" } else { text };

    let chars = content.chars().count();
    let lines_c = content.lines().count();
    let hovered = in_rect(text_area, app.mouse_col, app.mouse_row);
    let border = if hovered { Style::default().fg(Color::Rgb(108, 112, 134)) } else { Style::default().fg(Color::Rgb(49, 50, 68)) };

    // Build styled text with cursor
    let styled: ratatui::text::Text = {
        let normal = Style::default().fg(Color::Rgb(205, 214, 244));
        let cursor_style = Style::default().fg(Color::Rgb(30, 30, 46)).bg(Color::Rgb(137, 180, 250));
        let sel_style = Style::default().fg(Color::Rgb(205, 214, 244)).bg(Color::Rgb(69, 71, 90));

        let has_sel = app.selecting && app.sel_start.is_some();
        let (x1, y1, x2, y2) = if has_sel {
            let (sx, sy) = app.sel_start.unwrap();
            let (ex, ey) = app.sel_end.unwrap_or((sx, sy));
            if (sy, sx) <= (ey, ex) { (sx as usize, sy as usize, ex as usize, ey as usize) }
            else { (ex as usize, ey as usize, sx as usize, sy as usize) }
        } else { (0, 0, 0, 0) };

        fn cb(s: &str, idx: usize) -> usize { let i = idx.min(s.len()); let mut j = i; while j > 0 && !s.is_char_boundary(j) { j -= 1; } j }

        let lines: Vec<TLine> = content.lines().enumerate().map(|(li, line)| {
            let cur_start = if app.current_paper.is_some() { app.preview_cursor } else { usize::MAX };
            let total_before: usize = content.lines().take(li).map(|l| l.len() + 1).sum();
            let cursor_in_line = if cur_start >= total_before && cur_start < total_before + line.len() + 1 {
                Some(cur_start - total_before)
            } else { None };

            let in_sel = has_sel && li >= y1 && li <= y2;
            let sel_start = if li == y1 { cb(line, x1) } else { 0 };
            let sel_end = if li == y2 { cb(line, x2) } else { line.len() };

            let mut spans: Vec<Span> = Vec::new();
            let mut pos = 0;
            while pos < line.len() {
                let sel_in_range = in_sel && pos >= sel_start && pos < sel_end;

                let mut end = line.len();
                if let Some(cp) = cursor_in_line { if cp > pos && cp < end { end = cp; } }
                if in_sel && sel_in_range {
                    if sel_end > pos && sel_end < end { end = sel_end; }
                } else {
                    if sel_start > pos && sel_start < end { end = sel_start; }
                }
                // Find next cursor position
                if let Some(cp) = cursor_in_line {
                    if cp == pos {
                        let ch = line[cp..].chars().next().unwrap_or(' ');
                        let clen = ch.len_utf8();
                        spans.push(Span::styled(ch.to_string(), cursor_style));
                        pos = cp + clen;
                        continue;
                    }
                }

                let style = if in_sel && pos >= sel_start && pos < sel_end { sel_style } else { normal };
                spans.push(Span::styled(line[pos..end].to_string(), style));
                pos = end;
            }
            // Handle cursor at end of line
            if let Some(cp) = cursor_in_line {
                if cp >= line.len() {
                    spans.push(Span::styled(" ".to_string(), cursor_style));
                }
            }
            TLine::from(spans)
        }).collect();
        ratatui::text::Text::from(lines)
    };

    f.render_widget(
        Paragraph::new(styled)
            .block(Block::default().title(format!(" {} chars · {} lines ", chars, lines_c))
                .title_bottom(if app.hotkeys_locked { "🔒 Ctrl+Alt+Q unlock | scroll ↑↓/wheel →/← " } else { "🔓 unlocked | scroll ↑↓/wheel →/← " })
                .borders(Borders::ALL).border_style(border))
            .wrap(Wrap { trim: true })
            .scroll((app.preview_scroll, app.preview_hscroll)),
        text_area,
    );

    // Tab buttons
    let tab_labels = ["Body", "Appdx", "Note", "Desc"];
    let tab_widths = [6u16, 7u16, 6u16, 6u16];
    let mut tx = tab_area.x;
    for i in 0..4 {
        let w = tab_widths[i] + 2;
        let tr = Rect::new(tx, tab_area.y, w, 1);
        let active = i == app.view_mode as usize;
        let hover_tab = app.mouse_col >= tr.x && app.mouse_col < tr.x + tr.width && app.mouse_row == tr.y;
        let tab_style = if active {
            Style::default().fg(Color::Rgb(30, 30, 46)).bg(Color::Rgb(137, 180, 250)).add_modifier(Modifier::BOLD)
        } else if hover_tab {
            Style::default().fg(Color::Rgb(205, 214, 244)).bg(Color::Rgb(69, 71, 90))
        } else {
            Style::default().fg(Color::Rgb(108, 112, 134)).bg(Color::Rgb(30, 30, 46))
        };
        f.render_widget(Paragraph::new(format!(" {} ", tab_labels[i])).style(tab_style), tr);
        tx += w;
    }
}

fn render_chat(f: &mut Frame, app: &App, area: Rect, input_area: Rect) {
    let inner = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(3)]).split(area);
    let hovered = in_rect(area, app.mouse_col, app.mouse_row);
    let border = if hovered { Style::default().fg(Color::Rgb(108, 112, 134)) } else { Style::default().fg(Color::Rgb(49, 50, 68)) };

    let msgs: Vec<ListItem> = app.chat_messages.iter().map(|m| {
        let style = if m.speaker == "user" { Style::default().fg(Color::Rgb(137, 180, 250)) } else { Style::default().fg(Color::Rgb(205, 214, 244)) };
        let prefix = if m.speaker == "user" { "You" } else { "AI" };
        ListItem::new(format!("<{prefix}> {}", m.content)).style(style)
    }).collect();
    f.render_widget(List::new(msgs).block(Block::default().title(format!(" Chat [{}] {} ", app.chat_model, if app.deep_thinking { "🧠" } else { "" })).borders(Borders::ALL).border_style(border)), inner[0]);

    let input_hover = app.mouse_col >= input_area.x + 1 && app.mouse_col < input_area.x + input_area.width && app.mouse_row >= input_area.y && app.mouse_row < input_area.y + input_area.height;
    let input_style = if app.input_mode == InputMode::Chat { Style::default().fg(Color::Rgb(205, 214, 244)).bg(Color::Rgb(49, 50, 68)) }
    else if input_hover { Style::default().fg(Color::Rgb(166, 173, 200)).bg(Color::Rgb(40, 40, 56)) }
    else { Style::default().fg(Color::Rgb(108, 112, 134)) };
    let border_color = if app.input_mode == InputMode::Chat { Color::Rgb(137, 180, 250) } else if input_hover { Color::Rgb(108, 112, 134) } else { Color::Rgb(49, 50, 68) };
    f.render_widget(
        Paragraph::new(build_cursor_text(&app.chat_input, app.chat_cursor, "> ", app.input_mode == InputMode::Chat))
            .style(input_style).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color))),
        input_area,
    );
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let mode = match app.input_mode { InputMode::Normal => "NORMAL", InputMode::Chat => "CHAT", InputMode::NoteEdit => "EDIT", InputMode::Command => "CMD" };
    let txt: ratatui::text::Text = if app.input_mode == InputMode::Command { build_cursor_text(&app.status, app.cmd_cursor, "", true) }
    else if app.just_selected { ratatui::text::Text::from("[copied to clipboard]") }
    else if !app.status.is_empty() { ratatui::text::Text::from(app.status.as_str()) }
    else {
        let hint = if app.current_paper.is_some() { " | c:chat" } else { "" };
        ratatui::text::Text::from(format!("[{mode}] ↑↓/scroll/jk:nav 1-4:view{hint} e:edit s:scan d:dl :cmd q:quit ?:help"))
    };
    let hovered = in_rect(area, app.mouse_col, app.mouse_row);
    f.render_widget(
        Paragraph::new(txt).style(if hovered { Style::default().fg(Color::Rgb(205, 214, 244)) } else { Style::default().fg(Color::Rgb(166, 173, 200)) })
            .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Rgb(49, 50, 68)))),
        area,
    );
}

fn build_cursor_text(text: &str, cursor: usize, prefix: &str, active: bool) -> ratatui::text::Text<'static> {
    if !active { return ratatui::text::Text::from(format!("{prefix}{text}")); }
    let mut spans: Vec<Span<'static>> = vec![Span::raw(prefix.to_string())];
    let before = &text[..cursor.min(text.len())];
    let cur_char = if cursor < text.len() { text[cursor..].chars().next().unwrap_or(' ') } else { ' ' };
    let after = if cursor < text.len() { let rest = &text[cursor..]; let l = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(0); &rest[l..] } else { "" };
    spans.push(Span::raw(before.to_string()));
    spans.push(Span::styled(cur_char.to_string(), Style::default().fg(Color::Rgb(30, 30, 46)).bg(Color::Rgb(137, 180, 250))));
    if !after.is_empty() { spans.push(Span::raw(after.to_string())); }
    ratatui::text::Text::from(TLine::from(spans))
}

fn is_ctrl(key: &event::KeyEvent) -> bool {
    key.modifiers == crossterm::event::KeyModifiers::CONTROL
}

fn has_sel(app: &App) -> bool {
    app.sel_start.is_some() && app.sel_end.is_some()
}

fn del_sel(app: &mut App) {
    if let (Some(start), Some(end)) = (app.sel_start, app.sel_end) {
        let (sx, sy) = start;
        let (ex, ey) = end;
        let (x1, y1, x2, y2) = if (sy, sx) <= (ey, ex) { (sx, sy, ex, ey) } else { (ex, ey, sx, sy) };
        let selected = app.get_sel_text(x1, y1, x2, y2);
        let text = app.current_text_mut();
        if let Some(pos) = text.find(&selected) {
            text.replace_range(pos..pos + selected.len(), "");
        } else {
            let lines: Vec<&str> = text.lines().collect();
            let sy = y1.min(y2) as usize;
            let ey = y1.max(y2) as usize;
            if sy < lines.len() {
                let start: usize = lines.iter().take(sy).map(|l| l.len() + 1).sum();
                let end: usize = lines.iter().take((ey + 1).min(lines.len())).map(|l| l.len() + 1).sum::<usize>().saturating_sub(1);
                text.replace_range(start..end.min(text.len()), "");
            }
        }
    }
    app.sel_start = None;
    app.sel_end = None;
}
