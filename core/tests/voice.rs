//! Voice notes made by each platform's web view (E-18, D-21). The fixtures are
//! 3 s of a synthetic 440 Hz tone, Opus-encoded by WebCodecs in the Android
//! WebView, WebKitGTK, WebView2 and WKWebView, muxed into Ogg by the page
//! (spike/voice-notes). The core must accept what every platform produces.

use mossbook_core::domain::inspect_ogg_opus;

const FIXTURES: [&str; 4] = [
    "android-webview",
    "linux-webkitgtk",
    "windows-webview2",
    "macos-wkwebview",
];

#[test]
fn recordings_from_every_platform_are_accepted() {
    for name in FIXTURES {
        let path = format!(
            "{}/tests/fixtures/voice/{name}.ogg",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(inspect_ogg_opus(&bytes), Ok(3000), "{name}");
    }
}
