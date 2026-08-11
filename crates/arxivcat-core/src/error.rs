use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArxivError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

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
