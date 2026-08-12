use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArxivError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Non-2xx status after retry exhaustion (429/5xx). Maps to exit 3 /
    /// kind http / retryable true per the frozen contract.
    #[error("HTTP status {0}")]
    HttpStatus(u16),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Extraction error: {0}")]
    Extraction(String),

    #[error("Chat error: {0}")]
    Chat(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl From<String> for ArxivError {
    fn from(s: String) -> Self {
        ArxivError::Other(s)
    }
}

impl From<&str> for ArxivError {
    fn from(s: &str) -> Self {
        ArxivError::Other(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ArxivError>;
