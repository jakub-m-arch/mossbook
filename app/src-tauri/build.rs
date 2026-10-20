include!("src/command_names.rs");

fn main() {
    // Build time shown in Settings ("this version is a few months old", offline).
    // SOURCE_DATE_EPOCH keeps builds reproducible (e.g. for F-Droid).
    let epoch = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        });
    println!("cargo:rustc-env=MOSSBOOK_BUILD_EPOCH={epoch}");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    // Every app command gets an `allow-<command>` permission; only those granted
    // in capabilities/default.json can be invoked from the webview (SPEC §8.4).
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
