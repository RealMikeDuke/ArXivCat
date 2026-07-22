use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use arxivcat_core::error::{ArxivError, ErrorLevel};
use arxivcat_core::{chat, config, extract, workspace};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

pub struct CancelState(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

impl CancelState {
    pub fn new() -> Self {
        CancelState(Arc::new(Mutex::new(HashMap::new())))
    }
}

#[derive(Serialize)]
pub struct CommandError {
    pub message: String,
    pub level: ErrorLevel,
}

fn map_err(e: ArxivError) -> String {
    serde_json::to_string(&CommandError {
        message: e.to_string(),
        level: e.level(),
    })
    .unwrap_or_else(|_| format!(r#"{{"message":"{}","level":"Toast"}}"#, e))
}

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

#[derive(Serialize)]
pub struct StreamChatResponse {
    pub session_id: String,
}

#[tauri::command]
pub async fn extract_paper(arxiv_id: String) -> Result<String, String> {
    let downloads_dir = config::get_downloads_dir();
    let output_dir = config::get_cache_dir().join("outputs");
    let result = extract::extract_paper(&arxiv_id, &downloads_dir, &output_dir)
        .await
        .map_err(map_err)?;
    Ok(result.body)
}

#[tauri::command]
pub async fn get_paper_list(workspace_path: String) -> Result<Vec<PaperDto>, String> {
    let ws = workspace::Workspace::open(std::path::Path::new(&workspace_path))
        .map_err(map_err)?;
    Ok(ws.papers.iter().map(PaperDto::from).collect())
}

#[tauri::command]
pub async fn open_workspace(path: String) -> Result<Vec<PaperDto>, String> {
    let ws = workspace::Workspace::open(std::path::Path::new(&path))
        .map_err(map_err)?;
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
    std::fs::write(&note_path, &content).map_err(|e| map_err(ArxivError::from(e)))?;
    Ok(())
}

#[tauri::command]
pub async fn save_description(workspace_path: String, folder_name: String, content: String) -> Result<(), String> {
    let desc_path = std::path::Path::new(&workspace_path)
        .join(&folder_name)
        .join("description.md");
    std::fs::write(&desc_path, &content).map_err(|e| map_err(ArxivError::from(e)))?;
    std::fs::write(desc_path.with_file_name(".description_ready"), "ok\n").map_err(|e| map_err(ArxivError::from(e)))?;
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
        .map_err(map_err)?;
    workspace::scan_workspace_pdfs(&mut ws)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn start_chat(
    app_handle: tauri::AppHandle,
    cancel_state: tauri::State<'_, CancelState>,
    messages: Vec<serde_json::Value>,
    model: String,
    reasoning_effort: String,
    paper_context: Option<String>,
) -> Result<StreamChatResponse, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    cancel_state.0.lock().map_err(|e| e.to_string())?.insert(session_id.clone(), cancel_flag.clone());

    let app = app_handle.clone();
    let mut full_messages = messages;
    if let Some(ctx) = paper_context {
        let system_content = format!("You are a helpful assistant discussing an arXiv paper.\n\nPaper context:\n{ctx}");
        full_messages.insert(0, serde_json::json!({
            "role": "system",
            "content": system_content,
        }));
    }

    let sid = session_id.clone();
    let effort = reasoning_effort.clone();
    tauri::async_runtime::spawn(async move {
        let result = chat::deepseek::stream_chat(
            &full_messages,
            &model,
            &effort,
            chat::deepseek::StreamCallbacks {
                on_token: |text, _is_first| {
                    let _ = app.emit("chat:token", serde_json::json!({
                        "session_id": sid,
                        "token": text,
                    }));
                },
                on_status: |status| {
                    let _ = app.emit("chat:status", serde_json::json!({
                        "session_id": sid,
                        "status": status,
                    }));
                },
                on_complete: |text| {
                    let _ = app.emit("chat:done", serde_json::json!({
                        "session_id": sid,
                        "text": text,
                    }));
                },
            },
            &cancel_flag,
        )
        .await;

        if let Err(e) = result {
            let _ = app.emit("chat:error", serde_json::json!({
                "session_id": sid,
                "error": map_err(e),
            }));
        }
    });

    Ok(StreamChatResponse { session_id })
}

#[tauri::command]
pub async fn cancel_chat(
    cancel_state: tauri::State<'_, CancelState>,
    session_id: String,
) -> Result<(), String> {
    if let Some(flag) = cancel_state.0.lock().map_err(|e| e.to_string())?.remove(&session_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn download_all(
    app_handle: tauri::AppHandle,
    workspace_path: String,
) -> Result<(), String> {
    let ws = workspace::Workspace::open(std::path::Path::new(&workspace_path))
        .map_err(map_err)?;
    let pending: Vec<_> = ws.pending_papers().into_iter().map(|p| {
        let folder = p.folder.clone();
        let arxiv_id = p.arxiv_id.clone();
        let title = p.title.clone();
        let has_body = p.has_body;
        let description_ready = p.description_ready;
        (folder, arxiv_id, title, has_body, description_ready)
    }).collect();

    let total = pending.len();
    if total == 0 {
        let _ = app_handle.emit("download:done", serde_json::json!({ "count": 0 }));
        return Ok(());
    }

    let downloads_dir = config::get_downloads_dir();
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let mut completed = 0usize;
        let cancel_flag = Arc::new(AtomicBool::new(false));

        for (folder, arxiv_id, title, has_body, description_ready) in &pending {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let _ = app.emit("download:progress", serde_json::json!({
                "current": completed,
                "total": total,
                "arxiv_id": arxiv_id,
                "status": "processing",
            }));

            let paper = workspace::Paper {
                arxiv_id: arxiv_id.clone(),
                title: title.clone(),
                folder_name: folder.file_name().unwrap().to_string_lossy().to_string(),
                folder: folder.clone(),
                has_body: *has_body,
                description_ready: *description_ready,
                is_complete: *has_body && *description_ready,
            };

            let result = workspace::process_pending_paper(
                &paper,
                &downloads_dir,
                std::path::Path::new(&workspace_path),
                &cancel_flag,
            )
            .await;

            match result {
                Ok(true) => {
                    completed += 1;
                    let _ = app.emit("download:progress", serde_json::json!({
                        "current": completed,
                        "total": total,
                        "arxiv_id": arxiv_id,
                        "status": "done",
                    }));
                }
                Ok(false) => {
                    let _ = app.emit("download:progress", serde_json::json!({
                        "current": completed,
                        "total": total,
                        "arxiv_id": arxiv_id,
                        "status": "skipped",
                    }));
                }
                Err(e) => {
                    let _ = app.emit("download:progress", serde_json::json!({
                        "current": completed,
                        "total": total,
                        "arxiv_id": arxiv_id,
                        "status": "error",
                        "error": map_err(e),
                    }));
                }
            }
        }

        let _ = app.emit("download:done", serde_json::json!({
            "count": completed,
            "total": total,
        }));
    });

    Ok(())
}

#[tauri::command]
pub async fn build_description(
    paper_dir: String,
    arxiv_id: String,
    title: String,
    context: Option<String>,
) -> Result<(), String> {
    chat::description::build_description(
        std::path::Path::new(&paper_dir),
        &arxiv_id,
        &title,
        None,
        context.as_deref(),
    )
    .await
    .map_err(map_err)
}

#[tauri::command]
pub async fn get_token_status() -> Result<serde_json::Value, String> {
    let token = config::load_cached_token();
    let masked = token.as_ref().map(|t| {
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
    config::save_token(&token).map_err(map_err)
}

#[tauri::command]
pub async fn validate_token() -> Result<bool, String> {
    let token = config::load_cached_token().ok_or_else(|| {
        map_err(ArxivError::Config("no token configured".into()))
    })?;
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.deepseek.com/models")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| ArxivError::Http(e))
        .map_err(map_err)?;
    Ok(response.status().is_success())
}

#[tauri::command]
pub async fn get_chat_sessions(session_dir: String) -> Result<Vec<serde_json::Value>, String> {
    let sessions = chat::session::list_sessions(std::path::Path::new(&session_dir))
        .map_err(map_err)?;
    Ok(sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "path": s.path.to_string_lossy(),
                "title": s.title,
                "kind": s.kind,
                "model": s.model,
                "reasoning_effort": s.reasoning_effort,
                "locked_fields": s.locked_fields,
                "messages": s.messages,
                "context_selection": s.context_selection,
                "context_snapshot": s.context_snapshot,
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
) -> Result<String, String> {
    let path = std::path::PathBuf::from(
        session_data["path"]
            .as_str()
            .unwrap_or("")
    );

    let title = session_data["title"].as_str().unwrap_or("Chat");
    let kind = session_data["kind"].as_str().unwrap_or("paper");
    let model = session_data["model"].as_str().unwrap_or("Flash");
    let reasoning_effort = session_data["reasoning_effort"].as_str().unwrap_or("low").to_string();

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

    let locked_fields: std::collections::HashMap<String, Vec<String>> = serde_json::from_value(
        session_data.get("locked_fields").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
    ).unwrap_or_default();

    let mut session = chat::session::ChatSession {
        path,
        title: title.to_string(),
        kind: kind.to_string(),
        model: model.to_string(),
        reasoning_effort,
        locked_fields,
        messages,
        context_selection,
        context_snapshot,
        view_name,
        updated_at: String::new(),
    };

    chat::session::save_session(&mut session, Some(std::path::Path::new(&session_dir)))
        .map_err(map_err)?;

    Ok(session.path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn rename_chat_session_data(path: String, new_title: String) -> Result<(), String> {
    chat::session::rename_session(std::path::Path::new(&path), &new_title)
        .map_err(map_err)
}

#[tauri::command]
pub async fn delete_chat_session_data(path: String) -> Result<bool, String> {
    chat::session::delete_session(std::path::Path::new(&path)).map_err(map_err)
}

#[tauri::command]
pub async fn generate_chat_title(messages: Vec<serde_json::Value>) -> Result<String, String> {
    chat::deepseek::generate_title(&messages).await.map_err(map_err)
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

#[tauri::command]
pub async fn read_pdf_base64(workspace_path: String, folder_name: String, arxiv_id: String) -> Result<String, String> {
    let folder = std::path::Path::new(&workspace_path).join(&folder_name);
    let pdf_path = folder.join(format!("{arxiv_id}.pdf"));
    let path = if pdf_path.exists() { pdf_path } else {
        let pattern = format!("{}/*.pdf", folder.display());
        match glob::glob(&pattern).ok().and_then(|mut e| e.next()) {
            Some(Ok(p)) => p,
            _ => return Err("PDF not found".into()),
        }
    };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes))
}

#[tauri::command]
pub async fn download_paper(
    raw_input: String,
    workspace_path: String,
) -> Result<PaperDto, String> {
    let arxiv_id = arxivcat_core::extract::arxiv::extract_arxiv_id(&raw_input)
        .ok_or_else(|| map_err(ArxivError::Other(format!("cannot parse arXiv ID from: {raw_input}"))))?;

    let downloads_dir = config::get_downloads_dir();

    let title = arxivcat_core::extract::arxiv::fetch_title_from_arxiv(&arxiv_id)
        .await
        .map_err(map_err)?
        .unwrap_or_else(|| "unknown".to_string());

    let folder_name = format!(
        "{}_{}",
        arxiv_id.replace('.', "_"),
        arxivcat_core::extract::arxiv::sanitize_filename(&title)
    );
    let out_dir = std::path::Path::new(&workspace_path).join(&folder_name);
    std::fs::create_dir_all(&out_dir).map_err(|e| map_err(ArxivError::from(e)))?;

    workspace::ensure_paper_meta_files(&out_dir).map_err(map_err)?;

    let (paper_dir_opt, _) =
        arxivcat_core::extract::source::download_source(&arxiv_id, &downloads_dir)
            .await
            .map_err(map_err)?;

    let paper_dir = paper_dir_opt.ok_or_else(|| {
        map_err(ArxivError::Extraction("source download returned None".into()))
    })?;

    arxivcat_core::extract::tex::extract_body_from_dir(&paper_dir, &out_dir)
        .map_err(map_err)?;

    let _ = arxivcat_core::extract::source::download_pdf(&arxiv_id, &out_dir).await;

    let _ = chat::description::build_description(&out_dir, &arxiv_id, &title, None, None).await;

    let has_body = out_dir.join("body.tex").exists();
    let desc_ready = out_dir.join("description.md").exists()
        && out_dir.join(".description_ready").exists();
    let is_complete = has_body && desc_ready;

    Ok(PaperDto {
        arxiv_id,
        title,
        folder_name,
        has_body,
        description_ready: desc_ready,
        is_complete,
    })
}
