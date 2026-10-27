//! App-vinduer som enheds-mockups.
//!
//! Hvert app-vindue er gennemsigtigt og uden systemramme og indeholder to webviews:
//! - `frame-<id>`: lokal side (ui/frame.html), der tegner telefonen og værktøjslinjen.
//! - `content-<id>`: selve webappen, lagt præcis over telefonens skærm. Den har
//!   sin egen vedvarende data-mappe og ingen adgang til launcherens kommandoer.

pub mod actions;
mod clip;
pub mod device;
mod emulate;
mod status;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::webview::{Color, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Url, Webview, WebviewUrl, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

use crate::apps::AppEntry;
use crate::manifest::{self, Meta};
use crate::secrets;
use crate::store::{DeviceChoice, Store, WindowGeom};
use device::{Layout, Mode, DEVICES};

/// Mockup med ramme og værktøjslinje kræver frit placerbare børne-webviews.
/// Det understøtter Tauri på Windows og macOS; på Linux stables de i en GTK-box,
/// så dér åbnes appen i et almindeligt vindue med enhedens viewport.
const MOCKUP: bool = cfg!(not(target_os = "linux"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    Production,
    Preview,
}

/// Tilstand for ét åbent app-vindue.
pub struct WinCtx {
    entry: AppEntry,
    nav: NavPolicy,
    deployment: Mutex<Deployment>,
    /// (indeks i DEVICES, landskab)
    device: Mutex<(usize, bool)>,
    /// Appen bruger iOS-statuslinjen `black-translucent` og fylder hele skærmen.
    fullscreen: AtomicBool,
    /// Seneste status fra siden (til værktøjslinjen og vinduestitlen).
    status: Mutex<status::Snapshot>,
    /// Tæller til at finde ud af, hvornår brugeren er færdig med at trække i vinduet.
    resize_gen: AtomicU64,
    window_label: String,
    frame_label: String,
    content_label: String,
    /// Statuslinjens og bundens farve (appens theme_color/background_color).
    colors: AppColors,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppColors {
    status_bar: String,
    background: String,
}

impl AppColors {
    fn from_meta(meta: Option<&Meta>) -> Self {
        let valid = |c: &Option<String>| {
            c.as_deref()
                .filter(|c| manifest::parse_hex_color(c).is_some())
                .map(str::to_string)
        };
        let background = meta
            .and_then(|m| valid(&m.background_color))
            .unwrap_or_else(|| "#ffffff".into());
        let status_bar = meta
            .and_then(|m| valid(&m.theme_color))
            .unwrap_or_else(|| background.clone());
        Self {
            status_bar,
            background,
        }
    }
}

impl WinCtx {
    fn deployment(&self) -> Deployment {
        *self.deployment.lock().unwrap()
    }

    fn device(&self) -> (&'static device::Device, bool) {
        let (i, landscape) = *self.device.lock().unwrap();
        (&DEVICES[i], landscape)
    }

    fn mode(&self) -> Mode {
        Mode {
            overlay: clip::SUPPORTED,
            fullscreen: self.fullscreen.load(Ordering::SeqCst),
        }
    }

    fn layout(&self, win_w: f64, win_h: f64) -> Layout {
        let (dev, landscape) = self.device();
        dev.layout(landscape, win_w, win_h, self.mode())
    }

    /// Det, appens side skal vide om enheden lige nu.
    fn page_config(&self) -> emulate::PageConfig {
        let (dev, landscape) = self.device();
        emulate::PageConfig::new(dev, landscape, self.mode(), &self.colors.background)
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

    /// Adressen uden bypass-token (til "åbn i browser").
    fn public_url(&self) -> Url {
        match (self.deployment(), &self.entry.preview) {
            (Deployment::Preview, Some(preview)) => preview.clone(),
            _ => self.entry.url.clone(),
        }
    }

    fn content(&self, app: &AppHandle) -> Option<Webview> {
        app.get_webview(&self.content_label)
    }

    fn frame(&self, app: &AppHandle) -> Option<Webview> {
        app.get_webview(&self.frame_label)
    }

    fn window(&self, app: &AppHandle) -> Option<Window> {
        app.get_window(&self.window_label)
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
    pub fn get(&self, label: &str) -> Option<Arc<WinCtx>> {
        self.0.lock().unwrap().get(label).cloned()
    }
}

/// Åbner appen i sit eget mockup-vindue, eller fokuserer det, hvis det er åbent.
///
/// Må ikke kaldes på hovedtråden (`add_child` venter på den) – kald den fra en
/// async kommando.
pub fn open(app: &AppHandle, spec: &AppEntry, meta: Option<&Meta>) -> tauri::Result<()> {
    let label = format!("app-{}", spec.id);
    if let Some(win) = app.get_window(&label) {
        win.unminimize()?;
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }

    let store = app.state::<Store>();
    let choice = store.read(|s| s.devices.get(&spec.id).cloned());
    let (dev_idx, landscape) = choice
        .map(|c| (device::index_of(&c.device), c.landscape))
        .unwrap_or((0, false));
    let dev = &DEVICES[dev_idx];

    let mut allowed = vec![&spec.url];
    allowed.extend(spec.preview.as_ref());
    let ctx = Arc::new(WinCtx {
        entry: spec.clone(),
        nav: NavPolicy::new(&allowed),
        deployment: Mutex::new(Deployment::Production),
        device: Mutex::new((dev_idx, landscape)),
        fullscreen: AtomicBool::new(
            store.read(|s| s.fullscreen.get(&spec.id).copied().unwrap_or(false)),
        ),
        status: Mutex::new(status::Snapshot::default()),
        resize_gen: AtomicU64::new(0),
        window_label: label.clone(),
        frame_label: format!("frame-{}", spec.id),
        content_label: format!("content-{}", spec.id),
        colors: AppColors::from_meta(meta),
    });
    // Registrér før webviews oprettes: rammen spørger efter sit layout, så snart
    // den er indlæst (før blev den først tegnet ved næste størrelsesændring).
    app.state::<Windows>()
        .0
        .lock()
        .unwrap()
        .insert(label.clone(), ctx.clone());

    if !MOCKUP {
        return open_plain(app, &ctx, dev, landscape, meta);
    }

    // Størrelse: husket (hvis den stadig er på en skærm) eller så stor som skærmen tillader.
    let saved = store
        .read(|s| s.windows.get(&spec.id).copied())
        .filter(|g| on_some_monitor(app, g));
    let (min, max) = dev.size_limits(landscape);
    let (win_w, win_h) = match saved {
        Some(g) => (g.width.clamp(min.0, max.0), g.height.clamp(min.1, max.1)),
        None => {
            let (sw, sh) = primary_screen(app);
            dev.window_size(landscape, dev.fit_scale(landscape, sw, sh))
        }
    };

    let mut wb = WindowBuilder::new(app, &label)
        .title(&spec.name)
        .decorations(false)
        .resizable(true)
        .inner_size(win_w, win_h)
        .min_inner_size(min.0, min.1)
        .max_inner_size(max.0, max.1);
    #[cfg(not(target_os = "macos"))]
    {
        wb = wb.transparent(true).shadow(false);
    }
    wb = match saved {
        Some(g) => wb.position(g.x, g.y),
        None => wb.center(),
    };
    let window = wb.build()?;

    let layout = ctx.layout(win_w, win_h);

    // Rammen først (nederst) – appen lægges ovenpå.
    let frame_url = WebviewUrl::App(format!("frame.html?label={label}").into());
    let mut frame = WebviewBuilder::new(&ctx.frame_label, frame_url).auto_resize();
    #[cfg(not(target_os = "macos"))]
    {
        frame = frame.transparent(true);
    }
    #[cfg(target_os = "macos")]
    {
        frame = frame.background_color(Color(24, 24, 27, 255));
    }
    window.add_child(
        frame,
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(win_w, win_h),
    )?;

    let content = window.add_child(
        content_builder(app, &ctx, meta)?,
        LogicalPosition::new(layout.content.x, layout.content.y),
        LogicalSize::new(layout.content.w, layout.content.h),
    )?;
    let _ = content.set_zoom(layout.scale);
    start_content(&content, &ctx, dev, &layout);

    attach_events(app, &ctx, &window);
    // Sikr at rammen får layoutet, også hvis den nåede at starte før appen.
    relayout(app, &ctx, win_w, win_h);
    Ok(())
}

/// Linux: almindeligt vindue med kun appen, i enhedens viewport (zoomet til skærmen).
fn open_plain(
    app: &AppHandle,
    ctx: &Arc<WinCtx>,
    dev: &device::Device,
    landscape: bool,
    meta: Option<&Meta>,
) -> tauri::Result<()> {
    let (sw, sh) = primary_screen(app);
    let scale = dev.fit_scale(landscape, sw, sh);
    let (vw, vh) = dev.viewport(landscape, Mode::default());
    let (w, h) = (vw * scale, vh * scale);
    let window = WindowBuilder::new(app, &ctx.window_label)
        .title(&ctx.entry.name)
        .inner_size(w, h)
        .center()
        .build()?;
    let content = window.add_child(
        content_builder(app, ctx, meta)?.auto_resize(),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(w, h),
    )?;
    let _ = content.set_zoom(scale);
    let layout = dev.layout(landscape, w, h, Mode::default());
    start_content(&content, ctx, dev, &layout);
    attach_events(app, ctx, &window);
    Ok(())
}

/// Windows: webviewet oprettes med about:blank, så enhedens user agent, touch og
/// safe-area er sat, før appen indlæses. Andre platforme starter direkte på appen.
fn start_content(content: &Webview, ctx: &WinCtx, dev: &device::Device, layout: &Layout) {
    emulate::apply(content, dev, layout);
    if cfg!(windows) {
        let _ = content.navigate(ctx.start_url());
    }
}

fn attach_events(app: &AppHandle, ctx: &Arc<WinCtx>, window: &Window) {
    let handle = app.clone();
    let ctx_ev = ctx.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            let Some(win) = ctx_ev.window(&handle) else {
                return;
            };
            let scale = win.scale_factor().unwrap_or(1.0);
            let size = size.to_logical::<f64>(scale);
            relayout(&handle, &ctx_ev, size.width, size.height);
            snap_when_idle(&handle, &ctx_ev);
        }
        WindowEvent::ScaleFactorChanged { .. } => {
            if let Some((w, h)) = ctx_ev.window(&handle).and_then(|w| logical_inner_size(&w)) {
                relayout(&handle, &ctx_ev, w, h);
            }
        }
        WindowEvent::CloseRequested { .. } => {
            if let Some(win) = ctx_ev.window(&handle) {
                save_geometry(&handle, &ctx_ev, &win);
            }
        }
        WindowEvent::Destroyed => {
            handle
                .state::<Windows>()
                .0
                .lock()
                .unwrap()
                .remove(&ctx_ev.window_label);
        }
        _ => {}
    });

    status::spawn_poller(app.clone(), ctx.window_label.clone());
}

fn content_builder(
    app: &AppHandle,
    ctx: &Arc<WinCtx>,
    meta: Option<&Meta>,
) -> tauri::Result<WebviewBuilder<tauri::Wry>> {
    let first_url = if cfg!(windows) {
        Url::parse("about:blank").expect("gyldig adresse")
    } else {
        ctx.start_url()
    };
    let (dev, _) = ctx.device();
    let mut b = WebviewBuilder::new(&ctx.content_label, WebviewUrl::External(first_url))
        .zoom_hotkeys_enabled(false)
        .initialization_script(emulate::init_script(ctx.page_config()))
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
            move |url, _features| {
                if ctx.nav.allows(&url) {
                    if let Some(content) = ctx.content(&app) {
                        let _ = content.navigate(url);
                    }
                } else {
                    open_external(&app, &url);
                }
                NewWindowResponse::Deny
            }
        })
        .on_page_load({
            let ctx = ctx.clone();
            move |webview, payload| {
                // about:blank (start og "ryd data") tæller ikke som appens første side.
                if payload.event() == PageLoadEvent::Finished
                    && matches!(payload.url().scheme(), "http" | "https")
                {
                    ctx.nav.mark_loaded();
                    let _ = webview.eval(emulate::configure_js(ctx.page_config()));
                }
            }
        });

