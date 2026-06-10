// Tauri commands - thin wrappers around arxivcat-core

use arxivcat_core::{chat, config, extract, workspace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PaperDto {
    pub arxiv_id: String,
    pub title: String,
    pub folder_name: String,
    pub has_body: bool,
    pub description_ready: bool,
    pub is_complete: bool,
}

impl From<&workspace::Paper> for PaperDto {
    fn from(p: &workspace::Paper) -> Self {
        Self {
            arxiv_id: p.arxiv_id.clone(),
            title: p.title.clone(),
            folder_name: p.folder_name.clone(),
            has_body: p.has_body,
            description_ready: p.description_ready,
            is_complete: p.is_complete,
        }
    }
}

#[tauri::command]
pub async fn extract_paper(arxiv_id: String) -> Result<String, String> {
    let downloads_dir = config::get_downloads_dir();
    let output_dir = config::get_cache_dir().join("outputs");
    let result = extract::extract_paper(&arxiv_id, &downloads_dir, &output_dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.body)
}

#[tauri::command]
pub async fn get_paper_list(workspace_path: String) -> Result<Vec<PaperDto>, String> {
    let ws = workspace::Workspace::open(std::path::Path::new(&workspace_path))
        .map_err(|e| e.to_string())?;
    Ok(ws.papers.iter().map(PaperDto::from).collect())
}

#[tauri::command]
pub async fn open_workspace(path: String) -> Result<Vec<PaperDto>, String> {
    let ws = workspace::Workspace::open(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;
    let _ = config::save_workspace_path(std::path::Path::new(&path));
    Ok(ws.papers.iter().map(PaperDto::from).collect())
}

#[tauri::command]
pub async fn load_paper(
    workspace_path: String,
    folder_name: String,
) -> Result<serde_json::Value, String> {
    let paper_dir = std::path::Path::new(&workspace_path).join(&folder_name);

    let mut result = serde_json::json!({});

    if let Ok(body) = std::fs::read_to_string(paper_dir.join("body.tex")) {
        result["body"] = serde_json::Value::String(body);
    }
    if let Ok(appendix) = std::fs::read_to_string(paper_dir.join("appendix.tex")) {
        result["appendix"] = serde_json::Value::String(appendix);
    }
    if let Ok(desc) = std::fs::read_to_string(paper_dir.join("description.md")) {
        result["description"] = serde_json::Value::String(desc);
    }
    if let Ok(note) = std::fs::read_to_string(paper_dir.join("note.txt")) {
        result["note"] = serde_json::Value::String(note);
    }

    Ok(result)
}

#[tauri::command]
pub async fn save_note(workspace_path: String, folder_name: String, content: String) -> Result<(), String> {
    let note_path = std::path::Path::new(&workspace_path)
        .join(&folder_name)
        .join("note.txt");
    std::fs::write(&note_path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn strip_comments(content: String) -> Result<String, String> {
    let re = regex::Regex::new(r"(?<!\\)%.*").map_err(|e| e.to_string())?;
    let stripped = re.replace_all(&content, "").to_string();
    let re_nl = regex::Regex::new(r"\n{3,}").map_err(|e| e.to_string())?;
    Ok(re_nl.replace_all(&stripped, "\n\n").to_string())
}

#[tauri::command]
pub async fn scan_pdfs(workspace_path: String) -> Result<usize, String> {
    let mut ws = workspace::Workspace::open(std::path::Path::new(&workspace_path))
        .map_err(|e| e.to_string())?;
    workspace::scan_workspace_pdfs(&mut ws)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_all(workspace_path: String) -> Result<usize, String> {
    // Will be implemented with proper event emission
    Err("not yet implemented".to_string())
}

#[tauri::command]
pub async fn stream_chat(messages: Vec<serde_json::Value>) -> Result<String, String> {
    Err("not yet implemented".to_string())
}

#[tauri::command]
pub async fn build_description(
    paper_dir: String,
    arxiv_id: String,
    title: String,
) -> Result<(), String> {
    chat::description::build_description(
        std::path::Path::new(&paper_dir),
        &arxiv_id,
        &title,
        None,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_token_status() -> Result<serde_json::Value, String> {
    let token = config::load_cached_token();
    let masked = token.map(|t| {
        if t.len() > 8 {
            format!("{}...{}", &t[..4], &t[t.len() - 4..])
        } else {
            "***".to_string()
        }
    });
    Ok(serde_json::json!({
        "has_token": token.is_some(),
        "masked": masked.unwrap_or_default(),
    }))
}

#[tauri::command]
pub async fn set_token(token: String) -> Result<(), String> {
    config::save_token(&token).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_token() -> Result<bool, String> {
    let token = config::load_cached_token().ok_or("no token configured")?;
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.deepseek.com/models")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.status().is_success())
}

#[tauri::command]
pub async fn get_chat_sessions(session_dir: String) -> Result<Vec<serde_json::Value>, String> {
    let sessions = chat::session::list_sessions(std::path::Path::new(&session_dir))
        .map_err(|e| e.to_string())?;
    Ok(sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "path": s.path.to_string_lossy(),
                "title": s.title,
                "kind": s.kind,
                "model": s.model,
                "deep_thinking": s.deep_thinking,
                "messages": s.messages,
                "view_name": s.view_name,
                "updated_at": s.updated_at,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn save_chat_session_data(
    session_dir: String,
    session_data: serde_json::Value,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(
        session_data["path"]
            .as_str()
            .unwrap_or("")
    );

    let title = session_data["title"].as_str().unwrap_or("Chat");
    let kind = session_data["kind"].as_str().unwrap_or("paper");
    let model = session_data["model"].as_str().unwrap_or("Flash");
    let deep_thinking = session_data["deep_thinking"].as_bool().unwrap_or(true);

    let messages: Vec<chat::session::ChatMessage> =
        serde_json::from_value(session_data["messages"].clone()).unwrap_or_default();

    let context_selection: chat::ContextSelection =
        serde_json::from_value(session_data.get("context_selection").cloned().unwrap_or_default())
            .unwrap_or_default();

    let context_snapshot = session_data["context_snapshot"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let view_name = session_data["view_name"].as_str().unwrap_or("body").to_string();

    let mut session = chat::session::ChatSession {
        path,
        title: title.to_string(),
        kind: kind.to_string(),
        model: model.to_string(),
        deep_thinking,
        messages,
        context_selection,
        context_snapshot,
        view_name,
        updated_at: String::new(),
    };

    chat::session::save_session(&mut session, Some(std::path::Path::new(&session_dir)))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_chat_session_data(path: String, new_title: String) -> Result<(), String> {
    chat::session::rename_session(std::path::Path::new(&path), &new_title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_chat_session_data(path: String) -> Result<bool, String> {
    chat::session::delete_session(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_last_workspace() -> Result<Option<String>, String> {
    Ok(config::load_workspace_path())
}

#[tauri::command]
pub async fn open_paper_folder(workspace_path: String, folder_name: String) -> Result<(), String> {
    let p = std::path::Path::new(&workspace_path).join(&folder_name);
    open::that(&p).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_paper_pdf(workspace_path: String, folder_name: String, arxiv_id: String) -> Result<(), String> {
    let folder = std::path::Path::new(&workspace_path).join(&folder_name);
    let pattern = format!("{}/*.pdf", folder.display());
    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            return open::that(&entry).map_err(|e| e.to_string());
        }
    }
    open::that(format!("https://arxiv.org/pdf/{arxiv_id}")).map_err(|e| e.to_string())
}
