use crate::config;
use crate::error::{ArxivError, Result};

pub const CHAT_MODELS: &[(&str, &str)] =
    &[("Flash", "deepseek-v4-flash"), ("Pro", "deepseek-v4-pro")];

pub fn model_id(name: &str) -> Option<&'static str> {
    CHAT_MODELS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, id)| *id)
}

#[derive(Debug, Clone)]
pub struct ChatMetrics {
    pub ttft_ms: f64,
    pub tokens_per_sec: f64,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub struct StreamCallbacks<F1, F2, F3>
where
    F1: Fn(&str, bool),
    F2: Fn(&str),
    F3: Fn(&str),
{
    pub on_token: F1,
    pub on_status: F2,
    pub on_complete: F3,
}

pub async fn stream_chat<F1, F2, F3>(
    cfg: &crate::net::HttpConfig,
    messages: &[serde_json::Value],
    model: &str,
    reasoning_effort: &str,
    callbacks: StreamCallbacks<F1, F2, F3>,
    cancel_flag: &std::sync::atomic::AtomicBool,
) -> Result<()>
where
    F1: Fn(&str, bool),
    F2: Fn(&str),
    F3: Fn(&str),
{
    let api_key = config::load_cached_token()
        .ok_or_else(|| ArxivError::Config("no DeepSeek API key configured".into()))?;

    let model_id = model_id(model).unwrap_or("deepseek-v4-flash");

    let mut body = serde_json::json!({
        "model": model_id,
        "messages": messages,
        "stream": true,
    });

    if reasoning_effort != "off" {
        body["thinking"] = serde_json::json!({"type": "enabled"});
        body["reasoning_effort"] = serde_json::Value::String(reasoning_effort.to_string());
    }

    let response = cfg
        .client
        .post(cfg.deepseek_chat_url())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ArxivError::Chat(format!("API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ArxivError::Chat(format!("API error {status}: {text}")));
    }

    let start = std::time::Instant::now();
    let mut buffer = String::new();
    let mut ttft: Option<std::time::Duration> = None;
    let mut token_count = 0u32;
    let mut first_chunk = true;

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    let newline_collapse_re = regex::Regex::new(r"\n{2,}").unwrap();

    while let Some(chunk_result) = stream.next().await {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            (callbacks.on_status)("cancelled");
            return Ok(());
        }

        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                return Err(ArxivError::Chat(format!("stream error: {e}")));
            }
        };

        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed == "data: [DONE]" {
                continue;
            }

            let data = trimmed.strip_prefix("data: ").unwrap_or(trimmed);
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                    if !content.is_empty() && ttft.is_none() {
                        ttft = Some(start.elapsed());
                    }

                    let collapsed = newline_collapse_re.replace_all(content, "\n").to_string();

                    buffer.push_str(&collapsed);
                    (callbacks.on_token)(&collapsed, first_chunk);
                    first_chunk = false;
                    token_count += 1;
                }
            }
        }
    }

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        (callbacks.on_status)("cancelled");
        return Ok(());
    }

    if !buffer.is_empty() {
        let elapsed = start.elapsed();
        let tps = if elapsed.as_secs_f64() > 0.0 {
            token_count as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let mut metrics = format!("{} | {:.0} tok/s | {} tokens", model_id, tps, buffer.len());

        if let Some(t) = ttft {
            metrics = format!(
                "{} | TTFT {:.0}ms | {:.0} tok/s | {} tokens",
                model_id,
                t.as_secs_f64() * 1000.0,
                tps,
                buffer.len()
            );
        }

        (callbacks.on_status)(&metrics);
        (callbacks.on_complete)(buffer.trim());
    }

    Ok(())
}

pub async fn generate_title(
    cfg: &crate::net::HttpConfig,
    messages: &[serde_json::Value],
) -> Result<String> {
    let api_key = config::load_cached_token()
        .ok_or_else(|| ArxivError::Config("no DeepSeek API key configured".into()))?;

    let mut body = serde_json::json!({
        "model": "deepseek-v4-flash",
        "messages": messages,
        "max_tokens": 20,
        "stream": false,
        "thinking": {"type": "disabled"},
    });

    body["messages"].as_array_mut().unwrap().push(serde_json::json!({
        "role": "user",
        "content": "Generate a short title for this conversation in the same language as the conversation. Output only the title."
    }));

    let response = cfg
        .client
        .post(cfg.deepseek_chat_url())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ArxivError::Chat(format!("title API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ArxivError::Chat(format!(
            "title API error {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ArxivError::Chat(format!("failed to parse title response: {e}")))?;

    let title = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if title.is_empty() {
        return Err(ArxivError::Chat("empty title response".into()));
    }

    Ok(title)
}
