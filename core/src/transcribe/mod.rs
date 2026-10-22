//! Voice notes turned into text on the device (E-19, decision D-23).
//!
//! Off until the user imports the model ([`model`]). A recording is decoded
//! to 16 kHz ([`audio`]), cut into pieces at pauses and run through NVIDIA
//! Parakeet TDT 0.6B v3 with sherpa-onnx (onnxruntime). Nothing leaves the
//! device. The result is a proposal: the UI shows it for the user to read and
//! correct before it is stored with the entry.
//!
//! The decoded samples are zeroed on drop; the copies sherpa-onnx makes in
//! native memory cannot be, like the web view's copy of what is typed.

pub mod audio;
pub mod model;

use core::fmt;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

pub use model::{ModelSpec, PARAKEET_V3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscribeError {
    /// The model is not imported on this device (transcription is off).
    ModelMissing,
    /// The picked file is not the expected model archive.
    ModelInvalid,
    /// The model is in place but the recognizer could not start, e.g. for lack
    /// of memory.
    ModelLoad,
    /// The recording cannot be decoded.
    Audio,
    Cancelled,
    Io,
}

impl fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ModelMissing => "the transcription model is not installed",
            Self::ModelInvalid => "not the expected model archive",
            Self::ModelLoad => "the transcription model could not be loaded",
            Self::Audio => "the recording cannot be decoded",
            Self::Cancelled => "transcription was cancelled",
            Self::Io => "cannot read or write the model files",
        })
    }
}

impl std::error::Error for TranscribeError {}

impl From<io::Error> for TranscribeError {
    fn from(_: io::Error) -> Self {
        Self::Io
    }
}

/// The loaded model. It takes about 1.2 GB of memory, so it is loaded for one
/// job and dropped after it.
pub struct Transcriber {
    recognizer: OfflineRecognizer,
}

impl fmt::Debug for Transcriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transcriber").finish_non_exhaustive()
    }
}

impl Transcriber {
    /// Loads the model imported under `root`, running on `threads` CPU threads.
    pub fn load(root: &Path, spec: &ModelSpec, threads: usize) -> Result<Self, TranscribeError> {
        if !model::is_installed(root, spec) {
            return Err(TranscribeError::ModelMissing);
        }
        let dir = model::model_dir(root, spec);
        let path = |name: &str| Some(dir.join(name).to_string_lossy().into_owned());
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer = OfflineTransducerModelConfig {
            encoder: path(model::ENCODER),
            decoder: path(model::DECODER),
            joiner: path(model::JOINER),
        };
        config.model_config.tokens = path(model::TOKENS);
        config.model_config.model_type = Some("nemo_transducer".into());
        config.model_config.num_threads = i32::try_from(threads.clamp(1, 8)).unwrap_or(1);
        // Never print the configuration or intermediate results.
        config.model_config.debug = false;
        let recognizer = OfflineRecognizer::create(&config).ok_or(TranscribeError::ModelLoad)?;
        Ok(Self { recognizer })
    }

    /// Transcribes a stored Ogg Opus recording. `progress` gets (pieces done,
    /// pieces in total); `cancel` is checked between pieces.
    pub fn transcribe(
        &self,
        recording: &[u8],
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<String, TranscribeError> {
        let pcm = audio::decode(recording)?;
        let pieces = audio::pieces(&pcm);
        let mut text = String::new();
        progress(0, pieces.len());
        for (done, piece) in pieces.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(TranscribeError::Cancelled);
            }
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(audio::SAMPLE_RATE as i32, &pcm[piece.clone()]);
            self.recognizer.decode(&stream);
            if let Some(result) = stream.get_result() {
                let words = result.text.trim();
                if !words.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(words);
                }
            }
            progress(done + 1, pieces.len());
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn without_the_model_transcription_is_off() {
        let root = TempDir::new().unwrap();
        assert_eq!(
            Transcriber::load(root.path(), &PARAKEET_V3, 2).err(),
            Some(TranscribeError::ModelMissing)
        );
    }

    /// Runs the real model when `MOSSBOOK_MODEL_ROOT` points to a folder with it
    /// imported (it is ~670 MB, so not in CI): the synthetic tone fixtures hold
    /// no speech, and the model must not invent any.
    #[test]
    #[ignore = "needs the imported model; set MOSSBOOK_MODEL_ROOT"]
    fn the_real_model_hears_no_words_in_a_tone() {
        let root = std::env::var("MOSSBOOK_MODEL_ROOT").expect("MOSSBOOK_MODEL_ROOT");
        let t = Transcriber::load(Path::new(&root), &PARAKEET_V3, 4).unwrap();
        let tone = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/voice/android-webview.ogg"
        ))
        .unwrap();
        let mut seen = Vec::new();
        let text = t
            .transcribe(&tone, &AtomicBool::new(false), &mut |d, n| {
                seen.push((d, n))
            })
            .unwrap();
        assert_eq!(text, "");
        assert_eq!(seen, vec![(0, 1), (1, 1)]);
        assert_eq!(
            t.transcribe(&tone, &AtomicBool::new(true), &mut |_, _| {}),
            Err(TranscribeError::Cancelled)
        );
    }
}