    // Uden DevTools-protokollen (macOS/Linux) sættes user agent ved oprettelsen.
    if !cfg!(windows) {
        if let Some(ua) = emulate::profile(dev).user_agent {
            b = b.user_agent(ua);
        }
    }

    if let Some((r, g, b_)) = meta
        .and_then(|m| m.background_color.as_deref())
        .and_then(manifest::parse_hex_color)
    {
        b = b.background_color(Color(r, g, b_, 255));
    }

    // Hver app får sin egen storage, cookies, cache og service worker,
    // som overlever genstart. macOS har ingen data-mappe, men en data store-id (macOS 14+).
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let dir = app
            .path()
            .app_data_dir()?
            .join("webdata")
            .join(&ctx.entry.id);
        b = b.data_directory(dir);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        b = b.data_store_identifier(store_identifier(&ctx.entry.id));
    }
    Ok(b)
}

/// Lægger appens webview over skærmen og fortæller rammen om det nye layout.
fn relayout(app: &AppHandle, ctx: &WinCtx, win_w: f64, win_h: f64) -> Layout {
    let layout = ctx.layout(win_w, win_h);
    if !MOCKUP {
        // Appen fylder vinduet (auto_resize); hold viewport-bredden.
        if let Some(content) = ctx.content(app) {
            let _ = content.set_zoom(win_w / layout.viewport.0);
        }
        return layout;
    }
    if let Some(content) = ctx.content(app) {
        let _ = content.set_position(LogicalPosition::new(layout.content.x, layout.content.y));
        let _ = content.set_size(LogicalSize::new(layout.content.w, layout.content.h));
        let _ = content.set_zoom(layout.scale);
        let scale_factor = ctx
            .window(app)
            .and_then(|w| w.scale_factor().ok())
            .unwrap_or(1.0);
        clip::apply(&content, &layout, scale_factor);
    }
    push_to_frame(app, ctx, "layout", &layout);
    layout
}

