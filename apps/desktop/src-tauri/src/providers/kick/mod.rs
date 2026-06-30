//! Kick chat via Pusher. Reads `chatroom.id` from the public channel API, opens
//! the Pusher socket, subscribes, and normalizes `ChatMessageSentEvent` frames.
//! Every external field is treated as untrusted (no unwrap on their JSON).

use super::{now_ms, ChatMessage, ChatProvider, ChatUser, ProviderEvent};
use crate::domain::snapshot::ConnectionState;
use crate::error::{AppError, AppResult};
use crate::net;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

const PUSHER_APP_KEY: &str = "32cbd69e4b950bf97679";
const KICK_CHANNEL_API: &str = "https://kick.com/api/v2/channels";
const SUBSCRIBE_ACK: &str = "pusher_internal:subscription_succeeded";
const CHAT_EVENT: &str = "App\\Events\\ChatMessageSentEvent";
const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 30;

/// A Kick channel slug must match `^[A-Za-z0-9_-]+$` before being interpolated
/// into the channel API URL.
pub fn validate_channel(channel: &str) -> AppResult<()> {
    let ok = !channel.is_empty()
        && channel
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(AppError::Invalid(format!("invalid kick channel: {channel}")))
    }
}

/// Pure reconnect schedule. Grows exponentially after an error and linearly even
/// after a *clean* close, so an anti-bot clean-disconnect loop can't pin at ~1s.
/// Always bounded by `MAX_BACKOFF_SECS`.
fn next_backoff(prev: u64, clean_close: bool) -> u64 {
    let next = if clean_close { prev + 1 } else { prev * 2 };
    next.clamp(INITIAL_BACKOFF_SECS, MAX_BACKOFF_SECS)
}

pub struct KickProvider {
    channel: String,
}

impl KickProvider {
    pub fn new(channel: String) -> Self {
        Self {
            channel: channel.trim().to_string(),
        }
    }

