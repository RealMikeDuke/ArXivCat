use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::chat::ContextSelection;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub speaker: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    #[serde(skip)]
    pub path: PathBuf,

    pub title: String,
    pub kind: String,
    pub model: String,
    pub deep_thinking: bool,
    pub messages: Vec<ChatMessage>,
    pub context_selection: ContextSelection,
    pub context_snapshot: String,
    pub view_name: String,
    pub updated_at: String,
}

impl ChatSession {
    pub fn new(kind: &str, arxiv_id: &str) -> Self {
        let now = Local::now();
        let title = if kind == "global" {
            format!("Global Chat {}", now.format("%Y-%m-%d %H:%M"))
        } else {
            format!(
                "{} {}",
                if arxiv_id.is_empty() { "Paper" } else { arxiv_id },
                now.format("%Y-%m-%d %H:%M")
            )
        };

        Self {
            path: PathBuf::new(),
            title,
            kind: kind.to_string(),
            model: "Flash".to_string(),
            deep_thinking: true,
            messages: Vec::new(),
            context_selection: ContextSelection::default(),
            context_snapshot: String::new(),
            view_name: "body".to_string(),
            updated_at: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        }
    }
}

pub fn new_session_path(session_dir: &Path) -> PathBuf {
    let now = Local::now();
    let base = now.format("%Y%m%d_%H%M%S").to_string();

    let mut path = session_dir.join(format!("{base}.json"));
    let mut suffix = 1;
    while path.exists() {
        path = session_dir.join(format!("{base}_{suffix}.json"));
        suffix += 1;
    }
    path
}

pub fn save_session(session: &mut ChatSession, session_dir: Option<&Path>) -> Result<()> {
    if session.messages.is_empty() {
        return Ok(());
    }

    let dir = session_dir.unwrap_or_else(|| Path::new("."));

    if session.path.as_os_str().is_empty() || !session.path.exists() {
        session.path = new_session_path(dir);
    }

    session.updated_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let json = serde_json::to_string_pretty(session)?;
    if let Some(parent) = session.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&session.path, json)?;
    Ok(())
}

pub fn load_session(path: &Path) -> Result<ChatSession> {
    let content = std::fs::read_to_string(path)?;
    let mut session: ChatSession = serde_json::from_str(&content)?;
    session.path = path.to_path_buf();
    Ok(session)
}

pub fn list_sessions(session_dir: &Path) -> Result<Vec<ChatSession>> {
    if !session_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let glob_pattern = format!("{}/*.json", session_dir.display());
    if let Ok(entries) = glob::glob(&glob_pattern) {
        let mut paths: Vec<PathBuf> = entries.flatten().collect();
        paths.sort_by(|a, b| {
            b.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .cmp(&a.metadata().ok().and_then(|m| m.modified().ok()))
        });

        for path in paths {
            if let Ok(session) = load_session(&path) {
                sessions.push(session);
            }
        }
    }

    Ok(sessions)
}

pub fn rename_session(path: &Path, new_title: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)?;
    let mut json: serde_json::Value = serde_json::from_str(&content)?;
    json["title"] = serde_json::Value::String(new_title.to_string());
    json["updated_at"] =
        serde_json::Value::String(Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
    std::fs::write(path, serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

pub fn delete_session(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = ChatSession::new("paper", "2501.12948");
        session.messages.push(ChatMessage {
            speaker: "user".to_string(),
            content: "hello".to_string(),
        });
        session.messages.push(ChatMessage {
            speaker: "assistant".to_string(),
            content: "hi there".to_string(),
        });

        save_session(&mut session, Some(dir.path())).unwrap();
        assert!(session.path.exists());

        let loaded = load_session(&session.path).unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "hello");
    }

    #[test]
    fn test_list_sessions_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let mut s1 = ChatSession::new("paper", "id1");
        s1.messages.push(ChatMessage {
            speaker: "user".to_string(),
            content: "first".to_string(),
        });
        save_session(&mut s1, Some(dir.path())).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut s2 = ChatSession::new("paper", "id2");
        s2.messages.push(ChatMessage {
            speaker: "user".to_string(),
            content: "second".to_string(),
        });
        save_session(&mut s2, Some(dir.path())).unwrap();

        let sessions = list_sessions(dir.path()).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_rename_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = ChatSession::new("paper", "test");
        session.messages.push(ChatMessage {
            speaker: "user".to_string(),
            content: "test".to_string(),
        });
        save_session(&mut session, Some(dir.path())).unwrap();

        rename_session(&session.path, "New Title").unwrap();

        let loaded = load_session(&session.path).unwrap();
        assert_eq!(loaded.title, "New Title");
    }

    #[test]
    fn test_delete_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = ChatSession::new("paper", "test");
        session.messages.push(ChatMessage {
            speaker: "user".to_string(),
            content: "bye".to_string(),
        });
        save_session(&mut session, Some(dir.path())).unwrap();

        assert!(delete_session(&session.path).unwrap());
        assert!(!session.path.exists());
    }
}
