use std::path::Path;

use crate::config;
use crate::error::{ArxivError, Result};

const SYSTEM_PROMPT: &str = "You write structured markdown briefs for arXiv papers. The brief will later be used for semantic paper search inside a local workspace. Be detailed but compact, faithful to the provided paper text, and emphasize searchable technical concepts. Output markdown only. Use these sections exactly: # Overview, ## Problem, ## Method, ## Key Contributions, ## Technical Details, ## Search Tags, ## Good Match Queries.";

pub async fn build_description(
    cfg: &crate::net::HttpConfig,
    paper_dir: &Path,
    arxiv_id: &str,
    title: &str,
    log_cb: Option<&(dyn Fn(&str) + Sync)>,
    context_override: Option<&str>,
) -> Result<()> {
    let api_key = config::load_cached_token().ok_or_else(|| {
        ArxivError::Config("no DeepSeek API key configured".into())
    })?;

    let desc_path = paper_dir.join("description.md");
    let flag_path = paper_dir.join(".description_ready");

    let context = if let Some(override_text) = context_override {
        override_text.to_string()
    } else {
        let body_path = paper_dir.join("body.tex");
        let appendix_path = paper_dir.join("appendix.tex");
        let mut ctx = String::new();
        if body_path.exists() {
            let body = std::fs::read_to_string(&body_path)?;
            ctx.push_str(&body);
        }
        if appendix_path.exists() {
            let appendix = std::fs::read_to_string(&appendix_path)?;
            ctx.push_str("\n\n[Appendix]\n");
            ctx.push_str(&appendix);
        }
        ctx
    };

    if context.trim().is_empty() {
        return Err(ArxivError::Extraction("paper text is empty".into()));
    }

    let user_msg = format!(
        "arXiv ID: {arxiv_id}\nTitle: {title}\n\nPaper text snippet:\n{context}"
    );

    // Reuse the configured model preference (Flash/Pro) instead of a
    // hardcoded model (P0.14): user's chat_model choice applies to describe.
    let model = crate::config::load_model_preference();
    let model_id = crate::chat::deepseek::model_id(&model).unwrap_or("deepseek-v4-flash");
    let body = serde_json::json!({
        "model": model_id,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_msg}
        ],
        "max_tokens": 1400,
        "stream": false,
    });

    let response = cfg
        .client
        .post(cfg.deepseek_chat_url())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ArxivError::Chat(format!("description API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ArxivError::Chat(format!("description API error {status}: {text}")));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| {
        ArxivError::Chat(format!("failed to parse description response: {e}"))
    })?;

    let description = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if description.is_empty() {
        return Err(ArxivError::Chat("empty description response".into()));
    }

    let _ = std::fs::remove_file(&flag_path);
    std::fs::write(&desc_path, &description)?;
    std::fs::write(&flag_path, "ok\n")?;

    if let Some(cb) = log_cb {
        cb(&format!("description generated for {arxiv_id} ({})", desc_path.display()));
    }

    Ok(())
}
