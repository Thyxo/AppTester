mod appwin;

use appwin::AppSpec;

/// Trin (a): én hardcoded app. Erstattes af launcher-griddet i trin (c).
const HARDCODED_APP: (&str, &str, &str) = (
    "stopwatchplus",
    "Stopwatch+",
    "https://stopwatchplus.vercel.app/",
);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (id, name, url) = HARDCODED_APP;
            let spec = AppSpec::new(id, name, url)?;
            appwin::open(app.handle(), &spec)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("PWA Launcher kunne ikke starte");
}
