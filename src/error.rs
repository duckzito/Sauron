use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum SauronError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Screenshot capture failed: {0}")]
    Capture(String),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Email error: {0}")]
    Email(String),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, SauronError>;
