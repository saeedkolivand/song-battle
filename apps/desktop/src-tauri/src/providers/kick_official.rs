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
pub fn expiry_from(expires_in: u64) -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (now + expires_in) as i64
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
}
