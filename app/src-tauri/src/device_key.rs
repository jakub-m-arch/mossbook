//! The platform side of fingerprint unlock (V-7, D-18): asks the Android
//! Keystore, after a strong biometric check, to encrypt or decrypt the vault's
//! device key. Only Rust talks to the Kotlin plugin (`DeviceKeyPlugin.kt`); the
//! web view gets no command of it. Desktop has no biometrics (SPEC §6.5).

use serde::Deserialize;
use tauri::{AppHandle, Runtime};
use zeroize::Zeroizing;

/// Texts of the system biometric prompt, in the UI language.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PromptText {
    pub title: String,
    pub subtitle: Option<String>,
    pub cancel: String,
}

impl PromptText {
    /// Plain UI strings of a sensible length.
    pub fn is_valid(&self) -> bool {
        let ok = |s: &str| !s.trim().is_empty() && s.chars().count() <= 200;
        ok(&self.title) && ok(&self.cancel) && self.subtitle.as_deref().is_none_or(ok)
    }
}

/// What the device offers for fingerprint unlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// On desktop only `Unavailable` is ever returned.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum Support {
    /// Strong biometrics are enrolled and ready.
    Ready,
    /// A strong sensor, but no fingerprint is enrolled yet.
    NoneEnrolled,
    /// No strong biometrics (or not on this platform).
    Unavailable,
}

/// Why the platform did not hand out a key. No details from the platform are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// On desktop only `Unavailable` is ever returned.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub enum PlatformError {
    /// No strong biometrics on this device, or not on this platform at all.
    Unavailable,
    /// The user closed the prompt.
    Cancelled,
    /// Too many failed attempts; the system locks biometrics for a while.
    Lockout,
    /// Fingerprints changed since the key was made; the key is gone for good.
    Invalidated,
    /// Anything else (a damaged key file, a Keystore error).
    Failed,
}

impl PlatformError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "biometric_unavailable",
            Self::Cancelled => "cancelled",
            Self::Lockout => "biometric_lockout",
            Self::Invalidated => "device_key_invalidated",
            Self::Failed => "biometric_failed",
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Serialize};
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Runtime};
    use zeroize::Zeroizing;

    use super::{PlatformError, PromptText, Support};

    struct Handle<R: Runtime>(PluginHandle<R>);

    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("device-key")
            .setup(|app, api| {
                let handle = api
                    .register_android_plugin("io.github.jakubmarch.mossbook", "DeviceKeyPlugin")?;
                app.manage(Handle(handle));
                Ok(())
            })
            .build()
    }

    fn handle<R: Runtime>(app: &AppHandle<R>) -> Result<&PluginHandle<R>, PlatformError> {
        app.try_state::<Handle<R>>()
            .map(|h| &h.inner().0)
            .ok_or(PlatformError::Unavailable)
    }

    fn map_error(e: tauri::plugin::mobile::PluginInvokeError) -> PlatformError {
        use tauri::plugin::mobile::PluginInvokeError;
        match e {
            PluginInvokeError::InvokeRejected(r) => match r.code.as_deref() {
                Some("cancelled") => PlatformError::Cancelled,
                Some("lockout") => PlatformError::Lockout,
                Some("invalidated") => PlatformError::Invalidated,
                Some("unavailable") => PlatformError::Unavailable,
                _ => PlatformError::Failed,
            },
            _ => PlatformError::Failed,
        }
    }

    #[derive(Deserialize)]
    struct StatusResponse {
        available: bool,
        enrollable: bool,
    }

    pub fn status<R: Runtime>(app: &AppHandle<R>) -> Support {
        match handle(app).and_then(|h| {
            h.run_mobile_plugin::<StatusResponse>("status", ())
                .map_err(map_error)
        }) {
            Ok(s) if s.available => Support::Ready,
            Ok(s) if s.enrollable => Support::NoneEnrolled,
            _ => Support::Unavailable,
        }
    }

    #[derive(Serialize)]
    struct SealArgs<'a> {
        key: &'a str,
        prompt: &'a PromptText,
    }

    #[derive(Deserialize)]
    struct SealResponse {
        sealed: String,
    }

    pub fn seal<R: Runtime>(
        app: &AppHandle<R>,
        key: &[u8],
        prompt: &PromptText,
    ) -> Result<Vec<u8>, PlatformError> {
        let encoded = Zeroizing::new(STANDARD.encode(key));
        let response: SealResponse = handle(app)?
            .run_mobile_plugin(
                "seal",
                SealArgs {
                    key: &encoded,
                    prompt,
                },
            )
            .map_err(map_error)?;
        STANDARD
            .decode(response.sealed)
            .map_err(|_| PlatformError::Failed)
    }

    #[derive(Serialize)]
    struct OpenArgs<'a> {
        sealed: &'a str,
        prompt: &'a PromptText,
    }

    #[derive(Deserialize)]
    struct OpenResponse {
        key: String,
    }

    pub fn open<R: Runtime>(
        app: &AppHandle<R>,
        sealed: &[u8],
        prompt: &PromptText,
    ) -> Result<Zeroizing<Vec<u8>>, PlatformError> {
        let encoded = STANDARD.encode(sealed);
        let response: OpenResponse = handle(app)?
            .run_mobile_plugin(
                "open",
                OpenArgs {
                    sealed: &encoded,
                    prompt,
                },
            )
            .map_err(map_error)?;
        let key = Zeroizing::new(response.key);
        STANDARD
            .decode(key.as_bytes())
            .map(Zeroizing::new)
            .map_err(|_| PlatformError::Failed)
    }

    pub fn forget<R: Runtime>(app: &AppHandle<R>) {
        if let Ok(h) = handle(app) {
            let _ = h.run_mobile_plugin::<serde_json::Value>("forget", ());
        }
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::plugin::{Builder, TauriPlugin};
    use tauri::{AppHandle, Runtime};
    use zeroize::Zeroizing;

    use super::{PlatformError, PromptText, Support};

    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("device-key").build()
    }

    pub fn status<R: Runtime>(_app: &AppHandle<R>) -> Support {
        Support::Unavailable
    }

    pub fn seal<R: Runtime>(
        _app: &AppHandle<R>,
        _key: &[u8],
        _prompt: &PromptText,
    ) -> Result<Vec<u8>, PlatformError> {
        Err(PlatformError::Unavailable)
    }

    pub fn open<R: Runtime>(
        _app: &AppHandle<R>,
        _sealed: &[u8],
        _prompt: &PromptText,
    ) -> Result<Zeroizing<Vec<u8>>, PlatformError> {
        Err(PlatformError::Unavailable)
    }

    pub fn forget<R: Runtime>(_app: &AppHandle<R>) {}
}

