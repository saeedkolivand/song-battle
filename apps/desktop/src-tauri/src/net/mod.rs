//! The ONLY home of `reqwest::Client`. Build clients here, never inline.

mod http;
pub use http::shared;
