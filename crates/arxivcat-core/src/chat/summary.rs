//! Brief + deep summary generation (two-round, prefix-cache friendly).
//!
//! Round 1 generates the brief (writes `brief_summary.md` + `.description_ready`).
//! Round 2 continues the SAME conversation (system + user1 + assistant(brief),
//! then user2(deep instruction)) so the second request contains round 1 as a
//! byte-identical prefix — DeepSeek prefix cache then prices the shared
//! ~60k-token paper context at the cached rate. Round 1 and Round 2 MUST
//! share the same `SUMMARY_SYSTEM` and the same `build_user1` output.

use std::path::Path;

use crate::config;
use crate::error::{ArxivError, Result};

/// Shared system prompt for BOTH rounds. Byte-identical across the two
/// requests is what makes the second round's prefix hit the cache — never
/// change this per-round.
const SUMMARY_SYSTEM: &str = "You are a precise academic reader. You read arXiv paper LaTeX source and write faithful, information-dense summaries. Output markdown only. Never invent numbers, formulas, or claims that are not present in the provided text. Keep every number exactly as written in the source.";

/// Round-1 instruction, appended inside user1 (so user1 is byte-stable).
const BRIEF_INSTRUCTION: &str = "Write a structured markdown brief for this paper. The brief will later be used for semantic paper search inside a local workspace. Be detailed but compact, faithful to the provided paper text, and emphasize searchable technical concepts. Output markdown only. Use these sections exactly: # Overview, ## Problem, ## Method, ## Key Contributions, ## Technical Details, ## Search Tags, ## Good Match Queries.";

/// Round-2 instruction (a NEW user message after the brief was produced).
const DEEP_INSTRUCTION: &str = "Now write a DEEP technical recap of the same paper, in Chinese, based on the paper text and the brief above. Structure: 1. 核心问题 (why existing methods are not enough, with concrete numbers); 2. 核心洞察 (the single key idea); 3. 方法细节 (architecture, training, datasets, hyperparameters, formula intuition); 4. 实验结果 (main numbers, ablations, baselines); 5. 局限性与未解决问题 (author-stated and reviewer-perspective columns). Be detailed enough that a PhD student in the field can discuss the paper without reading the original. Preserve numbers at their original precision. Do NOT include tables from the source — raw tables are appended separately. Output markdown only.";

/// Defensive cap near the model's 1M-token context window (~2M chars ≈
/// 500k English / ~1M Chinese tokens) so a document only gets trimmed when
/// it genuinely exceeds the window — normal papers and long technical
/// reports pass through UNTRIMMED (user requirement: never truncate the
/// content they feed in). If the cap ever triggers, keep BOTH ends.
const MAX_CTX_CHARS: usize = 2_000_000;

fn truncate(s: &str) -> String {
    let len = s.chars().count();
    if len <= MAX_CTX_CHARS {
        return s.to_string();
    }
    let keep = MAX_CTX_CHARS / 2;
    let head: String = s.chars().take(keep).collect();
    let tail: String = s.chars().skip(len - keep).take(keep).collect();
    format!(
        "{head}\n\n...[truncated by arxivcat: {} chars omitted from middle]\n\n{tail}",
        len - MAX_CTX_CHARS
    )
}

/// Byte-stable round-1 user message: paper metadata + context + brief
/// instruction. MUST be produced identically by `generate_brief` and
/// `generate_deep` — this is the shared cache prefix.
fn build_user1(paper_dir: &Path, arxiv_id: &str, title: &str) -> Result<String> {
    let body_path = paper_dir.join("body.tex");
    let appendix_path = paper_dir.join("appendix.tex");
    let mut ctx = String::new();
    if body_path.exists() {
        let body = std::fs::read_to_string(&body_path)?;
        ctx.push_str(&truncate(&body));
    }
    if appendix_path.exists() {
        let appendix = std::fs::read_to_string(&appendix_path)?;
        ctx.push_str("\n\n[Appendix]\n");
        ctx.push_str(&truncate(&appendix));
    }
    if ctx.trim().is_empty() {
        return Err(ArxivError::Extraction("paper text is empty".into()));
    }
    Ok(format!(
        "arXiv ID: {arxiv_id}\nTitle: {title}\n\nPaper text snippet:\n{ctx}\n\n{BRIEF_INSTRUCTION}"
    ))
}

