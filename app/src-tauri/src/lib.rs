//! Mossbook app shell: Tauri setup, commands and window lifecycle (SPEC §8.2:
//! a thin layer over `mossbook-core`).

#[cfg(test)]
mod command_names;
mod commands;
mod device_key;
mod error;
mod screen;
mod session;
mod sync;
mod transcription;

use std::time::{Duration, Instant};

use tauri::webview::{PermissionKind, PermissionResponse};
use tauri::{AppHandle, Emitter, Manager, RunEvent, Window, WindowEvent};

use commands::{AppState, emit_locked};
use mossbook_core::vault::{self, Local};
use session::{RECORDING_GRACE, Session};
use transcription::Transcription;

/// Event telling the UI to cover or uncover its content (desktop focus loss, D-11).
const EVENT_CURTAIN: &str = "privacy-curtain";

/// How often the idle timer is checked.
const TICK: Duration = Duration::from_secs(5);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Plugins are used from Rust only; none of their JS commands is granted.
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Fingerprint unlock (Android Keystore); a no-op on desktop.
        .plugin(device_key::init())
        .plugin(screen::init())
        // The sync folder through the system's folder picker (Android); a no-op on desktop.
        .plugin(sync::init())
        // The web view may use the microphone for voice notes (E-18) and nothing
        // else: no camera, location, notifications or other web permissions.
        .on_permission_request(|_, kind| match kind {
            PermissionKind::Microphone => PermissionResponse::Allow,
            _ => PermissionResponse::Deny,
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let vault_path = dir.join("vault.db");
            let mut session = Session::new(vault_path.clone());
            // Sync stays on across restarts (D-22): the folder is recorded on this device.
            if let (Ok(Some(location)), Ok(Some(device))) = (
                vault::read_local(&vault_path, Local::SyncFolder),
                vault::read_local(&vault_path, Local::DeviceId),
            ) && let Some(folder) = sync::reopen_folder(app.handle(), &location)
            {
                session.attach_sync(folder, location, device);
            }
            // Voice note transcription (D-23): the model, when imported, lives here.
            let models = dir.join(mossbook_core::transcribe::model::DIR_NAME);
            app.manage(AppState::new(session, Transcription::new(models)));
            spawn_idle_timer(app.handle().clone());
            #[cfg(target_os = "linux")]
            if let Some(window) = app.get_webview_window("main") {
                enable_microphone(&window);
            }
            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::clock_sync,
            commands::activity_ping,
            commands::vault_status,
            commands::vault_create,
            commands::vault_cancel_create,
            commands::vault_confirm_recovery,
            commands::vault_unlock,
            commands::vault_unlock_recovery,
            commands::vault_lock,
            commands::vault_change_password,
            commands::vault_verify_recovery,
            commands::vault_destroy,
            commands::vault_unlock_device,
            commands::device_unlock_status,
            commands::device_unlock_enable,
            commands::device_unlock_disable,
            commands::entries_list,
            commands::entry_get,
            commands::entry_save,
            commands::entry_delete,
            commands::voice_save,
            commands::voice_get,
            commands::voice_recording,
            commands::voice_transcribe,
            commands::transcription_status,
            commands::transcription_open_download,
            commands::transcription_import,
            commands::transcription_cancel,
            commands::transcription_remove,
            commands::quick_entry,
            commands::quick_entry_locked,
            commands::tags_list,
            commands::tag_create,
            commands::tag_save,
            commands::tag_delete,
            commands::tag_merge,
            commands::plan_get,
            commands::plan_save,
            commands::settings_get,
            commands::settings_save,
            commands::unsaved_get,
            commands::unsaved_keep,
            commands::unsaved_discard,
            commands::stats_mood,
            commands::stats_patterns,
            commands::clipboard_clear_if,
            commands::crisis_lines,
            commands::dial,
            commands::app_info,
            commands::open_releases,
            commands::backup_export,
            commands::backup_pick,
            commands::backup_import,
            commands::backup_info,
            commands::backup_reveal,
            commands::export_readable,
            commands::sync_status,
            commands::sync_choose_folder,
            commands::sync_now,
            commands::sync_join,
            commands::sync_disconnect,
            commands::report_pdf,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Mossbook")
        .run(|app, event| {
            // Closing the app locks the vault: the WAL is checkpointed into the
            // database and the DEK and decrypted content are zeroed.
            if let RunEvent::Exit = event {
                let _ = app.state::<AppState>().session().lock();
            }
        });
}

/// Background lock and privacy curtain, decided in Rust (SPEC §8.3).
fn on_window_event(window: &Window, event: &WindowEvent) {
    match event {
        // Desktop: losing focus only covers the content (D-11).
        WindowEvent::Focused(focused) => {
            let _ = window.emit(EVENT_CURTAIN, !focused);
        }
        // Desktop: minimizing counts as going to the background.
        WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => {
            lock_on_background(window.app_handle())
        }
        // Mobile: the app went to the background.
        #[cfg(mobile)]
        WindowEvent::Suspended => lock_on_background(window.app_handle()),
        _ => {}
    }
}

fn lock_on_background(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut session = state.session();
    match session.on_background(Instant::now()) {
        Ok(Some(reason)) => emit_locked(app, reason),
        // A voice note is being recorded: the lock waits for it to be stored,
        // but never longer than the grace (D-25).
        Ok(None) if session.lock_pending() => {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(RECORDING_GRACE);
                let locked = app.state::<AppState>().session().on_tick(Instant::now());
                if let Ok(Some(reason)) = locked {
                    emit_locked(&app, reason);
                }
            });
        }
        _ => {}
    }
}

