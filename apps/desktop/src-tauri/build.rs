fn main() {
    // The test binaries don't receive the application manifest that
    // `tauri_build` embeds in the main exe. Without it, on Windows they bind to
    // comctl32 v5.82, which lacks TaskDialogIndirect / SetWindowSubclass (pulled
    // in by muda/wry) → STATUS_ENTRYPOINT_NOT_FOUND at load before any test runs.
    // Declare the Common-Controls v6 dependency for the test/bench/example bins.
    // Applies to the lib unit-test binary (a "test" link artifact). The main
    // exe gets its Common-Controls manifest from `tauri_build`; this matches it
    // for the test binary. The linker merges the identical dependency for the
    // main bin without conflict.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::build()
}