    async fn fetch_chatroom_id(&self) -> AppResult<i64> {
        validate_channel(&self.channel)?; // defense-in-depth before URL interpolation
        let url = format!("{KICK_CHANNEL_API}/{}", self.channel);
        let resp = net::shared()
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            // Usually a Cloudflare challenge (often 403, HTML body) on the public
            // channel API — log a snippet so the cause is obvious in the dev console.
            let body: String = resp.text().await.unwrap_or_default().chars().take(160).collect();
            tracing::error!("kick: channel API {url} -> {status}; body: {body}");
            return Err(AppError::Other(format!("kick channel API returned {status}")));
        }
        let v: serde_json::Value = resp.json().await?;
        let id = v
            .get("chatroom")
            .and_then(|c| c.get("id"))
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| AppError::Other("kick: no chatroom id in channel response".into()))?;
        tracing::info!("kick: chatroom id {id} for channel '{}'", self.channel);
        Ok(id)
    }

    async fn connect_once(&self, tx: &Sender<ProviderEvent>) -> AppResult<()> {
        let chatroom_id = self.fetch_chatroom_id().await?;
        let pusher_url = format!(
            "wss://ws-us2.pusher.com/app/{PUSHER_APP_KEY}?protocol=7&client=js&version=8.4.0-rc2&flash=false"
        );
        let (ws, _) = tokio_tungstenite::connect_async(pusher_url).await?;
        let (mut write, mut read) = ws.split();

        let subscribe = format!(
            r#"{{"event":"pusher:subscribe","data":{{"channel":"chatroom.{chatroom_id}"}}}}"#
        );
        write.send(Message::Text(subscribe)).await?;
        // Stay `Connecting` until Pusher acks the subscribe — not just because we
        // sent the frame.

        while let Some(frame) = read.next().await {
            if let Message::Text(t) = frame? {
                match event_name(&t).as_deref() {
                    Some(SUBSCRIBE_ACK) => {
                        tracing::info!("kick: subscribed to chatroom.{chatroom_id} — connected");
                        emit(tx, ProviderEvent::Connection(ConnectionState::Connected));
                    }
                    Some(CHAT_EVENT) => {
                        if let Some(msg) = parse_frame(&t) {
                            emit(tx, ProviderEvent::Message(msg));
                        }
                    }
                    // `pusher:connection_established`, pings, presence: ignore. If Kick
                    // renamed the chat event, run with RUST_LOG=debug to see what arrives.
                    other => tracing::debug!("kick: ignored frame event {other:?}"),
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ChatProvider for KickProvider {
    async fn run(&self, tx: Sender<ProviderEvent>) -> AppResult<()> {
        let mut backoff = INITIAL_BACKOFF_SECS;
        loop {
            emit(&tx, ProviderEvent::Connection(ConnectionState::Connecting));
            let clean = match self.connect_once(&tx).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!("kick connection error: {e}");
                    emit(&tx, ProviderEvent::Connection(ConnectionState::Error));
                    false
                }
            };
            emit(&tx, ProviderEvent::Connection(ConnectionState::Reconnecting));
            let jitter = rand::random::<u64>() % 500;
            tokio::time::sleep(Duration::from_millis(backoff * 1000 + jitter)).await;
            backoff = next_backoff(backoff, clean);
        }
    }
}

/// Bounded send: drop on full so a flood can't grow memory or stall the provider.
fn emit(tx: &Sender<ProviderEvent>, ev: ProviderEvent) {
    let _ = tx.try_send(ev);
}

/// The Pusher `event` field of a frame, if any.
fn event_name(frame: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(frame)
        .ok()?
        .get("event")?
        .as_str()
        .map(String::from)
}

/// Parse one Pusher frame into a `ChatMessage`. The interesting frames carry the
/// chat payload as a JSON *string* in `data` (so it's parsed twice). Anything
/// else (presence, pings, malformed) → `None`. Pure & untrusting.
pub fn parse_frame(frame: &str) -> Option<ChatMessage> {
    let v: serde_json::Value = serde_json::from_str(frame).ok()?;
    if v.get("event")?.as_str()? != CHAT_EVENT {
        return None;
    }
    let data: serde_json::Value = serde_json::from_str(v.get("data")?.as_str()?).ok()?;
    let sender = data.get("sender")?;

    // ids may be a JSON number or string — normalize to a plain string either way.
    let id_str = |v: &serde_json::Value| {
        v.as_str()
            .map(String::from)
            .unwrap_or_else(|| v.to_string())
    };
    let user_id = sender.get("id").map(&id_str).unwrap_or_default();
    let username = sender
        .get("username")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let (mut is_mod, mut is_sub, mut is_vip) = (false, false, false);
    if let Some(badges) = sender
        .get("identity")
        .and_then(|i| i.get("badges"))
        .and_then(serde_json::Value::as_array)
    {
        for badge in badges {
            match badge.get("type").and_then(serde_json::Value::as_str) {
                Some("moderator" | "broadcaster") => is_mod = true,
                Some("subscriber" | "founder") => is_sub = true,
                Some("vip") => is_vip = true,
                _ => {}
            }
        }
    }

    Some(ChatMessage {
        id: data.get("id").map(&id_str).unwrap_or_default(),
        user: ChatUser {
            user_id,
            username: username.clone(),
            display_name: username,
            is_mod,
            is_sub,
            is_vip,
        },
        text: data
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        ts: now_ms(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A recorded ChatMessageSentEvent frame: note `data` is an escaped JSON string.
    const FRAME: &str = r#"{"event":"App\\Events\\ChatMessageSentEvent","data":"{\"id\":\"abc-123\",\"chatroom_id\":456,\"content\":\"!vote 1\",\"sender\":{\"id\":789,\"username\":\"voter_bob\",\"identity\":{\"badges\":[{\"type\":\"moderator\",\"text\":\"Mod\"},{\"type\":\"subscriber\",\"text\":\"Sub\"}]}}}","channel":"chatroom.456"}"#;

    #[test]
    fn parses_golden_frame() {
        let m = parse_frame(FRAME).expect("should parse");
        assert_eq!(m.id, "abc-123");
        assert_eq!(m.text, "!vote 1");
        assert_eq!(m.user.user_id, "789");
        assert_eq!(m.user.username, "voter_bob");
        assert_eq!(m.user.display_name, "voter_bob");
        assert!(m.user.is_mod);
        assert!(m.user.is_sub);
        assert!(!m.user.is_vip);
    }

    #[test]
    fn ignores_non_chat_and_malformed() {
        assert!(parse_frame(r#"{"event":"pusher:pong","data":"{}"}"#).is_none());
        assert!(parse_frame("not json").is_none());
        assert!(parse_frame(r#"{"event":"App\\Events\\ChatMessageSentEvent"}"#).is_none());
    }

    #[test]
    fn event_name_extracted() {
        assert_eq!(event_name(FRAME).as_deref(), Some(CHAT_EVENT));
        assert_eq!(
            event_name(r#"{"event":"pusher_internal:subscription_succeeded","data":"{}"}"#)
                .as_deref(),
            Some(SUBSCRIBE_ACK)
        );
        assert_eq!(event_name("garbage"), None);
    }

    #[test]
    fn backoff_grows_on_error_and_is_bounded() {
        // exponential on errors, never below initial, capped at MAX
        let mut b = INITIAL_BACKOFF_SECS;
        let mut seen = vec![b];
        for _ in 0..10 {
            b = next_backoff(b, false);
            seen.push(b);
        }
        assert!(seen.windows(2).all(|w| w[1] >= w[0]), "monotonic: {seen:?}");
        assert!(seen.windows(2).take(4).all(|w| w[1] > w[0]), "grows early");
        assert_eq!(*seen.last().unwrap(), MAX_BACKOFF_SECS);
        assert!(seen.iter().all(|&x| x <= MAX_BACKOFF_SECS));
    }

    #[test]
    fn backoff_still_grows_on_clean_close() {
        // a clean close must NOT reset to ~1s (anti-bot disconnect loop)
        assert!(next_backoff(1, true) > 1);
        assert!(next_backoff(5, true) > 5);
        assert_eq!(next_backoff(MAX_BACKOFF_SECS, true), MAX_BACKOFF_SECS);
    }

    #[test]
    fn channel_slug_validation() {
        for ok in ["xqc", "Train_wreckstv", "some-channel-9"] {
            assert!(validate_channel(ok).is_ok(), "{ok}");
        }
        for bad in ["", "has space", "drop;table", "a/b", "../x", "name?x=1"] {
            assert!(validate_channel(bad).is_err(), "{bad}");
        }
    }
}
