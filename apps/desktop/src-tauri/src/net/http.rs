use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<Client> = OnceLock::new();

/// The single shared HTTP client (connection pooling, one TLS setup).
pub fn shared() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            // A browser-like UA: Kick's public channel API sits behind Cloudflare and
            // 403s obvious non-browser agents. (oEmbed providers accept any UA.)
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
            )
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client build")
    })
}