fn api_key() -> Result<String> {
    config::load_cached_token()
        .ok_or_else(|| ArxivError::Config("no DeepSeek API key configured".into()))
}

fn model_id() -> String {
    let model = config::load_model_preference();
    crate::chat::deepseek::model_id(&model)
        .unwrap_or("deepseek-v4-flash")
        .to_string()
}

async fn chat_once(
    cfg: &crate::net::HttpConfig,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
) -> Result<String> {
    let key = api_key()?;
    let body = serde_json::json!({
        "model": model_id(),
        "messages": messages,
        "max_tokens": max_tokens,
        "stream": false,
    });
    let response = cfg
        .client
        .post(cfg.deepseek_chat_url())
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ArxivError::Chat(format!("summary API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ArxivError::Chat(format!(
            "summary API error {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ArxivError::Chat(format!("failed to parse summary response: {e}")))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if content.is_empty() {
        return Err(ArxivError::Chat("empty summary response".into()));
    }
    Ok(content)
}

/// Round 1: generate the brief, write `brief_summary.md` + `.description_ready`
/// (flag name kept for manifest-contract compatibility), return the brief text.
pub async fn generate_brief(
    cfg: &crate::net::HttpConfig,
    paper_dir: &Path,
    arxiv_id: &str,
    title: &str,
) -> Result<String> {
    let user1 = build_user1(paper_dir, arxiv_id, title)?;
    let text = chat_once(
        cfg,
        vec![
            serde_json::json!({"role": "system", "content": SUMMARY_SYSTEM}),
            serde_json::json!({"role": "user", "content": user1}),
        ],
        1400,
    )
    .await?;

    let brief_path = paper_dir.join("brief_summary.md");
    let flag_path = paper_dir.join(".description_ready");
    let _ = std::fs::remove_file(&flag_path);
    std::fs::write(&brief_path, &text)?;
    std::fs::write(&flag_path, "ok\n")?;
    Ok(text)
}

/// Round 2: generate the deep recap in the SAME conversation as the brief
/// (cache-friendly prefix), append raw LaTeX tables (no LLM transcription),
/// write `deep_summary.md` + `.deep_ready`.
pub async fn generate_deep(
    cfg: &crate::net::HttpConfig,
    paper_dir: &Path,
    arxiv_id: &str,
    title: &str,
) -> Result<()> {
    // Brief must exist as the assistant message of round 1. If missing,
    // produce it first (idempotent rebuild).
    let brief_path = paper_dir.join("brief_summary.md");
    let brief_text = if brief_path.exists() {
        let content = std::fs::read_to_string(&brief_path)?;
        if content.trim().is_empty() {
            // Empty stub — rebuild instead of using it as the round-1
            // assistant message (jury-burst R3).
            generate_brief(cfg, paper_dir, arxiv_id, title).await?
        } else {
            content
        }
    } else {
        generate_brief(cfg, paper_dir, arxiv_id, title).await?
    };

    // Byte-identical to round 1's user1 — the cache prefix.
    let user1 = build_user1(paper_dir, arxiv_id, title)?;

    let deep_text = chat_once(
        cfg,
        vec![
            serde_json::json!({"role": "system", "content": SUMMARY_SYSTEM}),
            serde_json::json!({"role": "user", "content": user1}),
            serde_json::json!({"role": "assistant", "content": brief_text}),
            serde_json::json!({"role": "user", "content": DEEP_INSTRUCTION}),
        ],
        16000,
    )
    .await?;

    let mut content = deep_text;

    // Append raw source tables (deterministic copy — tables never pass
    // through the LLM, so numbers cannot be mistranscribed).
    let tables = crate::extract::tex::extract_tabular(paper_dir);
    if !tables.is_empty() {
        content.push_str("\n\n---\n\n## 附录：原始数据表格\n\n");
        for t in &tables {
            content.push_str("```latex\n");
            content.push_str(t);
            content.push_str("\n```\n\n");
        }
    }

    let deep_path = paper_dir.join("deep_summary.md");
    std::fs::write(&deep_path, &content)?;
    std::fs::write(paper_dir.join(".deep_ready"), "ok\n")?;
    Ok(())
}
