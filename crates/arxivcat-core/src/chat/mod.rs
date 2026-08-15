pub mod deepseek;
pub mod description;
pub mod session;
pub mod summary;

use std::path::Path;

use crate::workspace::Paper;

pub struct ChatContext {
    pub body: String,
    pub appendix: String,
    pub description: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ContextSelection {
    #[serde(default)]
    pub body: bool,
    #[serde(default)]
    pub appendix: bool,
    #[serde(default)]
    pub description: bool,
    #[serde(default)]
    pub note: bool,
}

/// Cap per-file context at ~120k chars (~30k tokens) so huge papers cannot
/// blow the model token limit or the request size.
const MAX_CONTEXT_CHARS: usize = 120_000;

fn truncate_context(content: &str) -> String {
    if content.chars().count() > MAX_CONTEXT_CHARS {
        let head: String = content.chars().take(MAX_CONTEXT_CHARS).collect();
        format!("{head}\n\n...[truncated by arxivcat]")
    } else {
        content.to_string()
    }
}

/// Brief summary content: `brief_summary.md` is canonical (since the
/// brief/deep pipeline), falling back to the legacy `description.md`.
fn read_brief(paper_dir: &std::path::Path) -> std::io::Result<String> {
    let brief = paper_dir.join("brief_summary.md");
    if brief.exists() {
        let content = std::fs::read_to_string(&brief)?;
        // An empty stub must not shadow a real legacy description.md
        // (jury-burst R3 MINOR: read path does not trigger lazy migration).
        if !content.trim().is_empty() {
            return Ok(content);
        }
    }
    std::fs::read_to_string(paper_dir.join("description.md"))
}

pub fn build_side_chat_context(paper_dir: &Path, selection: &ContextSelection) -> String {
    let mut parts: Vec<String> = Vec::new();

    if selection.body {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("body.tex")) {
            parts.push(format!("body:\n{}", truncate_context(&content)));
        }
    }
    if selection.appendix {
        if let Ok(content) = std::fs::read_to_string(paper_dir.join("appendix.tex")) {
            parts.push(format!("appendix:\n{}", truncate_context(&content)));
        }
    }
    if selection.description {
        if let Ok(content) = read_brief(paper_dir) {
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

pub fn build_global_chat_context(papers: &[Paper], selection: &ContextSelection) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, paper) in papers.iter().enumerate() {
        let mut sections: Vec<String> = Vec::new();

        if selection.body {
            if let Ok(content) = std::fs::read_to_string(paper.folder.join("body.tex")) {
                sections.push(format!("body:\n{}", truncate_context(&content)));
            }
        }
        if selection.appendix {
            if let Ok(content) = std::fs::read_to_string(paper.folder.join("appendix.tex")) {
                sections.push(format!("appendix:\n{}", truncate_context(&content)));
            }
        }
        if selection.description {
            if let Ok(content) = read_brief(&paper.folder) {
                sections.push(format!("description:\n{}", content));
            }
        }
        if selection.note {
            if let Ok(content) = std::fs::read_to_string(paper.folder.join("note.txt")) {
                sections.push(format!("note:\n{}", content));
            }
        }

        if sections.is_empty() {
            continue;
        }

        parts.push(format!(
            "Paper [{}]\narXiv ID: {}\nTitle: {}\n---\n{}",
            i + 1,
            paper.arxiv_id,
            paper.title,
            sections.join("\n\n")
        ));
    }
    if parts.is_empty() {
        "(no context selected or no matching papers)".to_string()
    } else {
        parts.join("\n\n---\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_paper(id: &str, title: &str, folder: std::path::PathBuf) -> Paper {
        Paper {
            arxiv_id: id.to_string(),
            title: title.to_string(),
            folder_name: folder.file_name().unwrap().to_string_lossy().to_string(),
            folder,
            has_body: false,
            description_ready: false,
            deep_ready: false,
            is_complete: false,
        }
    }

    #[test]
    fn test_global_context_empty_selection_returns_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test");
        std::fs::create_dir(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), "body text").unwrap();

        let papers = vec![make_paper("2501.12948", "Test", paper_dir)];
        let sel = ContextSelection {
            body: false,
            appendix: false,
            description: false,
            note: false,
        };
        let ctx = build_global_chat_context(&papers, &sel);
        assert!(ctx.contains("no context selected"));
    }

    #[test]
    fn test_global_context_body_only() {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test");
        std::fs::create_dir(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), "some body").unwrap();

        let papers = vec![make_paper("2501.12948", "Test", paper_dir)];
        let sel = ContextSelection {
            body: true,
            appendix: false,
            description: false,
            note: false,
        };
        let ctx = build_global_chat_context(&papers, &sel);
        assert!(ctx.contains("some body"));
        assert!(ctx.contains("Paper [1]"));
    }

    #[test]
    fn test_global_context_skips_paper_without_selected_fields() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("2501_12948_A");
        std::fs::create_dir(&p1).unwrap();
        std::fs::write(p1.join("description.md"), "desc A").unwrap();

        let p2 = dir.path().join("2412_04445_B");
        std::fs::create_dir(&p2).unwrap();
        std::fs::write(p2.join("body.tex"), "body B").unwrap();

        let papers = vec![
            make_paper("2501.12948", "A", p1),
            make_paper("2412.04445", "B", p2),
        ];
        let sel = ContextSelection {
            body: true,
            appendix: false,
            description: false,
            note: false,
        };
        let ctx = build_global_chat_context(&papers, &sel);
        assert!(!ctx.contains("desc A"));
        assert!(ctx.contains("body B"));
    }

    #[test]
    fn test_global_context_multiple_fields() {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test");
        std::fs::create_dir(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), "body text").unwrap();
        std::fs::write(paper_dir.join("note.txt"), "my note").unwrap();

        let papers = vec![make_paper("2501.12948", "Test", paper_dir)];
        let sel = ContextSelection {
            body: true,
            appendix: false,
            description: false,
            note: true,
        };
        let ctx = build_global_chat_context(&papers, &sel);
        assert!(ctx.contains("body text"));
        assert!(ctx.contains("my note"));
    }
}
