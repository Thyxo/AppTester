//! App-vinduer: ét Tauri-vindue pr. webapp med egen, vedvarende data-mappe.

pub mod menu;
mod status;
mod viewport;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::webview::{Color, NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, Manager, Theme, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

use crate::apps::AppEntry;
use crate::manifest::{self, Meta};
use crate::secrets;
use crate::store::{Store, WindowGeom};

/// Standardstørrelse: en telefon på højkant.
const DEFAULT_SIZE: (f64, f64) = (430.0, 900.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deployment {
    Production,
    Preview,
}

impl Deployment {
    fn label(self) -> &'static str {
        match self {
            Deployment::Production => "production",
            Deployment::Preview => "preview",
        }
    }
}

/// Tilstand for ét åbent app-vindue.
pub struct WinCtx {
    entry: AppEntry,
    nav: NavPolicy,
    deployment: Mutex<Deployment>,
    title: Mutex<String>,
}

impl WinCtx {
    fn deployment(&self) -> Deployment {
        *self.deployment.lock().unwrap()
    }

    /// Start-URL for den valgte deployment. Previews får Vercels bypass-token på,
    /// hvis et er gemt; Vercel sætter så en cookie, og resten af sessionen virker.
    fn start_url(&self) -> Url {
        match (self.deployment(), &self.entry.preview) {
            (Deployment::Preview, Some(preview)) => {
                with_bypass(preview, secrets::bypass_token(&self.entry.id))
            }
            _ => self.entry.url.clone(),
        }
    }
}

fn with_bypass(url: &Url, token: Option<String>) -> Url {
    let mut url = url.clone();
    if let Some(token) = token {
        url.query_pairs_mut()
            .append_pair("x-vercel-protection-bypass", &token)
            .append_pair("x-vercel-set-bypass-cookie", "true");
    }
    url
}

/// Alle åbne app-vinduer, nøglet på vindues-label.
#[derive(Default)]
pub struct Windows(Mutex<HashMap<String, Arc<WinCtx>>>);

impl Windows {
    fn get(&self, label: &str) -> Option<Arc<WinCtx>> {
        self.0.lock().unwrap().get(label).cloned()
    }
}

/// Åbner appen i sit eget vindue, eller fokuserer det, hvis det allerede er åbent.
pub fn open(app: &AppHandle, spec: &AppEntry, meta: Option<&Meta>) -> tauri::Result<WebviewWindow> {
    let label = format!("app-{}", spec.id);
    if let Some(win) = app.get_webview_window(&label) {
        win.unminimize()?;
        win.set_focus()?;
        return Ok(win);
    }

    let mut allowed = vec![&spec.url];
    allowed.extend(spec.preview.as_ref());
    let ctx = Arc::new(WinCtx {
        entry: spec.clone(),
        nav: NavPolicy::new(&allowed),
        deployment: Mutex::new(Deployment::Production),
        title: Mutex::new(String::new()),
    });

    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(ctx.start_url()))
        .title(status::base_title(&ctx))
        .min_inner_size(240.0, 320.0)
        .on_navigation({
            let ctx = ctx.clone();
            let app = app.clone();
            move |url| {
                if ctx.nav.allows(url) {
                    true
                } else {
                    open_external(&app, url);
                    false
                }
            }
        })
        .on_new_window({
            // target="_blank" / window.open: samme app → samme vindue, ellers browseren.
            let ctx = ctx.clone();
            let app = app.clone();
            let label = label.clone();
            move |url, _features| {
                if ctx.nav.allows(&url) {
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
            let ctx = ctx.clone();
            move |_win, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    ctx.nav.mark_loaded();
                }
            }
        });

