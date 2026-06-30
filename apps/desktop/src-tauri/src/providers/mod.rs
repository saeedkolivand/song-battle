//! Chat + media provider traits and implementations. A `ChatProvider` runs a
//! connection loop and emits `ProviderEvent`s; the command layer applies them to
//! `AppState` (so providers stay decoupled from app state).

pub mod kick;
pub mod sim;

use crate::domain::snapshot::ConnectionState;
use crate::domain::song::MediaMetadata;
use crate::error::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUser {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub is_mod: bool,
    pub is_sub: bool,
    pub is_vip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub user: ChatUser,
    pub text: String,
    pub ts: u64,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Connection(ConnectionState),
    Message(ChatMessage),
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    /// Run until the spawning task is aborted, emitting events on `tx`.
    /// `tx` is bounded — providers `try_send` and drop on full so a chat flood
    /// can't grow memory.
    async fn run(&self, tx: Sender<ProviderEvent>) -> AppResult<()>;
}

#[async_trait]
pub trait MediaProvider: Send + Sync {
    async fn fetch(&self, url: &str) -> AppResult<MediaMetadata>;
}

/// Unix time in ms (used for vote ordering / message timestamps).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
