mod apps;
mod appwin;
mod manifest;
mod secrets;
mod store;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, Url};

use apps::AppsFile;
use manifest::Meta;
use store::Store;

/// Det launcher-griddet viser for én app.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    id: String,
    name: String,
    url: Url,
    preview: Option<Url>,
    /// Om der er gemt et Vercel bypass-token i keychain.
    has_bypass: bool,
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
fn list_apps(
    app: AppHandle,
    apps: State<AppsFile>,
    store: State<Store>,
) -> Result<Vec<AppView>, String> {
    let cache = cache_dir(&app).ok();
    let views = apps
        .load()?
        .iter()
        .map(|a| AppView {
            id: a.id.clone(),
            name: a.name.clone(),
            url: a.url.clone(),
            preview: a.preview.clone(),
            has_bypass: a.preview.is_some() && secrets::bypass_token(&a.id).is_some(),
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
async fn refresh_meta(
    app: AppHandle,
    apps: State<'_, AppsFile>,
    http: State<'_, Http>,
    id: String,
) -> Result<Meta, String> {
    let entry = apps.find(&id)?;
    let meta = manifest::fetch(&http.0, &entry.url).await?;
    if let Err(err) = manifest::save_cached(&cache_dir(&app)?, &id, &meta) {
        eprintln!("Kunne ikke cache ikon for {id}: {err}");
    }
    Ok(meta)
}

#[tauri::command]
fn toggle_favorite(apps: State<AppsFile>, store: State<Store>, id: String) -> Result<bool, String> {
    apps.find(&id)?;
    Ok(store.update(|s| {
        if s.favorites.remove(&id) {
            false
        } else {
            s.favorites.insert(id);
            true
        }
    }))
}

/// Gemmer (eller sletter ved tom streng) bypass-token til appens preview.
#[tauri::command]
fn set_bypass_token(apps: State<AppsFile>, id: String, token: String) -> Result<bool, String> {
    let entry = apps.find(&id)?;
    if entry.preview.is_none() {
        return Err(format!("{} har ingen \"preview\" i apps.json", entry.name));
    }
    secrets::set_bypass_token(&id, Some(&token))?;
    Ok(secrets::bypass_token(&id).is_some())
}

#[tauri::command]
fn open_app(
    app: AppHandle,
    apps: State<AppsFile>,
    store: State<Store>,
    id: String,
) -> Result<(), String> {
    let entry = apps.find(&id)?;
    let meta = cache_dir(&app)
        .ok()
        .and_then(|c| manifest::load_cached(&c, &id));
    appwin::open(&app, &entry, meta.as_ref())
        .map_err(|e| format!("{} kunne ikke åbnes: {e}", entry.name))?;
    store.update(|s| s.last_opened.insert(id, store::now_ms()));
    Ok(())
}

/// Åbner apps.json i standardeditoren (Notesblok, hvis .json ikke har en).
#[tauri::command]
fn edit_apps_file(app: AppHandle, apps: State<AppsFile>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    apps.ensure_exists()
        .map_err(|e| format!("Kan ikke oprette {}: {e}", apps.path().display()))?;
    let path = apps.path().to_string_lossy().into_owned();
    let opened = app.opener().open_path(&path, None::<&str>);
    #[cfg(windows)]
    let opened = opened.or_else(|_| app.opener().open_path(&path, Some("notepad.exe")));
    opened.map_err(|e| format!("Kunne ikke åbne {path}: {e}"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let apps = AppsFile::new(config_dir.join("apps.json"));
            if let Err(err) = apps.ensure_exists() {
                eprintln!("Kunne ikke oprette {}: {err}", apps.path().display());
            }
            app.manage(apps);
            app.manage(Store::load(config_dir.join("state.json")));
            app.manage(Http(manifest::http_client()));
            app.manage(appwin::Windows::default());
            app.on_menu_event(appwin::menu::handle);
            #[cfg(target_os = "macos")]
            appwin::menu::install_app_menu(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_apps,
            refresh_meta,
            toggle_favorite,
            set_bypass_token,
            edit_apps_file,
            open_app
        ])
        .run(tauri::generate_context!())
        .expect("PWA Launcher kunne ikke starte");
}
