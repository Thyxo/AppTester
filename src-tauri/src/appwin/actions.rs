//! Testhandlinger, som knapperne i mockup-rammens værktøjslinje udløser.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, Webview};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::{
    device, open_external, set_device, status, switch_deployment, Deployment, WinCtx, Windows,
};

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    DevTools,
    Reload,
    HardReload,
    ClearData,
    Device(usize),
    Rotate,
    Deploy(Deployment),
    OpenInBrowser,
}

impl Action {
    #[cfg(test)]
    fn id(self) -> String {
        match self {
            Action::DevTools => "devtools".into(),
            Action::Reload => "reload".into(),
            Action::HardReload => "hard-reload".into(),
            Action::ClearData => "clear-data".into(),
            Action::Device(i) => format!("device:{}", device::DEVICES[i].id),
            Action::Rotate => "rotate".into(),
            Action::Deploy(Deployment::Production) => "deploy:production".into(),
            Action::Deploy(Deployment::Preview) => "deploy:preview".into(),
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
            "deploy:production" => Action::Deploy(Deployment::Production),
            "deploy:preview" => Action::Deploy(Deployment::Preview),
            "open-in-browser" => Action::OpenInBrowser,
            _ => {
                let id = s.strip_prefix("device:")?;
                Action::Device(device::DEVICES.iter().position(|d| d.id == id)?)
            }
        })
    }
}

/// Udfører en handling for app-vinduet `label` (kaldt fra rammens værktøjslinje).
pub fn run(app: &AppHandle, label: &str, action: &str) -> Result<(), String> {
    let action = Action::parse(action).ok_or_else(|| format!("Ukendt handling: {action}"))?;
    let ctx = app
        .state::<Windows>()
        .get(label)
        .ok_or("Vinduet er lukket")?;
    let content = ctx.content(app).ok_or("Appen er ikke indlæst")?;

    match action {
        Action::DevTools => {
            if content.is_devtools_open() {
                content.close_devtools();
            } else {
                content.open_devtools();
            }
        }
        Action::Reload => {
            let _ = content.reload();
        }
        Action::HardReload => {
            let _ = content.eval(HARD_RELOAD_JS);
        }
        Action::ClearData => confirm_clear(app, ctx, content),
        Action::Device(i) => {
            let (_, landscape) = ctx.device();
            set_device(app, &ctx, i, landscape);
        }
        Action::Rotate => {
            let (dev, landscape) = ctx.device();
            set_device(app, &ctx, device::index_of(dev.id), !landscape);
        }
        Action::Deploy(to) => switch_deployment(app, &ctx, to),
        Action::OpenInBrowser => open_external(app, &ctx.public_url()),
    }
    Ok(())
}

fn confirm_clear(app: &AppHandle, ctx: Arc<WinCtx>, content: Webview) {
    let mut dialog = app
        .dialog()
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
        ));
    if let Some(win) = ctx.window(app) {
        dialog = dialog.parent(&win);
    }
    let app = app.clone();
    dialog.show(move |confirmed| {
        if confirmed {
            thread::spawn(move || clear_data(&app, &ctx, &content));
        }
    });
}

/// Rydder appens data og starter den forfra.
///
/// Platformens rydning sker asynkront uden besked om, hvornår den er færdig.
/// Derfor ryddes den aktuelle origin først via JS, og webviewet står på
/// `about:blank`, mens platformen rydder resten – så appen ikke når at skrive
/// nye data, der bagefter forsvinder.
fn clear_data(app: &AppHandle, ctx: &WinCtx, content: &Webview) {
    let _ = content.eval(CLEAR_ORIGIN_JS);
    thread::sleep(Duration::from_millis(500));
    let _ = content.navigate("about:blank".parse().expect("gyldig URL"));
    thread::sleep(Duration::from_millis(300));
    if let Err(err) = content.clear_all_browsing_data() {
        eprintln!("Kunne ikke rydde data for {}: {err}", ctx.entry.name);
    }
    thread::sleep(Duration::from_millis(1200));
    ctx.nav.reset_loaded();
    status::reset(app, ctx);
    let _ = content.navigate(ctx.start_url());
}

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
            Action::Device(0),
            Action::Device(device::DEVICES.len() - 1),
            Action::Rotate,
            Action::Deploy(Deployment::Production),
            Action::Deploy(Deployment::Preview),
            Action::OpenInBrowser,
        ];
        for a in all {
            assert_eq!(Action::parse(&a.id()), Some(a));
        }
        assert_eq!(Action::parse("device:nokia-3310"), None);
        assert_eq!(Action::parse("ukendt"), None);
    }
}
