use arxivcat_core::config;
use arxivcat_core::workspace::{Paper, Workspace};

pub struct App {
    pub workspace: Option<Workspace>,
    pub papers: Vec<Paper>,
    pub current_paper: Option<Paper>,
    pub preview_content: String,
    pub note_content: String,
    pub desc_content: String,
    pub appendix_content: String,
    pub view_mode: ViewMode,
    pub paper_list_scroll: usize,
    pub paper_list_selected: usize,
    pub preview_scroll: u16,
    pub show_chat: bool,
    pub chat_input: String,
    pub chat_messages: Vec<ChatMsg>,
    #[allow(dead_code)]
    pub chat_scroll: usize,
    #[allow(dead_code)]
    pub chat_streaming: bool,
    pub chat_model: String,
    pub chat_cursor: usize,
    pub cmd_cursor: usize,
    pub deep_thinking: bool,
    pub log_lines: Vec<String>,
    pub status: String,
    pub input_mode: InputMode,
    pub quit: bool,
    pub workspace_path_str: String,
    pub selecting: bool,
    pub sel_start: Option<(u16, u16)>,
    pub sel_end: Option<(u16, u16)>,
    pub just_selected: bool,
    pub mouse_col: u16,
    pub mouse_row: u16,
    pub left_width_pct: u16,
    pub right_width_pct: u16,
    pub dragging_border: Option<usize>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Body,
    Appendix,
    Note,
    Description,
}

#[derive(Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Chat,
    NoteEdit,
    Command,
}

pub struct ChatMsg {
    pub speaker: String,
    pub content: String,
}

impl App {
    pub fn new() -> Self {
        let model = config::load_model_preference();
        Self {
            workspace: None,
            papers: Vec::new(),
            current_paper: None,
            preview_content: String::new(),
            note_content: String::new(),
            desc_content: String::new(),
            appendix_content: String::new(),
            view_mode: ViewMode::Body,
            paper_list_scroll: 0,
            paper_list_selected: 0,
            preview_scroll: 0,
            show_chat: false,
            chat_input: String::new(),
            chat_messages: Vec::new(),
            chat_scroll: 0,
            chat_streaming: false,
            chat_model: model,
            chat_cursor: 0,
            cmd_cursor: 1,
            deep_thinking: true,
            log_lines: Vec::new(),
            status: String::from("Press ? for help, q to quit"),
            input_mode: InputMode::Normal,
            quit: false,
            workspace_path_str: String::new(),
            selecting: false,
            sel_start: None,
            sel_end: None,
            just_selected: false,
            mouse_col: 0,
            mouse_row: 0,
            left_width_pct: 25,
            right_width_pct: 35,
            dragging_border: None,
        }
    }

    pub fn add_log(&mut self, msg: &str) {
        self.log_lines.push(msg.to_string());
        if self.log_lines.len() > 200 {
            self.log_lines.remove(0);
        }
    }

    pub fn load_paper(&mut self, idx: usize) {
        if idx >= self.papers.len() {
            return;
        }
        let paper = &self.papers[idx];
        let folder = &paper.folder;

        self.preview_content =
            std::fs::read_to_string(folder.join("body.tex")).unwrap_or_default();
        self.appendix_content =
            std::fs::read_to_string(folder.join("appendix.tex")).unwrap_or_default();
        self.desc_content =
            std::fs::read_to_string(folder.join("description.md")).unwrap_or_default();
        self.note_content =
            std::fs::read_to_string(folder.join("note.txt")).unwrap_or_default();

        self.current_paper = Some(paper.clone());
        self.view_mode = ViewMode::Body;
        self.preview_scroll = 0;
    }

    pub fn current_text(&self) -> &str {
        match self.view_mode {
            ViewMode::Body => &self.preview_content,
            ViewMode::Appendix => &self.appendix_content,
            ViewMode::Note => &self.note_content,
            ViewMode::Description => &self.desc_content,
        }
    }

    pub fn current_text_mut(&mut self) -> &mut String {
        match self.view_mode {
            ViewMode::Body => &mut self.preview_content,
            ViewMode::Appendix => &mut self.appendix_content,
            ViewMode::Note => &mut self.note_content,
            ViewMode::Description => &mut self.desc_content,
        }
    }

    pub async fn scan_pdfs(&mut self) {
        self.add_log("scanning workspace for PDFs...");
        if let Some(ref mut ws) = self.workspace {
            let result = arxivcat_core::workspace::scan_workspace_pdfs(ws).await;
            match result {
                Ok(count) => {
                    self.papers = ws.papers.clone();
                    self.add_log(&format!("found {count} new papers"));
                }
                Err(e) => self.add_log(&format!("scan error: {e}")),
            }
        }
    }

    pub async fn download_all(&mut self) {
        if self.workspace.is_none() {
            return;
        }
        let ws = self.workspace.as_ref().unwrap();
        let pending: Vec<_> = ws.pending_papers().into_iter().cloned().collect();
        let ws_path = ws.path.clone();
        let pending_count = pending.len();

        if pending_count == 0 {
            self.add_log("all papers complete");
            return;
        }
        self.add_log(&format!("downloading {} papers...", pending_count));

        let downloads_dir = config::get_downloads_dir();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut done = 0usize;

        for (i, p) in pending.iter().enumerate() {
            self.status = format!("[{}/{}] {} ...", i + 1, pending_count, p.arxiv_id);
            match arxivcat_core::workspace::process_pending_paper(
                p, &downloads_dir, &ws_path, &cancel,
            )
            .await
            {
                Ok(true) => {
                    done += 1;
                    self.add_log(&format!("ok: {}", p.arxiv_id));
                }
                Ok(false) => self.add_log(&format!("skip: {}", p.arxiv_id)),
                Err(e) => self.add_log(&format!("error {}: {e}", p.arxiv_id)),
            }
        }

        self.status = format!("done: {done}/{}", pending_count);
        if let Some(ref mut ws) = self.workspace {
            ws.refresh();
            self.papers = ws.papers.clone();
        }
    }
}
