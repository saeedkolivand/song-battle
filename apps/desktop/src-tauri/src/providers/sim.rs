//! No-network chat provider: emits N synthetic vote messages, alternating
//! `"1"`/`"2"`. For tests and offline dev.

use super::{ChatMessage, ChatProvider, ChatUser, ProviderEvent};
use crate::domain::snapshot::ConnectionState;
use crate::error::AppResult;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

// ponytail: the dev/test harness — exercised by tests, not yet wired into a
// command. Allow until a "sim mode" toggle lands.
#[allow(dead_code)]
pub struct SimChatProvider {
    pub count: usize,
}

#[async_trait]
impl ChatProvider for SimChatProvider {
    async fn run(&self, tx: Sender<ProviderEvent>) -> AppResult<()> {
        let _ = tx.send(ProviderEvent::Connection(ConnectionState::Connected)).await;
        for i in 0..self.count {
            let text = if i % 2 == 0 { "1" } else { "2" };
            let _ = tx
                .send(ProviderEvent::Message(ChatMessage {
                    id: i.to_string(),
                    user: ChatUser {
                        user_id: format!("u{i}"),
                        username: format!("user{i}"),
                        display_name: format!("user{i}"),
                        is_mod: false,
                        is_sub: false,
                        is_vip: false,
                    },
                    text: text.into(),
                    ts: 0,
                }))
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emits_connected_then_n_messages() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        SimChatProvider { count: 4 }.run(tx).await.unwrap();
        let mut msgs = 0;
        let mut connected = false;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                ProviderEvent::Connection(ConnectionState::Connected) => connected = true,
                ProviderEvent::Message(_) => msgs += 1,
                _ => {}
            }
        }
        assert!(connected);
        assert_eq!(msgs, 4);
    }
}
