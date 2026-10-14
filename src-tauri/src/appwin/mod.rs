//! App-vinduer: ét Tauri-vindue pr. webapp med egen, vedvarende data-mappe.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::webview::{Color, NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, Manager, Runtime, Theme, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

use crate::apps::AppEntry;
use crate::manifest::{self, Meta};
use crate::store::{Store, WindowGeom};

/// Standardstørrelse: en telefon på højkant.
const DEFAULT_SIZE: (f64, f64) = (430.0, 900.0);

/// Åbner appen i sit eget vindue, eller fokuserer det, hvis det allerede er åbent.
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    spec: &AppEntry,
    meta: Option<&Meta>,
) -> tauri::Result<WebviewWindow<R>> {
    let label = format!("app-{}", spec.id);
    if let Some(win) = app.get_webview_window(&label) {
        win.unminimize()?;
        win.set_focus()?;
        return Ok(win);
    }

    let nav = Arc::new(NavPolicy::new(&spec.url));

    let mut builder =
        WebviewWindowBuilder::new(app, &label, WebviewUrl::External(spec.url.clone()))
            .title(&spec.name)
            .min_inner_size(240.0, 320.0)
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

    // Husket placering, hvis den stadig er på en tilsluttet skærm.
    let saved = app
        .state::<Store>()
        .read(|s| s.windows.get(&spec.id).copied())
        .filter(|g| on_some_monitor(app, g));
    builder = match saved {
        Some(g) => builder.position(g.x, g.y).inner_size(g.width, g.height),
        None => builder.inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1).center(),
    };

    // Farver fra manifestet: baggrund mens siden indlæses, lyst/mørkt tema til titellinjen.
    if let Some(meta) = meta {
        if let Some((r, g, b)) = meta
            .background_color
            .as_deref()
            .and_then(manifest::parse_hex_color)
        {
            builder = builder.background_color(Color(r, g, b, 255));
        }
        if let Some(rgb) = meta
            .theme_color
            .as_deref()
            .and_then(manifest::parse_hex_color)
        {
            let theme = if manifest::is_dark(rgb) {
                Theme::Dark
            } else {
                Theme::Light
            };
            builder = builder.theme(Some(theme));
        }
    }

    let win = builder.build()?;

    let id = spec.id.clone();
    let handle = win.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            if let Some(geom) = current_geom(&handle) {
                handle
                    .state::<Store>()
                    .update(|s| s.windows.insert(id.clone(), geom));
            }
        }
    });

    Ok(win)
}

/// Vinduets placering i logiske pixels – ikke når det er minimeret/maksimeret.
fn current_geom<R: Runtime>(win: &WebviewWindow<R>) -> Option<WindowGeom> {
    if win.is_minimized().ok()? || win.is_maximized().ok()? {
        return None;
    }
    let scale = win.scale_factor().ok()?;
    let pos = win.outer_position().ok()?.to_logical::<f64>(scale);
    let size = win.inner_size().ok()?.to_logical::<f64>(scale);
    Some(WindowGeom {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn on_some_monitor<R: Runtime>(app: &AppHandle<R>, g: &WindowGeom) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return true;
    };
    // Titellinjens venstre del skal kunne gribes.
    let (px, py) = (g.x + 40.0, g.y + 10.0);
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        px >= pos.x && px < pos.x + size.width && py >= pos.y && py < pos.y + size.height
    })
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
}