    // Testmenu pr. vindue. På macOS ligger den i app-menuen (se menu::install_app_menu).
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.menu(menu::window_menu(app, &label, spec.preview.is_some())?);
    }

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
    app.state::<Windows>()
        .0
        .lock()
        .unwrap()
        .insert(label.clone(), ctx.clone());

    let id = spec.id.clone();
    let handle = win.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { .. } => {
            if let Some(geom) = current_geom(&handle) {
                handle
                    .state::<Store>()
                    .update(|s| s.windows.insert(id.clone(), geom));
            }
        }
        WindowEvent::Destroyed => {
            handle
                .state::<Windows>()
                .0
                .lock()
                .unwrap()
                .remove(handle.label());
        }
        _ => {}
    });

    status::spawn_poller(app.clone(), label);
    Ok(win)
}

/// Skifter mellem production og preview og indlæser den nye adresse.
fn switch_deployment(win: &WebviewWindow, ctx: &WinCtx, to: Deployment) {
    if to == Deployment::Preview && ctx.entry.preview.is_none() {
        return;
    }
    *ctx.deployment.lock().unwrap() = to;
    // Tillad Vercels login-redirect m.m. igen indtil den nye side er indlæst.
    ctx.nav.reset_loaded();
    status::reset_title(win, ctx);
    let _ = win.navigate(ctx.start_url());
}

/// Vinduets placering i logiske pixels – ikke når det er minimeret/maksimeret.
fn current_geom(win: &WebviewWindow) -> Option<WindowGeom> {
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

fn on_some_monitor(app: &AppHandle, g: &WindowGeom) -> bool {
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
/// Appens egne origins (production og evt. preview) er altid tilladt. Indtil
/// siden er færdigindlæst, tillades og huskes også redirects (fx `*.vercel.app`
/// → eget domæne, eller Vercels login til beskyttede previews).
struct NavPolicy {
    origins: Mutex<HashSet<String>>,
    loaded: AtomicBool,
}

impl NavPolicy {
    fn new(start: &[&Url]) -> Self {
        Self {
            origins: Mutex::new(start.iter().map(|u| origin_key(u)).collect()),
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

    fn reset_loaded(&self) {
        self.loaded.store(false, Ordering::SeqCst);
    }
}

fn origin_key(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn open_external(app: &AppHandle, url: &Url) {
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
        let nav = NavPolicy::new(&[&url("https://stopwatchplus.vercel.app/")]);
        nav.mark_loaded();
        assert!(nav.allows(&url("https://stopwatchplus.vercel.app/settings?x=1")));
        assert!(!nav.allows(&url("https://github.com/Thyxo")));
        assert!(!nav.allows(&url("mailto:a@b.dk")));
        assert!(nav.allows(&url("about:blank")));
    }

    #[test]
    fn redirects_before_first_load_are_adopted() {
        let nav = NavPolicy::new(&[&url("https://calcplus.vercel.app/")]);
        assert!(nav.allows(&url("https://calcplus.dk/")));
        nav.mark_loaded();
        assert!(nav.allows(&url("https://calcplus.dk/om")));
        assert!(!nav.allows(&url("https://example.com/")));
        nav.reset_loaded();
        assert!(nav.allows(&url("https://vercel.com/sso-api")));
    }

    #[test]
    fn preview_origin_is_allowed() {
        let nav = NavPolicy::new(&[
            &url("https://calcplus.vercel.app/"),
            &url("https://calcplus-git-dev-thyxo.vercel.app/"),
        ]);
        nav.mark_loaded();
        assert!(nav.allows(&url("https://calcplus-git-dev-thyxo.vercel.app/x")));
    }

    #[test]
    fn bypass_token_is_added_to_query() {
        let u = with_bypass(&url("https://x.vercel.app/?a=1"), Some("tok en".into()));
        assert_eq!(
            u.as_str(),
            "https://x.vercel.app/?a=1&x-vercel-protection-bypass=tok+en&x-vercel-set-bypass-cookie=true"
        );
        assert_eq!(
            with_bypass(&url("https://x.dk/"), None).as_str(),
            "https://x.dk/"
        );
    }
}
