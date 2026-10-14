//! Test-menuen i app-vinduer.
//!
//! Menu-id'er har formen `<vindues-label>|<handling>`. På macOS deles én app-menu
//! af alle vinduer; der bruges label `*`, og handlingen rammer det fokuserede app-vindue.

use std::thread;
use std::time::Duration;

use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::{open_external, status, switch_deployment, viewport, Deployment, WinCtx, Windows};

/// Afregistrerer service workers og tømmer Cache Storage, så siden hentes friskt
/// fra netværket. localStorage, IndexedDB og cookies bevares.
const HARD_RELOAD_JS: &str = r#"(async function () {
  try {
    var sw = navigator.serviceWorker;
    if (sw && sw.getRegistrations) {
      var regs = await sw.getRegistrations();
      await Promise.all(regs.map(function (r) { return r.unregister(); }));
    }
    if (window.caches) {
      var keys = await caches.keys();
      await Promise.all(keys.map(function (k) { return caches.delete(k); }));
    }
  } catch (e) {}
  location.reload();
})();"#;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    DevTools,
    Reload,
    HardReload,
    ClearData,
    Viewport(usize),
    Rotate,
    Deploy(Deployment),
    OpenInBrowser,
}

impl Action {
    fn id(self) -> String {
        match self {
            Action::DevTools => "devtools".into(),
            Action::Reload => "reload".into(),
            Action::HardReload => "hard-reload".into(),
            Action::ClearData => "clear-data".into(),
            Action::Viewport(i) => format!("viewport-{i}"),
            Action::Rotate => "rotate".into(),
            Action::Deploy(Deployment::Production) => "deploy-production".into(),
            Action::Deploy(Deployment::Preview) => "deploy-preview".into(),
            Action::OpenInBrowser => "open-in-browser".into(),
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "devtools" => Action::DevTools,
            "reload" => Action::Reload,
            "hard-reload" => Action::HardReload,
            "clear-data" => Action::ClearData,
            "rotate" => Action::Rotate,
            "deploy-production" => Action::Deploy(Deployment::Production),
            "deploy-preview" => Action::Deploy(Deployment::Preview),
            "open-in-browser" => Action::OpenInBrowser,
            _ => {
                let i: usize = s.strip_prefix("viewport-")?.parse().ok()?;
                (i < viewport::PRESETS.len()).then_some(Action::Viewport(i))?
            }
        })
    }
}

/// Testmenu til ét vindue (Windows/Linux).
#[cfg(not(target_os = "macos"))]
pub fn window_menu(
    app: &AppHandle,
    label: &str,
    has_preview: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    Menu::with_items(app, &[&test_submenu(app, label, has_preview)?])
}

/// macOS: standardmenuen (Rediger m.m. – kræves for kopiér/indsæt) plus Test.
#[cfg(target_os = "macos")]
pub fn install_app_menu(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::default(app)?;
    menu.append(&test_submenu(app, "*", true)?)?;
    app.set_menu(menu)?;
    Ok(())
}

fn test_submenu(
    app: &AppHandle,
    label: &str,
    has_preview: bool,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let item = |action: Action, text: &str, accel: Option<&str>| {
        MenuItem::with_id(app, format!("{label}|{}", action.id()), text, true, accel)
    };
    let sep = || PredefinedMenuItem::separator(app);

    // WebView2 har selv F12 og Ctrl+R; der vises genvejen kun som tekst,
    // så de ikke udløses to gange.
    #[cfg(windows)]
    let (devtools, reload) = (
        item(Action::DevTools, "DevTools\tF12", None)?,
        item(Action::Reload, "Genindlæs\tCtrl+R", None)?,
    );
    #[cfg(not(windows))]
    let (devtools, reload) = (
        item(Action::DevTools, "DevTools", Some("F12"))?,
        item(Action::Reload, "Genindlæs", Some("CmdOrCtrl+R"))?,
    );

    let hard_reload = item(
        Action::HardReload,
        "Hard reload (ny service worker, tom cache)",
        Some("CmdOrCtrl+Shift+R"),
    )?;
    let clear = item(Action::ClearData, "Ryd alle data for appen…", None)?;

    let presets = viewport::PRESETS
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = format!("{}  ({}×{})", p.name, p.width, p.height);
            item(Action::Viewport(i), &text, None)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let rotate = item(Action::Rotate, "Vend (portræt/landskab)", None)?;
    let sep_vp = sep()?;
    let mut vp_items: Vec<&dyn IsMenuItem<tauri::Wry>> = presets
        .iter()
        .map(|p| p as &dyn IsMenuItem<tauri::Wry>)
        .collect();
    vp_items.extend([&sep_vp as &dyn IsMenuItem<tauri::Wry>, &rotate]);
    let viewports = Submenu::with_items(app, "Viewport", true, &vp_items)?;

    let prod = item(Action::Deploy(Deployment::Production), "Production", None)?;
    let preview = item(Action::Deploy(Deployment::Preview), "Preview", None)?;
    let deployments = Submenu::with_items(app, "Deployment", has_preview, &[&prod, &preview])?;

    let browser = item(Action::OpenInBrowser, "Åbn i standardbrowser", None)?;
    let (sep1, sep2, sep3) = (sep()?, sep()?, sep()?);

    Submenu::with_items(
        app,
        "Test",
        true,
        &[
            &devtools,
            &sep1,
            &reload,
            &hard_reload,
            &clear,
            &sep2,
            &viewports,
            &deployments,
            &sep3,
            &browser,
        ],
    )
}

