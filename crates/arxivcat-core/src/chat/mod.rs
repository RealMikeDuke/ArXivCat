pub mod deepseek;
pub mod description;
pub mod session;

use std::path::Path;

use crate::workspace::Paper;

pub struct ChatContext {
    pub body: String,
    pub appendix: String,
    pub description: String,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextSelection {
    pub body: bool,
    pub appendix: bool,
    pub description: bool,
    pub note: bool,
}

impl Default for ContextSelection {
    fn default() -> Self {
        Self {
            body: true,
            appendix: false,
            description: false,
            note: false,
        }
    }
}

pub fn build_side_chat_context(paper_dir: &Path, selection: &ContextSelection) -> String {
    let mut parts: Vec<String> = Vec::new();

    if selection.body {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("body.tex")) {
            parts.push(format!("body:\n{}", content));
        }
    }
    if selection.appendix {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("appendix.tex")) {
            parts.push(format!("appendix:\n{}", content));
        }
    }
    if selection.description {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("description.md")) {
            parts.push(format!("description:\n{}", content));
        }
    }
    if selection.note {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("note.txt")) {
            parts.push(format!("note:\n{}", content));
        }
    }

    if parts.is_empty() {
        "(no context selected)".to_string()
    } else {
        parts.join("\n\n")
    }
}

pub fn build_global_chat_context(papers: &[Paper]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, paper) in papers.iter().enumerate() {
        let desc_path = paper.folder.join("description.md");
        if let Ok(content) = std::fs::read_to_string(&desc_path) {
            parts.push(format!(
                "Paper [{}]\narXiv ID: {}\nTitle: {}\n---\n{}",
                i + 1,
                paper.arxiv_id,
                paper.title,
                content
            ));
        }
    }
    if parts.is_empty() {
        "(no descriptions found)".to_string()
    } else {
        parts.join("\n\n---\n\n")
    }
}

pub fn compute_selection_delta(
    current: &ContextSelection,
    last_sent: &ContextSelection,
) -> ContextSelection {
    ContextSelection {
        body: current.body && !last_sent.body,
        appendix: current.appendix && !last_sent.appendix,
        description: current.description && !last_sent.description,
        note: current.note && !last_sent.note,
    }
}
