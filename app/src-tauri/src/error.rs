//! Error returned by every command: a stable machine code for the UI (which
//! translates it) and an English message for developers. Never user data.

use std::time::Duration;

use mossbook_core::backup::BackupError;
use mossbook_core::domain::DomainError;
use mossbook_core::journal::JournalError;
use mossbook_core::transcribe::TranscribeError;
use mossbook_core::vault::VaultError;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    /// Set for `throttled` and `wrong_password`: wait this long before retrying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_ms: None,
        }
    }

    pub fn locked() -> Self {
        Self::new("locked", "the vault is locked")
    }

    pub fn invalid_state(message: &'static str) -> Self {
        Self::new("invalid_state", message)
    }

    pub fn throttled(wait: Duration) -> Self {
        Self {
            retry_after_ms: Some(millis(wait)),
            ..Self::new("throttled", "too many attempts, wait")
        }
    }

    pub fn with_retry(mut self, wait: Duration) -> Self {
        self.retry_after_ms = Some(millis(wait));
        self
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl From<VaultError> for AppError {
    fn from(e: VaultError) -> Self {
        let code = match e {
            VaultError::NotFound => "no_vault",
            VaultError::AlreadyExists => "vault_exists",
            VaultError::WrongPassword => "wrong_password",
            VaultError::WrongRecoveryKey => "wrong_recovery_key",
            VaultError::InvalidRecoveryPhrase => "invalid_recovery_phrase",
            VaultError::PasswordTooShort => "password_too_short",
            VaultError::NotConfirmed => "not_confirmed",
            VaultError::UnsupportedFormat => "unsupported_format",
            VaultError::Corrupted => "vault_corrupted",
            VaultError::InboxUnavailable => "inbox_unavailable",
            VaultError::DeviceKeyRejected => "device_key_rejected",
            VaultError::Crypto(_) | VaultError::Storage(_) | VaultError::Io => "internal",
        };
        Self::new(code, e.to_string())
    }
}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        let code = match e {
            DomainError::InvalidField(_) => "invalid_field",
            DomainError::NotFound => "not_found",
            DomainError::Encoding | DomainError::Rng => "internal",
        };
        // The message names the field (e.g. "mood"), never its value.
        Self::new(code, e.to_string())
    }
}

impl From<JournalError> for AppError {
    fn from(e: JournalError) -> Self {
        match e {
            JournalError::Domain(e) => e.into(),
            JournalError::Vault(e) => e.into(),
            JournalError::DuplicateTagName => Self::new("duplicate_tag_name", e.to_string()),
        }
    }
}

impl From<BackupError> for AppError {
    fn from(e: BackupError) -> Self {
        let code = match e {
            BackupError::NotABackup => "not_a_backup",
            BackupError::UnsupportedVersion => "unsupported_backup",
            BackupError::Corrupted => "backup_corrupted",
            BackupError::TooLarge => "backup_too_large",
            BackupError::OtherJournal => "other_journal",
            BackupError::WrongPassword => "wrong_password",
            BackupError::WrongRecoveryKey => "wrong_recovery_key",
            BackupError::InvalidRecoveryPhrase => "invalid_recovery_phrase",
            BackupError::Vault(e) => return e.into(),
        };
        Self::new(code, e.to_string())
    }
}

impl From<TranscribeError> for AppError {
    fn from(e: TranscribeError) -> Self {
        let code = match e {
            TranscribeError::ModelMissing => "transcription_off",
            TranscribeError::ModelInvalid => "model_invalid",
            TranscribeError::ModelLoad => "model_load",
            TranscribeError::Audio => "transcription_audio",
            TranscribeError::Cancelled => "cancelled",
            TranscribeError::Io => "io",
        };
        Self::new(code, e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_message_and_optional_retry() {
        let json = serde_json::to_string(&AppError::from(VaultError::WrongPassword)).unwrap();
        assert_eq!(
            json,
            r#"{"code":"wrong_password","message":"wrong password"}"#
        );
        let json =
            serde_json::to_string(&AppError::throttled(Duration::from_millis(1500))).unwrap();
        assert!(json.contains(r#""retry_after_ms":1500"#));
    }

    #[test]
    fn field_errors_name_the_field() {
        let e = AppError::from(JournalError::Domain(DomainError::InvalidField("mood")));
        assert_eq!(e.code, "invalid_field");
        assert!(e.message.contains("mood"));
    }
}