/// Når brugeren har sluppet vindueskanten, tilpasses vinduet til rammen,
/// så der ikke er tom plads rundt om telefonen.
fn snap_when_idle(app: &AppHandle, ctx: &Arc<WinCtx>) {
    if !MOCKUP {
        return;
    }
    let gen = ctx.resize_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        if ctx.resize_gen.load(Ordering::SeqCst) != gen {
            return;
        }
        let Some(win) = ctx.window(&app) else { return };
        if win.is_maximized().unwrap_or(false) {
            let _ = win.unmaximize();
        }
        let Some((w, h)) = logical_inner_size(&win) else {
            return;
        };
        let (dev, landscape) = ctx.device();
        let layout = ctx.layout(w, h);
        let (tw, th) = dev.window_size(landscape, layout.scale);
        if (tw - w).abs() > 1.5 || (th - h).abs() > 1.5 {
            let _ = win.set_size(LogicalSize::new(tw, th));
        }
    });
}

/// Skifter enhed og/eller retning, men bevarer skalaen.
fn set_device(app: &AppHandle, ctx: &Arc<WinCtx>, idx: usize, landscape: bool) {
    let Some(win) = ctx.window(app) else { return };
    let old_scale = logical_inner_size(&win)
        .map(|(w, h)| ctx.layout(w, h).scale)
        .unwrap_or(1.0);

    let dev = &DEVICES[idx];
    let landscape = landscape && dev.can_rotate;
    *ctx.device.lock().unwrap() = (idx, landscape);
    app.state::<Store>().update(|s| {
        s.devices.insert(
            ctx.entry.id.clone(),
            DeviceChoice {
                device: dev.id.to_string(),
                landscape,
            },
        )
    });

    // Samme skala, men aldrig større end skærmen.
    let (sw, sh) = primary_screen(app);
    let scale = old_scale.min(dev.fit_scale(landscape, sw, sh));
    let (min, max) = dev.size_limits(landscape);
    let (w, h) = dev.window_size(landscape, scale);
    // Først grænserne vide nok, så den nye størrelse er tilladt.
    let _ = win.set_min_size(Some(LogicalSize::new(min.0.min(w), min.1.min(h))));
    let _ = win.set_max_size(Some(LogicalSize::new(max.0.max(w), max.1.max(h))));
    let _ = win.set_size(LogicalSize::new(w, h));
    let _ = win.set_min_size(Some(LogicalSize::new(min.0, min.1)));
    let _ = win.set_max_size(Some(LogicalSize::new(max.0, max.1)));
    let layout = relayout(app, ctx, w, h);
    emulate_device(app, ctx, &layout);
}