/// WebKitGTK keeps media capture off unless asked; voice notes need the
/// microphone (E-18). Access is still decided by `on_permission_request`.
#[cfg(target_os = "linux")]
fn enable_microphone(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| {
        use webkit2gtk::{SettingsExt, WebViewExt};
        if let Some(settings) = webview.inner().settings() {
            settings.set_enable_media_stream(true);
        }
    });
}

/// Locks after inactivity. The timer lives in Rust; the UI only resets it.
fn spawn_idle_timer(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            let state = app.state::<AppState>();
            let locked = state.session().on_tick(Instant::now());
            if let Ok(Some(reason)) = locked {
                emit_locked(&app, reason);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::command_names::COMMANDS;

    /// Every command must be granted in the capability, and nothing else.
    #[test]
    fn capability_grants_exactly_the_app_commands() {
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).expect("valid JSON");
        let granted: Vec<String> = cap["permissions"]
            .as_array()
            .expect("permissions array")
            .iter()
            .filter_map(|p| p.as_str())
            .filter_map(|p| p.strip_prefix("allow-"))
            .map(str::to_owned)
            .collect();
        let expected: Vec<String> = COMMANDS.iter().map(|c| c.replace('_', "-")).collect();
        assert_eq!(granted, expected);
        let others: Vec<&str> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str())
            .filter(|p| !p.starts_with("allow-"))
            .collect();
        assert_eq!(
            others,
            ["core:event:allow-listen", "core:event:allow-unlisten"]
        );
    }

    /// The release version is the same in Cargo, the Tauri config and package.json.
    #[test]
    fn versions_match() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let npm: serde_json::Value =
            serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(npm["version"], env!("CARGO_PKG_VERSION"));
    }

    /// Android hardening from SPEC §8.4 stays in the generated project: no network
    /// permission in release, no system backups, no screenshots.
    #[test]
    fn android_project_keeps_the_hardening() {
        let android = concat!(env!("CARGO_MANIFEST_DIR"), "/gen/android/app/src");
        let read = |path: &str| std::fs::read_to_string(format!("{android}/{path}")).unwrap();

        let release = read("release/AndroidManifest.xml");
        for permission in ["INTERNET", "ACCESS_NETWORK_STATE"] {
            assert!(
                release.contains(&format!(
                    "android.permission.{permission}\" tools:node=\"remove\""
                )),
                "release manifest must remove {permission}"
            );
        }

        let main = read("main/AndroidManifest.xml");
        assert!(main.contains("android:allowBackup=\"false\""));
        assert!(main.contains("android:fullBackupContent=\"false\""));
        assert!(main.contains("android:dataExtractionRules=\"@xml/data_extraction_rules\""));

        let rules = read("main/res/xml/data_extraction_rules.xml");
        assert!(!rules.contains("<include"));
        for section in ["<cloud-backup>", "<device-transfer>"] {
            assert!(rules.contains(section), "{section} missing");
        }

        let activity = read("main/java/io/github/jakubmarch/mossbook/MainActivity.kt");
        assert!(activity.contains("FLAG_SECURE"));

        // Voice notes need the microphone; nothing else that senses the user.
        assert!(main.contains("android.permission.RECORD_AUDIO"));
        for permission in ["CAMERA", "ACCESS_FINE_LOCATION", "ACCESS_COARSE_LOCATION"] {
            assert!(!main.contains(&format!("android.permission.{permission}\"")));
        }
    }

    /// macOS: the hardened runtime and WebKit both refuse the microphone
    /// silently without these (spike for D-21).
    #[test]
    fn macos_bundle_declares_the_microphone() {
        let plist = include_str!("../Info.plist");
        assert!(plist.contains("<key>NSMicrophoneUsageDescription</key>"));
        let entitlements = include_str!("../Entitlements.plist");
        assert!(entitlements.contains("<key>com.apple.security.device.audio-input</key>"));
        assert!(!entitlements.contains("com.apple.security.device.camera"));
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(
            conf["bundle"]["macOS"]["entitlements"],
            serde_json::json!("Entitlements.plist")
        );
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        assert!(
            csp.contains("media-src blob:"),
            "playback from decrypted bytes"
        );
    }

    /// Every command in the list is registered in `generate_handler!`.
    #[test]
    fn handler_registers_every_command() {
        let source = include_str!("lib.rs");
        for command in COMMANDS {
            assert!(
                source.contains(&format!("commands::{command},")),
                "{command} not registered"
            );
        }
    }
}

