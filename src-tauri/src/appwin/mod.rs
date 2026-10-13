//! App-vinduer: ét Tauri-vindue pr. webapp med egen, vedvarende data-mappe.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

/// Det launcheren skal vide for at åbne en app.
#[derive(Debug, Clone)]
pub struct AppSpec {
    /// Stabilt id (fx Vercel-projektnavn). Bruges til vindues-label og data-mappe.
    pub id: String,
    pub name: String,
    pub url: Url,
}

impl AppSpec {
    pub fn new(id: &str, name: &str, url: &str) -> Result<Self, url::ParseError> {
        Ok(Self {
            id: sanitize_id(id),
            name: name.to_string(),
            url: Url::parse(url)?,
        })
    }

    fn label(&self) -> String {
        format!("app-{}", self.id)
    }
}

/// Åbner appen i sit eget vindue, eller fokuserer det, hvis det allerede er åbent.
pub fn open<R: Runtime>(app: &AppHandle<R>, spec: &AppSpec) -> tauri::Result<WebviewWindow<R>> {
    let label = spec.label();
    if let Some(win) = app.get_webview_window(&label) {
        win.unminimize()?;
        win.set_focus()?;
        return Ok(win);
    }

    let nav = Arc::new(NavPolicy::new(&spec.url));

    let mut builder =
        WebviewWindowBuilder::new(app, &label, WebviewUrl::External(spec.url.clone()))
            .title(&spec.name)
            .inner_size(430.0, 900.0)
            .min_inner_size(320.0, 480.0)
            .on_navigation({
                let nav = nav.clone();
                let app = app.clone();
                move |url| {
                    if nav.allows(url) {
                        true
                    } else {
                        open_external(&app, url);
                        false
                    }
                }
            })
            .on_new_window({
                // target="_blank" / window.open: samme app → samme vindue, ellers browseren.
                let nav = nav.clone();
                let app = app.clone();
                let label = label.clone();
                move |url, _features| {
                    if nav.allows(&url) {
                        if let Some(win) = app.get_webview_window(&label) {
                            let _ = win.navigate(url);
                        }
                    } else {
                        open_external(&app, &url);
                    }
                    NewWindowResponse::Deny
                }
            })
            .on_page_load({
                let nav = nav.clone();
                move |_win, payload| {
                    if payload.event() == PageLoadEvent::Finished {
                        nav.mark_loaded();
                    }
                }
            });

    // Hver app får sin egen storage, cookies, cache og service worker,
    // som overlever genstart. macOS har ingen data-mappe, men en data store-id (macOS 14+).
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let dir = app.path().app_data_dir()?.join("webdata").join(&spec.id);
        builder = builder.data_directory(dir);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        builder = builder.data_store_identifier(store_identifier(&spec.id));
    }

    builder.build()
}

/// Hvilke navigationer der bliver i app-vinduet.
///
/// Appens egen origin er altid tilladt. Indtil første side er færdigindlæst,
/// tillades og huskes også redirects (fx `*.vercel.app` → eget domæne,
/// eller Vercels login til beskyttede previews).
struct NavPolicy {
    origins: Mutex<HashSet<String>>,
    loaded: AtomicBool,
}

impl NavPolicy {
    fn new(start: &Url) -> Self {
        Self {
            origins: Mutex::new(HashSet::from([origin_key(start)])),
            loaded: AtomicBool::new(false),
        }
    }

    fn allows(&self, url: &Url) -> bool {
        match url.scheme() {
            "http" | "https" => {
                let key = origin_key(url);
                let mut origins = self.origins.lock().unwrap();
                if origins.contains(&key) {
                    true
                } else if !self.loaded.load(Ordering::SeqCst) {
                    origins.insert(key);
                    true
                } else {
                    false
                }
            }
            // Interne sider og indhold uden netværk.
            "about" | "data" | "blob" | "javascript" => true,
            // mailto:, tel:, sms: osv. håndteres af OS'et.
            _ => false,
        }
    }

    fn mark_loaded(&self) {
        self.loaded.store(true, Ordering::SeqCst);
    }
}

fn origin_key(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    if let Err(err) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("Kunne ikke åbne {url} i standardbrowseren: {err}");
    }
}

/// Tilladte tegn i vindues-labels og mappenavne.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Stabil 16-byte id pr. app til WKWebsiteDataStore (to FNV-1a-hashes).
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn store_identifier(id: &str) -> [u8; 16] {
    fn fnv1a(seed: u64, data: &[u8]) -> u64 {
        data.iter().fold(seed, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        })
    }
    let a = fnv1a(0xcbf2_9ce4_8422_2325, id.as_bytes());
    let b = fnv1a(0x6c62_272e_07bb_0142, id.as_bytes());
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&a.to_be_bytes());
    out[8..].copy_from_slice(&b.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn same_origin_stays_in_app() {
        let nav = NavPolicy::new(&url("https://stopwatchplus.vercel.app/"));
        nav.mark_loaded();
        assert!(nav.allows(&url("https://stopwatchplus.vercel.app/settings?x=1")));
        assert!(!nav.allows(&url("https://github.com/Thyxo")));
        assert!(!nav.allows(&url("mailto:a@b.dk")));
        assert!(nav.allows(&url("about:blank")));
    }

    #[test]
    fn redirects_before_first_load_are_adopted() {
        let nav = NavPolicy::new(&url("https://calcplus.vercel.app/"));
        assert!(nav.allows(&url("https://calcplus.dk/")));
        nav.mark_loaded();
        assert!(nav.allows(&url("https://calcplus.dk/om")));
        assert!(!nav.allows(&url("https://example.com/")));
    }

    #[test]
    fn ids_are_label_safe() {
        assert_eq!(sanitize_id("Penge Trick!"), "penge_trick_");
        assert_eq!(sanitize_id("notes-app"), "notes-app");
    }
}