pub use platform::init;

/// What this device offers for fingerprint unlock.
pub fn status<R: Runtime>(app: &AppHandle<R>) -> Support {
    platform::status(app)
}

/// Encrypts the device key in the Keystore after a biometric check.
pub fn seal<R: Runtime>(
    app: &AppHandle<R>,
    key: &[u8],
    prompt: &PromptText,
) -> Result<Vec<u8>, PlatformError> {
    platform::seal(app, key, prompt)
}

/// Decrypts the device key after a biometric check.
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    sealed: &[u8],
    prompt: &PromptText,
) -> Result<Zeroizing<Vec<u8>>, PlatformError> {
    platform::open(app, sealed, prompt)
}

/// Deletes the Keystore key (best effort).
pub fn forget<R: Runtime>(app: &AppHandle<R>) {
    platform::forget(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(title: &str, subtitle: Option<&str>, cancel: &str) -> PromptText {
        PromptText {
            title: title.into(),
            subtitle: subtitle.map(Into::into),
            cancel: cancel.into(),
        }
    }

    #[test]
    fn prompt_texts_are_checked() {
        assert!(prompt("Open the journal", None, "Use password").is_valid());
        assert!(prompt("Open", Some("Mossbook"), "Cancel").is_valid());
        assert!(!prompt(" ", None, "Cancel").is_valid());
        assert!(!prompt("Open", None, "").is_valid());
        assert!(!prompt("Open", Some(""), "Cancel").is_valid());
        assert!(!prompt(&"x".repeat(201), None, "Cancel").is_valid());
    }

    #[test]
    fn error_codes_are_stable() {
        let codes: Vec<_> = [
            PlatformError::Unavailable,
            PlatformError::Cancelled,
            PlatformError::Lockout,
            PlatformError::Invalidated,
            PlatformError::Failed,
        ]
        .map(PlatformError::code)
        .into();
        assert_eq!(
            codes,
            [
                "biometric_unavailable",
                "cancelled",
                "biometric_lockout",
                "device_key_invalidated",
                "biometric_failed"
            ]
        );
    }
}
