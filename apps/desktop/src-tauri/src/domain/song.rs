use serde::{Deserialize, Serialize};

/// Where a song came from. Serializes lowercase to match the TS `Source` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Youtube,
    Spotify,
    Soundcloud,
}

/// A battle contestant. This is both the domain entity and the wire DTO
/// (`packages/types` `Song`), so optional fields are omitted when absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<u32>,
    pub source: Source,
    pub source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// What a `MediaProvider` returns for a URL (no id/submitter yet).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub thumbnail: Option<String>,
    pub duration_sec: Option<u32>,
    pub source: Source,
    pub source_url: String,
}

/// Best-effort source detection from a URL's host.
/// Detect the source from a URL. Requires an `https` scheme and a known host;
/// `javascript:`/`data:`/`http:`/unknown hosts return `None` so `import_song`
/// rejects them rather than storing an unsafe string.
pub fn detect_source(url: &str) -> Option<Source> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    match parsed.host_str()?.to_ascii_lowercase().as_str() {
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
        | "youtu.be" => Some(Source::Youtube),
        "open.spotify.com" => Some(Source::Spotify),
        "soundcloud.com" | "www.soundcloud.com" | "m.soundcloud.com" => Some(Source::Soundcloud),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_sources() {
        assert_eq!(detect_source("https://youtu.be/abc"), Some(Source::Youtube));
        assert_eq!(
            detect_source("https://www.youtube.com/watch?v=x"),
            Some(Source::Youtube)
        );
        assert_eq!(
            detect_source("https://open.spotify.com/track/x"),
            Some(Source::Spotify)
        );
        assert_eq!(
            detect_source("https://soundcloud.com/a/b"),
            Some(Source::Soundcloud)
        );
        assert_eq!(detect_source("https://example.com/x"), None);
    }

    #[test]
    fn rejects_unsafe_and_non_https_urls() {
        assert_eq!(detect_source("javascript:alert(1)"), None);
        assert_eq!(detect_source("data:text/html,<script>x</script>"), None);
        assert_eq!(detect_source("http://www.youtube.com/watch?v=x"), None); // not https
        assert_eq!(detect_source("not a url"), None);
        assert_eq!(detect_source("https://evil.com/youtube.com"), None); // host, not path
    }

    #[test]
    fn source_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Source::Youtube).unwrap(),
            "\"youtube\""
        );
    }
}