/// JSON shapes that `app/src/lib/api/types.ts` relies on (decision D-7).
#[cfg(test)]
mod ipc_shapes {
    use mossbook_core::domain::{AutoLock, Entry, EntryKind, EntryKindTag, Settings};
    use mossbook_core::query::stats::MoodPoint;
    use serde_json::json;
    use time::macros::{date, datetime};

    #[test]
    fn json_shapes_match_typescript_types() {
        let mut e = Entry::new(
            EntryKind::empty(EntryKindTag::Fear),
            datetime!(2026-10-19 21:30 +02:00),
        )
        .unwrap();
        e.mood = Some(3);
        let v = serde_json::to_value(&e).unwrap();
        assert!(v["id"].is_string());
        assert_eq!(v["occurred_at"], json!("2026-10-19T21:30:00+02:00"));
        assert_eq!(v["kind"]["fear"]["intensity_after"], json!(null));
        assert_eq!(v["kind"]["fear"]["symptoms"], json!([]));
        assert_eq!(
            serde_json::to_value(EntryKind::Note).unwrap(),
            json!("note")
        );

        let s = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(s["autolock"], json!("immediately"));
        assert_eq!(s["auto_backup"], json!(true));
        assert_eq!(s["last_export"], json!(null));
        let info = crate::session::BackupInfo {
            auto_enabled: true,
            auto_latest: Some(datetime!(2026-10-20 08:00 +02:00).into()),
            auto_count: 1,
            auto_failed: false,
            can_reveal: true,
            last_export: None,
        };
        assert_eq!(
            serde_json::to_value(info).unwrap(),
            json!({
                "auto_enabled": true,
                "auto_latest": "2026-10-20T08:00:00+02:00",
                "auto_count": 1,
                "auto_failed": false,
                "can_reveal": true,
                "last_export": null
            })
        );
        assert_eq!(
            serde_json::to_value(AutoLock::After1Min).unwrap(),
            json!("after1_min")
        );

        let p = MoodPoint {
            date: date!(2026 - 10 - 18),
            average: 3.5,
            count: 2,
        };
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            json!({ "date": "2026-10-18", "average": 3.5, "count": 2 })
        );
    }
}
