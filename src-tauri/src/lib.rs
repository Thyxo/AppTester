mod apps;
mod appwin;

use apps::AppEntry;
use tauri::AppHandle;

#[tauri::command]
fn list_apps() -> Result<Vec<AppEntry>, String> {
    apps::all().map(<[AppEntry]>::to_vec)
}

#[tauri::command]
fn open_app(app: AppHandle, id: String) -> Result<(), String> {
    let entry = apps::find(&id)?;
    appwin::open(&app, entry)
        .map(|_| ())
        .map_err(|e| format!("{} kunne ikke åbnes: {e}", entry.name))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_apps, open_app])
        .run(tauri::generate_context!())
        .expect("PWA Launcher kunne ikke starte");
}
