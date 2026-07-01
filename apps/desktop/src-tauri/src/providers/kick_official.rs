//! Official Kick API OAuth 2.1 + PKCE (K1). Pure PKCE/URL helpers plus the
//! token-endpoint calls; `server::oauth_callback` drives the round trip and
//! persists the result. `client_secret` is required even with PKCE — Kick's
//! OAuth app is a confidential client, not a public one.
//!
//! The webhook subscription (`events:subscribe`) and the actual API calls
//! that need a valid access token are K2 — this module stops at "tokens
//! exchanged/refreshed".

use crate::error::{AppError, AppResult};
use crate::net;
use crate::server;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const AUTHORIZE_URL: &str = "https://id.kick.com/oauth/authorize";
const TOKEN_URL: &str = "https://id.kick.com/oauth/token";

/// `(verifier, challenge)` — an RFC 7636 PKCE pair. `verifier` is
/// base64url-no-pad of 32 random bytes; `challenge` is
/// base64url-no-pad(sha256(verifier)).
pub fn pkce() -> (String, String) {
    let bytes: [u8; 32] = rand::random();
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = challenge_of(&verifier);
    (verifier, challenge)
}

fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// A random opaque CSRF token for the OAuth `state` query param (distinct
/// from the PKCE verifier/challenge pair).
pub fn random_state() -> String {
    let bytes: [u8; 16] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// `http://localhost:{PORT}/oauth/callback` — the registered loopback
/// redirect target (reuses the overlay server's port/const).
pub fn redirect_uri() -> String {
    format!("http://localhost:{}/oauth/callback", server::PORT)
}

/// Build the `https://id.kick.com/oauth/authorize` URL for the login popup.
pub fn authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    challenge: &str,
) -> String {
    let mut url = url::Url::parse(AUTHORIZE_URL).expect("static authorize URL");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scope)
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    url.to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    /// Kick doesn't always rotate the refresh token on `grant_type=refresh_token`
    /// — `None` means "keep whatever we already have" (see `db::set_kick_tokens`).
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> AppResult<Tokens> {
    post_token(&[
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ])
    .await
}

/// K2 wires this into an ensure-valid-token helper before each API call.
#[allow(dead_code)]
pub async fn refresh(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> AppResult<Tokens> {
    post_token(&[
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
    ])
    .await
}

async fn post_token(params: &[(&str, &str)]) -> AppResult<Tokens> {
    let resp = net::shared().post(token_url()).form(params).send().await?;
    let status = resp.status();
    if !status.is_success() {
        // Keep the body — Kick's error JSON (e.g. `invalid_grant`) says *why*,
        // which `error_for_status()` would have discarded.
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!(
            "kick token endpoint returned {status}: {body}"
        )));
    }
    Ok(resp.json::<Tokens>().await?)
}

/// The token endpoint, overridable in tests so the exchange can be driven
/// against a local mock server instead of hitting `id.kick.com`.
fn token_url() -> String {
    #[cfg(test)]
    if let Some(u) = TOKEN_URL_OVERRIDE.get() {
        return u.clone();
    }
    TOKEN_URL.to_string()
}

#[cfg(test)]
static TOKEN_URL_OVERRIDE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Point `post_token` at a local mock server (test-only, set once).
#[cfg(test)]
pub(crate) fn set_token_url_for_test(url: String) {
    let _ = TOKEN_URL_OVERRIDE.set(url);
}

/// Absolute unix-seconds expiry from a token response's relative `expires_in`.
/// `expires_in` is attacker-influenced (comes off the token response), so the
/// arithmetic saturates instead of wrapping into a negative (already-expired) i64.
pub fn expiry_from(expires_in: u64) -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    now.saturating_add(expires_in).min(i64::MAX as u64) as i64
}

// ── webhooks + event subscription (K2) ──────────────────────────────────────

const PUBLIC_KEY_URL: &str = "https://api.kick.com/public/v1/public-key";
const SUBSCRIPTIONS_URL: &str = "https://api.kick.com/public/v1/events/subscriptions";

/// Kick's webhook-signing public key is fixed, so fetch it once and cache it.
static PUBLIC_KEY: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// The cached PEM (`-----BEGIN PUBLIC KEY-----`), fetched on first use.
pub async fn public_key() -> AppResult<&'static str> {
    let pem = PUBLIC_KEY.get_or_try_init(fetch_public_key).await?;
    Ok(pem.as_str())
}

/// Seed the public-key cache with a test key so the webhook handler can be
/// exercised without reaching api.kick.com (set once).
#[cfg(test)]
pub(crate) fn set_public_key_for_test(pem: String) {
    let _ = PUBLIC_KEY.set(pem);
}

