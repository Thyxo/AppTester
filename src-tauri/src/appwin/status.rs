//! Status for siden: deployment, service worker og online/offline.
//!
//! Status læses med `eval_with_callback`, så webappen ikke behøver adgang til IPC,
//! og vises i rammens værktøjslinje og i vinduestitlen (proceslinjen).

use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{push_to_frame, set_fullscreen, Deployment, WinCtx, Windows};

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
  var sbs = null;
  if (/^https?:$/.test(location.protocol) && document.readyState !== "loading") {
    var m = document.querySelector('meta[name="apple-mobile-web-app-status-bar-style"]');
    sbs = m ? String(m.content).trim().toLowerCase() : "";
  }
  return JSON.stringify({ reg: s.reg, ctl: !!(sw && sw.controller), online: navigator.onLine, sbs: sbs });
})()"#;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PageStatus {
    /// En service worker er registreret for siden.
    reg: bool,
    /// Siden styres af en service worker (kan køre offline).
    ctl: bool,
    online: bool,
    /// `apple-mobile-web-app-status-bar-style` ("" = ingen; `None` = siden er ikke klar).
    #[serde(default)]
    sbs: Option<String>,
}

impl PageStatus {
    /// iOS lægger appen under statuslinjen (fuld skærm) med `black-translucent`.
    fn translucent(&self) -> Option<bool> {
        self.sbs.as_deref().map(|s| s == "black-translucent")
    }
}

/// Det, værktøjslinjen viser.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub deployment: Deployment,
    /// "ok" (styret af SW), "registered", "none" eller "unknown" (ikke målt endnu).
    pub sw: &'static str,
    /// "online", "offline-cache", "offline" eller "unknown".
    pub net: &'static str,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            deployment: Deployment::Production,
            sw: "unknown",
            net: "unknown",
        }
    }
}

impl Snapshot {
    fn from_page(deployment: Deployment, st: &PageStatus) -> Self {
        Self {
            deployment,
            sw: match (st.ctl, st.reg) {
                (true, _) => "ok",
                (false, true) => "registered",
                (false, false) => "none",
            },
            net: match (st.online, st.ctl) {
                (true, _) => "online",
                (false, true) => "offline-cache",
                (false, false) => "offline",
            },
        }
    }

    fn title(&self, name: &str) -> String {
        let dep = match self.deployment {
            Deployment::Production => "production",
            Deployment::Preview => "preview",
        };
        let sw = match self.sw {
            "ok" => " · SW ✓",
            "registered" => " · SW registreret",
            "none" => " · ingen SW",
            _ => "",
        };
        let net = match self.net {
            "online" => " · online",
            "offline-cache" => " · offline – kører fra cache",
            "offline" => " · offline",
            _ => "",
        };
        format!("{name} · {dep}{sw}{net}")
    }
}

fn publish(app: &AppHandle, ctx: &WinCtx, snap: Snapshot) {
    {
        let mut current = ctx.status.lock().unwrap();
        if *current == snap {
            return;
        }
        *current = snap.clone();
    }
    if let Some(win) = ctx.window(app) {
        let _ = win.set_title(&snap.title(&ctx.entry.name));
    }
    push_to_frame(app, ctx, "status", &snap);
}

/// Nulstiller status (fx efter skift af deployment eller rydning af data).
pub fn reset(app: &AppHandle, ctx: &WinCtx) {
    publish(
        app,
        ctx,
        Snapshot {
            deployment: ctx.deployment(),
            ..Snapshot::default()
        },
    );
}

/// Spørger siden om status med fast interval, så længe vinduet findes.
pub fn spawn_poller(app: AppHandle, label: String) {
    thread::spawn(move || loop {
        thread::sleep(POLL_INTERVAL);
        let Some(ctx) = app.state::<Windows>().get(&label) else {
            break;
        };
        let Some(content) = ctx.content(&app) else {
            break;
        };
        let handle = app.clone();
        let _ = content.eval_with_callback(STATUS_JS, move |raw| {
            if let Some(st) = parse(&raw) {
                if let Some(on) = st.translucent() {
                    set_fullscreen(&handle, &ctx, on);
                }
                publish(&handle, &ctx, Snapshot::from_page(ctx.deployment(), &st));
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
            sbs: None,
        };
        assert_eq!(
            parse(r#"{"reg":true,"ctl":false,"online":true}"#),
            Some(expected.clone())
        );
        assert_eq!(
            parse(r#""{\"reg\":true,\"ctl\":false,\"online\":true,\"sbs\":null}""#),
            Some(expected)
        );
        let st = parse(r#"{"reg":true,"ctl":false,"online":true,"sbs":"black-translucent"}"#);
        assert_eq!(st.unwrap().translucent(), Some(true));
        let st = parse(r#"{"reg":true,"ctl":false,"online":true,"sbs":""}"#);
        assert_eq!(st.unwrap().translucent(), Some(false));
        assert_eq!(parse("null"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn titles() {
        let snap = Snapshot::from_page(
            Deployment::Preview,
            &PageStatus {
                reg: true,
                ctl: true,
                online: false,
                sbs: None,
            },
        );
        assert_eq!(
            snap.title("Monizzz"),
            "Monizzz · preview · SW ✓ · offline – kører fra cache"
        );
        assert_eq!(Snapshot::default().title("Monizzz"), "Monizzz · production");
    }
}