/// Appen har skiftet statuslinje-stil: fuld skærm til/fra.
pub(super) fn set_fullscreen(app: &AppHandle, ctx: &WinCtx, on: bool) {
    if ctx.fullscreen.swap(on, Ordering::SeqCst) == on {
        return;
    }
    app.state::<Store>().update(|s| {
        s.fullscreen.insert(ctx.entry.id.clone(), on);
    });
    let Some((w, h)) = ctx.window(app).and_then(|w| logical_inner_size(&w)) else {
        return;
    };
    let layout = relayout(app, ctx, w, h);
    emulate_device(app, ctx, &layout);
}

/// Sender enhedens egenskaber og statuslinjen til appen.
fn emulate_device(app: &AppHandle, ctx: &WinCtx, layout: &Layout) {
    let Some(content) = ctx.content(app) else {
        return;
    };
    let (dev, _) = ctx.device();
    emulate::apply(&content, dev, layout);
    let _ = content.eval(emulate::configure_js(ctx.page_config()));
}

/// Skifter mellem production og preview og indlæser den nye adresse.
fn switch_deployment(app: &AppHandle, ctx: &WinCtx, to: Deployment) {
    if to == Deployment::Preview && ctx.entry.preview.is_none() {
        return;
    }
    *ctx.deployment.lock().unwrap() = to;
    // Tillad Vercels login-redirect m.m. igen indtil den nye side er indlæst.
    ctx.nav.reset_loaded();
    status::reset(app, ctx);
    if let Some(content) = ctx.content(app) {
        let _ = content.navigate(ctx.start_url());
    }
}

