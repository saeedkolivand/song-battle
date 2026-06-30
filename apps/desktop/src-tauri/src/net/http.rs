use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<Client> = OnceLock::new();

/// The single shared HTTP client (connection pooling, one TLS setup).
pub fn shared() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("SongBattle/0.1")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client build")
    })
}
