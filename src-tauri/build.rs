fn main() {
    // Appens egne kommandoer kræver eksplicit tilladelse i capabilities/,
    // så eksterne webapps i app-vinduerne ikke kan kalde dem.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "list_apps",
        "refresh_meta",
        "toggle_favorite",
        "set_bypass_token",
        "add_app",
        "remove_app",
        "edit_apps_file",
        "open_app",
        "frame_init",
        "frame_action",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build fejlede");
}