/// Sender data til rammen (ui/frame.js: `window.__frame(kind, data)`).
/// Bruger eval frem for events, så rammen ikke behøver flere rettigheder.
fn push_to_frame<T: Serialize>(app: &AppHandle, ctx: &WinCtx, kind: &str, data: &T) {
    let (Some(frame), Ok(json)) = (ctx.frame(app), serde_json::to_string(data)) else {
        return;
    };
    let _ = frame.eval(format!(
        "window.__frame && window.__frame({}, {json})",
        serde_json::Value::from(kind)
    ));
}

/// Det rammen får at vide, når den starter.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInit {
    name: String,
    devices: Vec<FrameDevice>,
    has_preview: bool,
    deployment: Deployment,
    layout: Layout,
    status: status::Snapshot,
    colors: AppColors,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameDevice {
    id: &'static str,
    name: &'static str,
    can_rotate: bool,
}

pub fn frame_init(app: &AppHandle, label: &str) -> Option<FrameInit> {
    let ctx = app.state::<Windows>().get(label)?;
    let win = ctx.window(app)?;
    let (w, h) = logical_inner_size(&win)?;
    let status = ctx.status.lock().unwrap().clone();
    Some(FrameInit {
        name: ctx.entry.name.clone(),
        devices: DEVICES
            .iter()
            .map(|d| FrameDevice {
                id: d.id,
                name: d.name,
                can_rotate: d.can_rotate,
            })
            .collect(),
        has_preview: ctx.entry.preview.is_some(),
        deployment: ctx.deployment(),
        layout: ctx.layout(w, h),
        status,
        colors: ctx.colors.clone(),
    })
}

fn logical_inner_size(win: &Window) -> Option<(f64, f64)> {
    let scale = win.scale_factor().ok()?;
    let size = win.inner_size().ok()?.to_logical::<f64>(scale);
    Some((size.width, size.height))
}

fn save_geometry(app: &AppHandle, ctx: &WinCtx, win: &Window) {
    if win.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(scale), Ok(pos)) = (win.scale_factor(), win.outer_position()) else {
        return;
    };
    let Some((width, height)) = logical_inner_size(win) else {
        return;
    };
    let pos = pos.to_logical::<f64>(scale);
    let geom = WindowGeom {
        x: pos.x,
        y: pos.y,
        width,
        height,
    };
    app.state::<Store>()
        .update(|s| s.windows.insert(ctx.entry.id.clone(), geom));
}

/// Primærskærmens logiske størrelse (1920×1080, hvis den ikke kan aflæses).
fn primary_screen(app: &AppHandle) -> (f64, f64) {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let size = m.size().to_logical::<f64>(m.scale_factor());
            (size.width, size.height)
        })
        .unwrap_or((1920.0, 1080.0))
}

fn on_some_monitor(app: &AppHandle, g: &WindowGeom) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return true;
    };
    // Værktøjslinjens venstre del skal kunne gribes.
    let (px, py) = (g.x + 40.0, g.y + 20.0);
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