/// Global menu-handler (registreres i `lib.rs`).
pub fn handle(app: &AppHandle, event: MenuEvent) {
    let Some((label, action)) = event.id().as_ref().split_once('|') else {
        return;
    };
    let Some(action) = Action::parse(action) else {
        return;
    };
    let win = if label == "*" {
        app.webview_windows()
            .into_values()
            .find(|w| w.label().starts_with("app-") && w.is_focused().unwrap_or(false))
    } else {
        app.get_webview_window(label)
    };
    let Some(win) = win else { return };
    let Some(ctx) = app.state::<Windows>().get(win.label()) else {
        return;
    };
    run(app, &win, &ctx, action);
}

fn run(app: &AppHandle, win: &WebviewWindow, ctx: &std::sync::Arc<WinCtx>, action: Action) {
    match action {
        Action::DevTools => {
            if win.is_devtools_open() {
                win.close_devtools();
            } else {
                win.open_devtools();
            }
        }
        Action::Reload => {
            let _ = win.reload();
        }
        Action::HardReload => {
            let _ = win.eval(HARD_RELOAD_JS);
        }
        Action::ClearData => confirm_clear(app, win, ctx),
        Action::Viewport(i) => viewport::apply(win, &viewport::PRESETS[i]),
        Action::Rotate => viewport::rotate(win),
        Action::Deploy(to) => switch_deployment(win, ctx, to),
        Action::OpenInBrowser => {
            // Start-URL'en (uden bypass-token) frem for den aktuelle side.
            let url = match ctx.deployment() {
                Deployment::Preview => ctx.entry.preview.clone(),
                Deployment::Production => None,
            }
            .unwrap_or_else(|| ctx.entry.url.clone());
            open_external(app, &url);
        }
    }
}

fn confirm_clear(app: &AppHandle, win: &WebviewWindow, ctx: &std::sync::Arc<WinCtx>) {
    let win = win.clone();
    let ctx = ctx.clone();
    app.dialog()
        .message(format!(
            "Slet alle data for {}?\n\nlocalStorage, IndexedDB, cookies, cache og service worker \
             fjernes. Appen starter forfra, som ved første besøg.",
            ctx.entry.name
        ))
        .title("Ryd data")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Ryd data".into(),
            "Annuller".into(),
        ))
        .parent(&win.clone())
        .show(move |confirmed| {
            if !confirmed {
                return;
            }
            thread::spawn(move || clear_data(&win, &ctx));
        });
}

/// Rydder appens data og starter den forfra.
///
/// Platformens rydning sker asynkront uden besked om, hvornår den er færdig.
/// Derfor ryddes den aktuelle origin først via JS, og vinduet står på
/// `about:blank`, mens platformen rydder resten – så appen ikke når at skrive
/// nye data, der bagefter forsvinder.
fn clear_data(win: &WebviewWindow, ctx: &WinCtx) {
    let _ = win.eval(CLEAR_ORIGIN_JS);
    thread::sleep(Duration::from_millis(500));
    let _ = win.navigate("about:blank".parse().expect("gyldig URL"));
    thread::sleep(Duration::from_millis(300));
    if let Err(err) = win.clear_all_browsing_data() {
        eprintln!("Kunne ikke rydde data for {}: {err}", ctx.entry.name);
    }
    thread::sleep(Duration::from_millis(1200));
    ctx.nav.reset_loaded();
    status::reset_title(win, ctx);
    let _ = win.navigate(ctx.start_url());
}

/// Rydder den aktuelle origin: storage, IndexedDB, Cache Storage, service workers
/// og cookies, som JS kan se.
const CLEAR_ORIGIN_JS: &str = r#"(async function () {
  try { localStorage.clear(); sessionStorage.clear(); } catch (e) {}
  try {
    document.cookie.split(";").forEach(function (c) {
      var name = c.split("=")[0].trim();
      if (name) document.cookie = name + "=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/";
    });
  } catch (e) {}
  try {
    if (indexedDB.databases) {
      var dbs = await indexedDB.databases();
      dbs.forEach(function (d) { if (d.name) indexedDB.deleteDatabase(d.name); });
    }
  } catch (e) {}
  try {
    if (window.caches) {
      var keys = await caches.keys();
      await Promise.all(keys.map(function (k) { return caches.delete(k); }));
    }
  } catch (e) {}
  try {
    var sw = navigator.serviceWorker;
    if (sw && sw.getRegistrations) {
      var regs = await sw.getRegistrations();
      await Promise.all(regs.map(function (r) { return r.unregister(); }));
    }
  } catch (e) {}
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_roundtrip() {
        let all = [
            Action::DevTools,
            Action::Reload,
            Action::HardReload,
            Action::ClearData,
            Action::Viewport(0),
            Action::Viewport(viewport::PRESETS.len() - 1),
            Action::Rotate,
            Action::Deploy(Deployment::Production),
            Action::Deploy(Deployment::Preview),
            Action::OpenInBrowser,
        ];
        for a in all {
            assert_eq!(Action::parse(&a.id()), Some(a));
        }
        assert_eq!(Action::parse("viewport-99"), None);
        assert_eq!(Action::parse("ukendt"), None);
    }
}
