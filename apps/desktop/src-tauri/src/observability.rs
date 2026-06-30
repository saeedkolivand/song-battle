//! Tracing setup. Moved out of `lib.rs` so the entrypoint just calls `init()`.

/// Install the global tracing subscriber. Honors `RUST_LOG`, defaults to `info`.
// ponytail: `EnvFilter` reads `RUST_LOG` directly here (crate-internal, not our
// `std::env`). Threading it through `platform/` is deferred — do it if config
// ownership ever needs to centralize the filter.
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
}
