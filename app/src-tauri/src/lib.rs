//! Mossbook app shell: Tauri setup, commands and window lifecycle (SPEC §8.2:
//! a thin layer over `mossbook-core`).

#[cfg(test)]
mod command_names;
mod commands;
mod error;
mod session;

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};

use commands::{AppState, emit_locked};
use session::Session;

/// Event telling the UI to cover or uncover its content (desktop focus loss, D-11).
const EVENT_CURTAIN: &str = "privacy-curtain";

/// How often the idle timer is checked.
const TICK: Duration = Duration::from_secs(5);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(AppState::new(Session::new(dir.join("vault.db"))));
            spawn_idle_timer(app.handle().clone());
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
            commands::vault_destroy,
            commands::entries_list,
            commands::entry_get,
            commands::entry_save,
            commands::entry_delete,
            commands::quick_entry,
            commands::tags_list,
            commands::tag_create,
            commands::tag_save,
            commands::tag_delete,
            commands::tag_merge,
            commands::plan_get,
            commands::plan_save,
            commands::settings_get,
            commands::settings_save,
            commands::stats_mood,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mossbook");
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
    let locked = state.session().on_background();
    if let Ok(Some(reason)) = locked {
        emit_locked(app, reason);
    }
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
