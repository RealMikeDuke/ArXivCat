use crate::config;
use crate::error::{ArxivError, Result};

pub const CHAT_MODELS: &[(&str, &str)] = &[
    ("Flash", "deepseek-v4-flash"),
    ("Pro", "deepseek-v4-pro"),
];

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
    messages: &[serde_json::Value],
    model: &str,
    deep_thinking: bool,
    callbacks: StreamCallbacks<F1, F2, F3>,
    cancel_flag: &std::sync::atomic::AtomicBool,
) -> Result<()>
where
    F1: Fn(&str, bool),
    F2: Fn(&str),
    F3: Fn(&str),
{
    let api_key = config::load_cached_token().ok_or_else(|| {
        ArxivError::Config("no DeepSeek API key configured".into())
    })?;

    let model_id = model_id(model).unwrap_or("deepseek-v4-flash");

    let mut body = serde_json::json!({
        "model": model_id,
        "messages": messages,
        "stream": true,
    });

    if deep_thinking {
        body["reasoning_effort"] = serde_json::Value::String("high".to_string());
        body["extra_body"] = serde_json::json!({
            "thinking": {"type": "enabled"}
        });
    }

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.deepseek.com/chat/completions")
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
    let mut ttft_recorded = false;
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
                    if !content.is_empty() && !ttft_recorded {
                        ttft_recorded = true;
                    }

                    let collapsed = newline_collapse_re
                        .replace_all(content, "\n")
                        .to_string();

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
        let _ttft_ms = if ttft_recorded {
            elapsed.as_secs_f64() * 1000.0 / token_count.max(1) as f64
        } else {
            0.0
        };
        let tps = if elapsed.as_secs_f64() > 0.0 {
            token_count as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let metrics = format!(
            "{} | {:.0} tok/s | {} tokens",
            model_id,
            tps,
            buffer.len()
        );

        (callbacks.on_status)(&metrics);
        (callbacks.on_complete)(buffer.trim());
    }

    Ok(())
}
