//! oEmbed-based `MediaProvider`s. One generic impl parameterized by `Source`,
//! since all three endpoints return the same `title`/`author_name`/`thumbnail_url`
//! shape (duration is not provided by oEmbed → left `None`, editable later).

use crate::domain::song::{MediaMetadata, Source};
use crate::error::AppResult;
use crate::net;
use crate::providers::MediaProvider;
use async_trait::async_trait;

pub struct OEmbed {
    pub source: Source,
}

#[async_trait]
impl MediaProvider for OEmbed {
    async fn fetch(&self, url: &str) -> AppResult<MediaMetadata> {
        let v: serde_json::Value = net::shared()
            .get(oembed_url(self.source, url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(MediaMetadata {
            title: v
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Untitled")
                .to_string(),
            artist: v
                .get("author_name")
                .and_then(serde_json::Value::as_str)
                .map(String::from),
            thumbnail: v
                .get("thumbnail_url")
                .and_then(serde_json::Value::as_str)
                .map(String::from),
            duration_sec: None,
            source: self.source,
            source_url: url.to_string(),
        })
    }
}

fn oembed_url(source: Source, url: &str) -> String {
    let enc: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
    match source {
        Source::Youtube => format!("https://www.youtube.com/oembed?url={enc}&format=json"),
        Source::Soundcloud => format!("https://soundcloud.com/oembed?url={enc}&format=json"),
        Source::Spotify => format!("https://open.spotify.com/oembed?url={enc}"),
    }
}

/// Fetch metadata for a URL via the matching provider.
pub async fn fetch(source: Source, url: &str) -> AppResult<MediaMetadata> {
    OEmbed { source }.fetch(url).await
}

/// Fallback used when oEmbed fails — the song is still added, just unenriched.
pub fn placeholder(source: Source, url: &str) -> MediaMetadata {
    MediaMetadata {
        title: url.to_string(),
        artist: None,
        thumbnail: None,
        duration_sec: None,
        source,
        source_url: url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_endpoints() {
        let u = oembed_url(Source::Youtube, "https://youtu.be/a b?c=1");
        assert!(u.starts_with("https://www.youtube.com/oembed?url="));
        assert!(u.contains("format=json"));
        assert!(!u.contains(' ')); // url-encoded
        assert!(oembed_url(Source::Spotify, "https://open.spotify.com/track/x")
            .starts_with("https://open.spotify.com/oembed?url="));
    }
}
