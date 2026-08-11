//! Shared HTTP configuration with retry/backoff (P0.5/P0.6).
//!
//! Single reqwest::Client + custom User-Agent + exponential backoff with
//! Retry-After support. Base URLs are overridable via env for tests/wiremock:
//!   ARXIVCAT_ARXIV_BASE_URL    (default https://arxiv.org)
//!   ARXIVCAT_DEEPSEEK_BASE_URL (default https://api.deepseek.com)

use std::time::Duration;

use crate::error::{ArxivError, Result};

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub client: reqwest::Client,
    pub arxiv_base: String,
    pub deepseek_base: String,
    /// Max retries for transient failures (429 / 5xx / timeout / conn error).
    pub max_retries: u32,
    /// Base backoff in ms; actual wait = base * 2^attempt (no jitter, testable).
    pub backoff_base_ms: u64,
    /// Hard cap for Retry-After handling (ms).
    pub retry_after_cap_ms: u64,
}

impl HttpConfig {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(format!(
                "arxivcat/{} (+https://github.com/ArXivCat)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(ArxivError::Http)?;

        Ok(Self {
            client,
            arxiv_base: std::env::var("ARXIVCAT_ARXIV_BASE_URL")
                .unwrap_or_else(|_| "https://arxiv.org".to_string()),
            deepseek_base: std::env::var("ARXIVCAT_DEEPSEEK_BASE_URL")
                .unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
            max_retries: 3,
            backoff_base_ms: 500,
            retry_after_cap_ms: 30_000,
        })
    }

    pub fn arxiv_abs_url(&self, id: &str) -> String {
        format!("{}/abs/{}", self.arxiv_base, id)
    }

    pub fn arxiv_src_url(&self, id: &str) -> String {
        format!("{}/src/{}", self.arxiv_base, id)
    }

    pub fn arxiv_pdf_url(&self, id: &str) -> String {
        format!("{}/pdf/{}", self.arxiv_base, id)
    }

    pub fn deepseek_chat_url(&self) -> String {
        format!("{}/chat/completions", self.deepseek_base)
    }

    pub fn deepseek_models_url(&self) -> String {
        format!("{}/models", self.deepseek_base)
    }

    fn is_retryable_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    /// Wait for the current attempt's backoff. Respects Retry-After if present.
    async fn backoff_wait(&self, attempt: u32, retry_after_secs: Option<u64>) {
        let wait_ms = match retry_after_secs {
            Some(s) => (s * 1000).min(self.retry_after_cap_ms),
            None => self.backoff_base_ms.saturating_mul(1u64 << attempt.min(10)),
        };
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
    }

    /// GET a URL with retry/backoff. Returns the response (caller reads body).
    /// Non-retryable HTTP errors are returned as `ArxivError::Http` via the
    /// response's status line.
    pub async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        loop {
            match self.client.get(url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !Self::is_retryable_status(status) || attempt >= self.max_retries {
                        return Ok(resp);
                    }
                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok());
                    self.backoff_wait(attempt, retry_after).await;
                    attempt += 1;
                }
                Err(e) => {
                    let retryable =
                        e.is_timeout() || e.is_connect() || e.is_request() || e.is_body();
                    if !retryable || attempt >= self.max_retries {
                        return Err(ArxivError::Http(e));
                    }
                    self.backoff_wait(attempt, None).await;
                    attempt += 1;
                }
            }
        }
    }
}