async fn fetch_public_key() -> AppResult<String> {
    #[derive(Deserialize)]
    struct Resp {
        data: KeyData,
    }
    #[derive(Deserialize)]
    struct KeyData {
        public_key: String,
    }
    let resp: Resp = net::shared()
        .get(PUBLIC_KEY_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.data.public_key)
}

/// Verify a Kick webhook's RSA-SHA256 (PKCS#1 v1.5) signature. The signed
/// payload is `{message_id}.{timestamp}.{raw_body}`; `signature_b64` is the
/// standard-base64 `Kick-Event-Signature` header. Any malformed input → `false`
/// (fail closed) — a bad signature must never be treated as authentic.
pub fn verify_webhook(
    public_key_pem: &str,
    msg_id: &str,
    timestamp: &str,
    body: &[u8],
    signature_b64: &str,
) -> bool {
    use base64::engine::general_purpose::STANDARD;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::DecodePublicKey;
    use rsa::signature::Verifier;
    use rsa::RsaPublicKey;

    let Ok(pubkey) = RsaPublicKey::from_public_key_pem(public_key_pem) else {
        return false;
    };
    let Ok(sig_bytes) = STANDARD.decode(signature_b64) else {
        return false;
    };
    let Ok(sig) = Signature::try_from(sig_bytes.as_slice()) else {
        return false;
    };
    let mut message = Vec::with_capacity(msg_id.len() + timestamp.len() + body.len() + 2);
    message.extend_from_slice(msg_id.as_bytes());
    message.push(b'.');
    message.extend_from_slice(timestamp.as_bytes());
    message.push(b'.');
    message.extend_from_slice(body);
    VerifyingKey::<Sha256>::new(pubkey)
        .verify(&message, &sig)
        .is_ok()
}

/// Max age of a webhook's signed timestamp before we ignore it. The signature
/// proves authenticity but not freshness; this bounds the replay window in TIME
/// (the id dedupe only bounds it by cache size). Both directions, to tolerate
/// small clock skew.
const WEBHOOK_MAX_AGE_SECS: i64 = 300;

/// True if `timestamp` (the signed RFC3339 `Kick-Event-Message-Timestamp`) is
/// within ±5 min of now. Malformed → `false`. Only meaningful AFTER the
/// signature verifies (the timestamp is attacker-controlled until then).
pub fn timestamp_is_fresh(timestamp: &str) -> bool {
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};
    let Ok(ts) = OffsetDateTime::parse(timestamp, &Rfc3339) else {
        return false;
    };
    let now = (crate::providers::now_ms() / 1000) as i64;
    (now - ts.unix_timestamp()).abs() <= WEBHOOK_MAX_AGE_SECS
}

