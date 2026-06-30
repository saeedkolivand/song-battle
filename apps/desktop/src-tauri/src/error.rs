//! The crate's single error type. `Result<_, String>` is banned elsewhere —
//! everything fallible returns `AppResult<T>`.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("db: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("url: {0}")]
    Url(#[from] url::ParseError),
    #[error("websocket: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("unsupported url: {0}")]
    UnsupportedUrl(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

// Tauri commands surface errors to JS, so the error must serialize. We send the
// rendered message (the source chain is preserved in logs, not over IPC).
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
