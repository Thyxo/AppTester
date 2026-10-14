mod apps;
mod appwin;
mod manifest;
mod store;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, Url};

use manifest::Meta;
use store::Store;

/// Det launcher-griddet viser for én app.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    id: String,
    name: String,
    url: Url,
    favorite: bool,
    last_opened: Option<u64>,
    /// Sidst hentede manifest-data (fra cache – virker offline).
    meta: Option<Meta>,
}

struct Http(reqwest::Client);

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_cache_dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_apps(app: AppHandle, store: State<Store>) -> Result<Vec<AppView>, String> {
    let cache = cache_dir(&app).ok();
    let views = apps::all()?
        .iter()
        .map(|a| AppView {
            id: a.id.clone(),
            name: a.name.clone(),
            url: a.url.clone(),
            favorite: store.read(|s| s.favorites.contains(&a.id)),
            last_opened: store.read(|s| s.last_opened.get(&a.id).copied()),
            meta: cache
                .as_deref()
                .and_then(|c| manifest::load_cached(c, &a.id)),
        })
        .collect();
    Ok(views)
}

/// Henter manifest + ikon på ny og opdaterer cachen.
#[tauri::command]
async fn refresh_meta(app: AppHandle, http: State<'_, Http>, id: String) -> Result<Meta, String> {
    let entry = apps::find(&id)?;
    let meta = manifest::fetch(&http.0, &entry.url).await?;
    if let Err(err) = manifest::save_cached(&cache_dir(&app)?, &id, &meta) {
        eprintln!("Kunne ikke cache ikon for {id}: {err}");
    }
    Ok(meta)
}

#[tauri::command]
fn toggle_favorite(store: State<Store>, id: String) -> Result<bool, String> {
    apps::find(&id)?;
    Ok(store.update(|s| {
        if s.favorites.remove(&id) {
            false
        } else {
            s.favorites.insert(id);
            true
        }
    }))
}

#[tauri::command]
fn open_app(app: AppHandle, store: State<Store>, id: String) -> Result<(), String> {
    let entry = apps::find(&id)?;
    let meta = cache_dir(&app)
        .ok()
        .and_then(|c| manifest::load_cached(&c, &id));
    appwin::open(&app, entry, meta.as_ref())
        .map_err(|e| format!("{} kunne ikke åbnes: {e}", entry.name))?;
    store.update(|s| s.last_opened.insert(id, store::now_ms()));
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state_file = app.path().app_config_dir()?.join("state.json");
            app.manage(Store::load(state_file));
            app.manage(Http(manifest::http_client()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_apps,
            refresh_meta,
            toggle_favorite,
            open_app
        ])
        .run(tauri::generate_context!())
        .expect("PWA Launcher kunne ikke starte");
}
