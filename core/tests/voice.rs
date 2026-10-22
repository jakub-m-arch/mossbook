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

#[test]
fn recordings_from_every_platform_decode_for_transcription() {
    use mossbook_core::transcribe::audio::{SAMPLE_RATE, decode};
    for name in FIXTURES {
        let path = format!(
            "{}/tests/fixtures/voice/{name}.ogg",
            env!("CARGO_MANIFEST_DIR")
        );
        let pcm = decode(&std::fs::read(&path).unwrap()).unwrap();
        let seconds = pcm.len() as f32 / SAMPLE_RATE as f32;
        assert!((2.95..3.1).contains(&seconds), "{name}: {seconds} s");
        // Still the 440 Hz tone: 880 zero crossings a second (2 s here), steady level.
        let middle = &pcm[SAMPLE_RATE / 2..SAMPLE_RATE * 5 / 2];
        let crossings = middle
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        assert!((1740..1780).contains(&crossings), "{name}: {crossings}");
        let rms = (middle.iter().map(|x| x * x).sum::<f32>() / middle.len() as f32).sqrt();
        assert!((0.15..0.3).contains(&rms), "{name}: {rms}");
    }
}
