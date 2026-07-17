use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorLevel {
    Silent,
    Toast,
    Notice,
    Blocking,
}

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

impl ArxivError {
    pub fn level(&self) -> ErrorLevel {
        match self {
            ArxivError::Config(_) => ErrorLevel::Blocking,
            ArxivError::Http(_) | ArxivError::Chat(_) => ErrorLevel::Toast,
            ArxivError::NotFound(_) | ArxivError::Extraction(_) => ErrorLevel::Notice,
            ArxivError::Io(_) | ArxivError::Parse(_) | ArxivError::Json(_) | ArxivError::Other(_) => {
                ErrorLevel::Toast
            }
        }
    }
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
