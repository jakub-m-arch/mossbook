include!("src/command_names.rs");

fn main() {
    // Every app command gets an `allow-<command>` permission; only those granted
    // in capabilities/default.json can be invoked from the webview (SPEC §8.4).
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