/// Parse a `chat.message.sent` webhook body into our `ChatMessage`. `None` if it
/// isn't a chat message (missing sender/content) — the handler then ignores it.
pub fn parse_chat_event(v: &serde_json::Value) -> Option<crate::providers::ChatMessage> {
    use crate::providers::{now_ms, ChatMessage, ChatUser};

    let sender = v.get("sender")?;
    // user_id is a JSON number; accept a string too, defensively.
    let user_id = sender.get("user_id").and_then(|u| {
        u.as_i64()
            .map(|n| n.to_string())
            .or_else(|| u.as_str().map(str::to_owned))
    })?;
    let username = sender.get("username")?.as_str()?.to_owned();
    let text = v.get("content")?.as_str()?.to_owned();
    let id = v
        .get("message_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();

    // Roles come from the sender's identity badges (identity may be null).
    let badges = sender
        .get("identity")
        .and_then(|i| i.get("badges"))
        .and_then(serde_json::Value::as_array);
    let has = |t: &str| {
        badges.is_some_and(|arr| {
            arr.iter()
                .any(|b| b.get("type").and_then(serde_json::Value::as_str) == Some(t))
        })
    };

    Some(ChatMessage {
        id,
        user: ChatUser {
            user_id,
            username: username.clone(),
            display_name: username,
            is_mod: has("moderator") || has("broadcaster"),
            is_sub: has("subscriber") || has("founder"), // parity with the Pusher parser
            is_vip: has("vip"),
        },
        text,
        ts: now_ms(),
    })
}

/// The events-subscriptions endpoint, overridable in tests to point at a local
/// mock server instead of api.kick.com.
fn subscriptions_url() -> String {
    #[cfg(test)]
    if let Some(u) = SUBSCRIPTIONS_URL_OVERRIDE.get() {
        return u.clone();
    }
    SUBSCRIPTIONS_URL.to_string()
}

#[cfg(test)]
static SUBSCRIPTIONS_URL_OVERRIDE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn set_subscriptions_url_for_test(url: String) {
    let _ = SUBSCRIPTIONS_URL_OVERRIDE.set(url);
}

/// Subscribe the authorized broadcaster to `chat.message.sent` webhooks. With a
/// USER access token Kick infers the broadcaster (so no `broadcaster_user_id`),
/// and the callback URL is the app-global one set at dev.kick.com. Returns the
/// new subscription id (Kick's `data[0].subscription_id`).
pub async fn subscribe_chat(access_token: &str) -> AppResult<String> {
    let body = serde_json::json!({
        "events": [{ "name": "chat.message.sent", "version": 1 }],
        "method": "webhook",
    });
    let v: serde_json::Value = net::shared()
        .post(subscriptions_url())
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    v.get("data")
        .and_then(|d| d.get(0))
        .and_then(|e| e.get("subscription_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::Other("subscribe: no subscription_id in response".into()))
}

/// Best-effort remote unsubscribe (local logout clears tokens regardless).
pub async fn unsubscribe(access_token: &str, subscription_id: &str) -> AppResult<()> {
    net::shared()
        .delete(subscriptions_url())
        .query(&[("id", subscription_id)])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7636 §A known vector.
    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            challenge_of(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn pkce_generates_a_fresh_pair_each_call() {
        let (v1, c1) = pkce();
        let (v2, _c2) = pkce();
        assert_eq!(v1.len(), 43); // 32 bytes, base64url no-pad
        assert_eq!(c1.len(), 43); // sha256 digest is also 32 bytes
        assert_ne!(v1, v2, "verifier must be random per call");
        assert_eq!(
            challenge_of(&v1),
            c1,
            "authorize_url must send the challenge matching this verifier"
        );
    }

    #[test]
    fn authorize_url_contains_all_required_params() {
        let url = authorize_url(
            "cid",
            "http://localhost:31337/oauth/callback",
            "user:read channel:read",
            "st4te",
            "chall",
        );
        assert!(url.starts_with("https://id.kick.com/oauth/authorize?"));
        for pair in [
            "response_type=code",
            "client_id=cid",
            "state=st4te",
            "code_challenge=chall",
            "code_challenge_method=S256",
        ] {
            assert!(url.contains(pair), "missing `{pair}` in {url}");
        }
        // redirect_uri / scope are form-encoded on the wire — assert via a
        // decoded round trip instead of a raw substring match.
        let parsed = url::Url::parse(&url).unwrap();
        let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(
            pairs.get("redirect_uri").map(String::as_str),
            Some("http://localhost:31337/oauth/callback")
        );
        assert_eq!(
            pairs.get("scope").map(String::as_str),
            Some("user:read channel:read")
        );
    }

    #[test]
    fn redirect_uri_uses_the_server_port() {
        assert_eq!(
            redirect_uri(),
            format!("http://localhost:{}/oauth/callback", server::PORT)
        );
    }

    #[test]
    fn random_state_is_nonempty_and_random() {
        let a = random_state();
        let b = random_state();
        assert!(!a.is_empty());
        assert_ne!(a, b);
    }

    #[test]
    fn tokens_parse_from_a_sample_response() {
        let json = r#"{"access_token":"AT123","refresh_token":"RT456","expires_in":3600,"token_type":"Bearer","scope":"user:read"}"#;
        let t: Tokens = serde_json::from_str(json).unwrap();
        assert_eq!(t.access_token, "AT123");
        assert_eq!(t.refresh_token.as_deref(), Some("RT456"));
        assert_eq!(t.expires_in, 3600);
    }

    #[test]
    fn tokens_parse_without_a_rotated_refresh_token() {
        let json = r#"{"access_token":"AT789","expires_in":1800}"#;
        let t: Tokens = serde_json::from_str(json).unwrap();
        assert_eq!(t.access_token, "AT789");
        assert!(t.refresh_token.is_none());
    }

    #[test]
    fn expiry_from_adds_the_relative_seconds() {
        let a = expiry_from(0);
        let b = expiry_from(120);
        assert_eq!(b - a, 120);
    }

    #[test]
    fn expiry_from_saturates_instead_of_wrapping_negative() {
        // A hostile/huge `expires_in` must clamp to i64::MAX, never wrap into a
        // negative (already-expired) timestamp.
        assert_eq!(expiry_from(u64::MAX), i64::MAX);
    }

    #[test]
    fn verify_webhook_accepts_valid_signature_and_rejects_tampering() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine as _;
        use rsa::pkcs1v15::SigningKey;
        use rsa::pkcs8::{EncodePublicKey, LineEnding};
        use rsa::signature::{SignatureEncoding, Signer};

        // A throwaway keypair stands in for Kick's signing key.
        let mut rng = rand::thread_rng();
        let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_pem = rsa::RsaPublicKey::from(&priv_key)
            .to_public_key_pem(LineEnding::LF)
            .unwrap();

        let (id, ts, body) = ("01H0MSG", "2026-07-01T00:00:00Z", br#"{"content":"1"}"#);
        let mut signed = Vec::new();
        signed.extend_from_slice(id.as_bytes());
        signed.push(b'.');
        signed.extend_from_slice(ts.as_bytes());
        signed.push(b'.');
        signed.extend_from_slice(body);
        let sig_b64 = STANDARD.encode(SigningKey::<Sha256>::new(priv_key).sign(&signed).to_bytes());

        assert!(
            verify_webhook(&pub_pem, id, ts, body, &sig_b64),
            "valid signature"
        );
        // Any change to the signed inputs must fail closed:
        assert!(
            !verify_webhook(&pub_pem, id, ts, br#"{"content":"2"}"#, &sig_b64),
            "tampered body"
        );
        assert!(
            !verify_webhook(&pub_pem, "01H0OTHER", ts, body, &sig_b64),
            "wrong message id"
        );
        assert!(
            !verify_webhook(&pub_pem, id, ts, body, "not-base64!!"),
            "garbage signature"
        );
    }

    #[test]
    fn parse_chat_event_extracts_text_and_roles() {
        let v = serde_json::json!({
            "message_id": "01H0ABC",
            "content": "!vote 1",
            "sender": {
                "user_id": 987654321i64,
                "username": "voter_bob",
                "is_anonymous": false,
                "identity": {
                    "badges": [
                        { "text": "Moderator", "type": "moderator" },
                        { "text": "Subscriber", "type": "subscriber", "count": 3 }
                    ]
                }
            }
        });
        let m = parse_chat_event(&v).expect("a chat message");
        assert_eq!(m.id, "01H0ABC");
        assert_eq!(m.text, "!vote 1");
        assert_eq!(m.user.user_id, "987654321"); // number → string
        assert_eq!(m.user.username, "voter_bob");
        assert!(m.user.is_mod);
        assert!(m.user.is_sub);
        assert!(!m.user.is_vip);
    }

    #[test]
    fn parse_chat_event_handles_null_identity_and_ignores_non_chat() {
        // Anonymous sender: identity null → no roles, still a valid message.
        let anon = serde_json::json!({
            "message_id": "x", "content": "hi",
            "sender": { "user_id": 5, "username": "anon", "identity": null }
        });
        let m = parse_chat_event(&anon).expect("still a chat message");
        assert!(!m.user.is_mod && !m.user.is_sub && !m.user.is_vip);
        // Missing content/sender → not a chat message.
        assert!(parse_chat_event(&serde_json::json!({ "foo": "bar" })).is_none());
    }

    #[test]
    fn timestamp_freshness_bounds_the_replay_window() {
        use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
        let now = OffsetDateTime::now_utc();
        let fresh = now.format(&Rfc3339).unwrap();
        let stale = (now - Duration::minutes(10)).format(&Rfc3339).unwrap();
        let future = (now + Duration::minutes(10)).format(&Rfc3339).unwrap();
        assert!(timestamp_is_fresh(&fresh), "now is fresh");
        assert!(!timestamp_is_fresh(&stale), "10 min old is stale");
        assert!(!timestamp_is_fresh(&future), "10 min ahead is rejected");
        assert!(!timestamp_is_fresh("not-a-timestamp"), "garbage → false");
    }

    #[tokio::test]
    async fn subscribe_chat_parses_the_subscription_id() {
        use axum::{routing::post, Json, Router};
        // Mock the events-subscriptions endpoint with Kick's documented shape.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = Router::new().route(
                "/subs",
                post(|| async {
                    Json(serde_json::json!({
                        "message": "OK",
                        "data": [{
                            "name": "chat.message.sent",
                            "version": 1,
                            "subscription_id": "SUB-123",
                            "error": ""
                        }]
                    }))
                }),
            );
            let _ = axum::serve(listener, app).await;
        });
        set_subscriptions_url_for_test(format!("http://{addr}/subs"));

        assert_eq!(subscribe_chat("tok").await.unwrap(), "SUB-123");
    }
}
