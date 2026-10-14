fn main() {
    // Appens egne kommandoer kræver eksplicit tilladelse i capabilities/,
    // så eksterne webapps i app-vinduerne ikke kan kalde dem.
    let manifest = tauri_build::AppManifest::new().commands(&["list_apps", "open_app"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri-build fejlede");
}
