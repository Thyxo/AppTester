//! Statuslinje i vinduestitlen: deployment, service worker og online/offline.
//!
//! Status læses med `eval_with_callback`, så webappen ikke behøver adgang til IPC.

use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewWindow};

use super::{WinCtx, Windows};

const POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// Returnerer sidste kendte status og opdaterer den asynkront til næste gang
/// (getRegistration er async). Returnerer en streng, fordi platformene
/// serialiserer objekter forskelligt.
const STATUS_JS: &str = r#"(function () {
  var s = window.__pwaLauncherStatus || (window.__pwaLauncherStatus = { reg: false });
  var sw = navigator.serviceWorker;
  if (sw && sw.getRegistration) {
    sw.getRegistration().then(function (r) { s.reg = !!r; }, function () {});
  }
  return JSON.stringify({ reg: s.reg, ctl: !!(sw && sw.controller), online: navigator.onLine });
})()"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct PageStatus {
    /// En service worker er registreret for siden.
    reg: bool,
    /// Siden styres af en service worker (kan køre offline).
    ctl: bool,
    online: bool,
}

pub fn base_title(ctx: &WinCtx) -> String {
    format!("{} · {}", ctx.entry.name, ctx.deployment().label())
}

fn full_title(ctx: &WinCtx, st: PageStatus) -> String {
    let sw = match (st.ctl, st.reg) {
        (true, _) => "SW ✓",
        (false, true) => "SW registreret",
        (false, false) => "ingen SW",
    };
    let net = match (st.online, st.ctl) {
        (true, _) => "online",
        (false, true) => "offline – kører fra cache",
        (false, false) => "offline",
    };
    format!("{} · {sw} · {net}", base_title(ctx))
}

pub fn reset_title(win: &WebviewWindow, ctx: &WinCtx) {
    set_title(win, ctx, base_title(ctx));
}

fn set_title(win: &WebviewWindow, ctx: &WinCtx, title: String) {
    let mut current = ctx.title.lock().unwrap();
    if *current != title {
        let _ = win.set_title(&title);
        *current = title;
    }
}

/// Spørger siden om status med fast interval, så længe vinduet findes.
pub fn spawn_poller(app: AppHandle, label: String) {
    thread::spawn(move || loop {
        thread::sleep(POLL_INTERVAL);
        let (Some(win), Some(ctx)) = (
            app.get_webview_window(&label),
            app.state::<Windows>().get(&label),
        ) else {
            break;
        };
        let target = win.clone();
        let _ = win.eval_with_callback(STATUS_JS, move |raw| {
            if let Some(st) = parse(&raw) {
                set_title(&target, &ctx, full_title(&ctx, st));
            }
        });
    });
}

fn parse(raw: &str) -> Option<PageStatus> {
    parse_eval(raw)
}

/// Fortolker resultatet af `eval_with_callback` for et script, der returnerer
/// `JSON.stringify(...)`. Windows leverer det JSON-kodet (en JSON-streng);
/// andre platforme leverer teksten rå.
pub(super) fn parse_eval<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
    match serde_json::from_str::<serde_json::Value>(raw).ok()? {
        serde_json::Value::String(inner) => serde_json::from_str(&inner).ok(),
        other => serde_json::from_value(other).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_encodings() {
        let expected = PageStatus {
            reg: true,
            ctl: false,
            online: true,
        };
        assert_eq!(
            parse(r#"{"reg":true,"ctl":false,"online":true}"#),
            Some(expected)
        );
        assert_eq!(
            parse(r#""{\"reg\":true,\"ctl\":false,\"online\":true}""#),
            Some(expected)
        );
        assert_eq!(parse("null"), None);
        assert_eq!(parse(""), None);
    }
}
