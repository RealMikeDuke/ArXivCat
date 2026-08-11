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

pub fn build_global_chat_context(papers: &[Paper], selection: &ContextSelection) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, paper) in papers.iter().enumerate() {
        let mut sections: Vec<String> = Vec::new();

        if selection.body {
            if let Ok(content) = std::fs::read_to_string(paper.folder.join("body.tex")) {
                sections.push(format!("body:\n{}", content));
            }
        }
        if selection.appendix {
            if let Ok(content) = std::fs::read_to_string(paper.folder.join("appendix.tex")) {
                sections.push(format!("appendix:\n{}", content));
            }
        }
        if selection.description {
            let desc_path = paper.folder.join("description.md");
            if let Ok(content) = std::fs::read_to_string(&desc_path) {
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
            body: false, appendix: false, description: false, note: false,
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
            body: true, appendix: false, description: false, note: false,
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
            body: true, appendix: false, description: false, note: false,
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
            body: true, appendix: false, description: false, note: true,
        };
        let ctx = build_global_chat_context(&papers, &sel);
        assert!(ctx.contains("body text"));
        assert!(ctx.contains("my note"));
    }

    #[test]
    fn test_compute_selection_delta_only_new_fields() {
        let current = ContextSelection {
            body: true, appendix: true, description: false, note: false,
        };
        let last_sent = ContextSelection {
            body: true, appendix: false, description: false, note: false,
        };
        let delta = compute_selection_delta(&current, &last_sent);
        assert!(!delta.body);      // already sent
        assert!(delta.appendix);   // new
        assert!(!delta.description);
        assert!(!delta.note);
    }

    #[test]
    fn test_compute_selection_delta_all_new() {
        let current = ContextSelection {
            body: true, appendix: true, description: true, note: true,
        };
        let last_sent = ContextSelection {
            body: false, appendix: false, description: false, note: false,
        };
        let delta = compute_selection_delta(&current, &last_sent);
        assert!(delta.body);
        assert!(delta.appendix);
        assert!(delta.description);
        assert!(delta.note);
    }

    #[test]
    fn test_compute_selection_delta_unselected_ignored() {
        let current = ContextSelection {
            body: false, appendix: false, description: false, note: false,
        };
        let last_sent = ContextSelection {
            body: true, appendix: true, description: true, note: true,
        };
        let delta = compute_selection_delta(&current, &last_sent);
        assert!(!delta.body);
        assert!(!delta.appendix);
        assert!(!delta.description);
        assert!(!delta.note);
    }
}
