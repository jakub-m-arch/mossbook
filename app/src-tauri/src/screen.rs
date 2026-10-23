//! Keeps the screen on while a voice note is recorded (D-25). On Android the
//! screen would otherwise turn off after the system's timeout, which sends the
//! app to the background and locks the journal mid-sentence. Only Rust talks to
//! the Kotlin plugin (`ScreenPlugin.kt`); the web view gets no command of it.
//! On desktop the screen going dark does not lock the journal, so nothing is done.

pub use platform::{init, keep_on};

#[cfg(target_os = "android")]
mod platform {
    use serde::Serialize;
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Runtime};

    struct Handle<R: Runtime>(PluginHandle<R>);

    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("screen")
            .setup(|app, api| {
                let handle =
                    api.register_android_plugin("io.github.jakubmarch.mossbook", "ScreenPlugin")?;
                app.manage(Handle(handle));
                Ok(())
            })
            .build()
    }

    #[derive(Serialize)]
    struct Args {
        on: bool,
    }

    /// Best effort: if it fails, the screen simply follows the system timeout.
    pub fn keep_on<R: Runtime>(app: &AppHandle<R>, on: bool) {
        if let Some(handle) = app.try_state::<Handle<R>>() {
            let _ = handle
                .inner()
                .0
                .run_mobile_plugin::<serde_json::Value>("keepOn", Args { on });
        }
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::plugin::{Builder, TauriPlugin};
    use tauri::{AppHandle, Runtime};

    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("screen").build()
    }

    pub fn keep_on<R: Runtime>(_app: &AppHandle<R>, _on: bool) {}
}
