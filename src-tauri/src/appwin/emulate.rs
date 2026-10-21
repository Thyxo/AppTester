//! Får appen til at opføre sig, som på enheden: iPhone-skrift, safe-area, touch,
//! user agent, skjulte rullebjælker og iOS-statuslinjen i fuld skærm.
//!
//! - `init_script` køres i appen før dens egne scripts (alle platforme).
//! - `apply` sætter resten via Chrome DevTools Protocol i WebView2 (kun Windows).
//!   Indstillingerne gælder for webviewet og overlever navigation.

use serde::Serialize;
use serde_json::json;
use tauri::Webview;

use super::device::{Device, Kind, Layout, Notch};

const INIT_JS: &str = include_str!("emulate.js");

/// Inter (OFL), afløser for SF Pro, som ikke findes uden for Apple.
#[cfg(not(target_os = "macos"))]
const FONT: &[u8] = include_bytes!("../../assets/inter-variable.woff2");

/// Safari på iOS 26 (iOS-versionen i user agent er frosset til 18_6).
const UA_IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1";
/// iPadOS-Safari udgiver sig for at være en Mac.
const UA_IPAD: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const UA_ANDROID: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36";

/// Hvad appen skal tro, den kører på.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// `None` = webviewets egen (desktop).
    pub user_agent: Option<&'static str>,
    pub platform: &'static str,
    /// Touch-skærm: musen sender touch-events, og rullebjælker skjules.
    pub touch: bool,
}

pub fn profile(dev: &Device) -> Profile {
    match (dev.kind, dev.ios) {
        (Kind::Phone, true) => Profile {
            user_agent: Some(UA_IPHONE),
            platform: "iPhone",
            touch: true,
        },
        (Kind::Tablet, true) => Profile {
            user_agent: Some(UA_IPAD),
            platform: "MacIntel",
            touch: true,
        },
        (Kind::Phone | Kind::Tablet, false) => Profile {
            user_agent: Some(UA_ANDROID),
            platform: "Linux armv81",
            touch: true,
        },
        (Kind::Desktop, _) => Profile {
            user_agent: None,
            platform: "",
            touch: false,
        },
    }
}

/// Statuslinjen, som appen selv skal vise (kun i fuld skærm; iPhone viser
/// ingen statuslinje i landskab).
pub fn statusbar_style(dev: &Device, landscape: bool, fullscreen: bool) -> Option<&'static str> {
    if !fullscreen {
        return None;
    }
    match (dev.kind, dev.notch, landscape) {
        (Kind::Tablet, _, _) => Some("tablet"),
        (Kind::Phone, Notch::Island, false) => Some("island"),
        (Kind::Phone, Notch::HomeButton, false) => Some("classic"),
        _ => None,
    }
}

/// Det, appens side får at vide (se `configure` i emulate.js).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageConfig {
    /// iOS-statuslinjen, som appen selv skal vise (fuld skærm).
    pub status_bar: Option<&'static str>,
    /// `screen.width/height` (enhedens skærm; `None` = den rigtige).
    pub screen: Option<(f64, f64)>,
}

impl PageConfig {
    pub fn new(dev: &Device, landscape: bool, fullscreen: bool) -> Self {
        let size = if landscape {
            (dev.screen_h, dev.screen_w)
        } else {
            (dev.screen_w, dev.screen_h)
        };
        let screen = (dev.kind != Kind::Desktop).then_some(size);
        Self {
            status_bar: statusbar_style(dev, landscape, fullscreen),
            screen,
        }
    }

    fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Script, der køres før sidens egne (ved hver sideindlæsning).
pub fn init_script(config: PageConfig) -> String {
    #[cfg(not(target_os = "macos"))]
    let font = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, FONT);
    // macOS har selv SF Pro (-apple-system).
    #[cfg(target_os = "macos")]
    let font = String::new();
    INIT_JS
        .replacen("__FONT__", &font, 1)
        .replacen("__CONFIG__", &config.json(), 1)
}

/// Opdaterer den side, der er indlæst nu.
pub fn configure_js(config: PageConfig) -> String {
    format!(
        "window.__pwaLauncher && window.__pwaLauncher.configure({})",
        config.json()
    )
}

/// DevTools-kommandoer for enheden og layoutet (metode, parametre).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn cdp_calls(dev: &Device, layout: &Layout) -> Vec<(&'static str, serde_json::Value)> {
    let p = profile(dev);
    let safe = layout.safe_area;
    let mut ua = json!({
        // Tom streng = webviewets egen user agent.
        "userAgent": p.user_agent.unwrap_or(""),
        "platform": p.platform,
    });
    if p.user_agent == Some(UA_ANDROID) {
        ua["userAgentMetadata"] = json!({
            "brands": [{ "brand": "Chromium", "version": "141" }, { "brand": "Google Chrome", "version": "141" }],
            "platform": "Android",
            "platformVersion": "14.0.0",
            "architecture": "",
            "model": dev.name,
            "mobile": true,
        });
    }
    vec![
        ("Emulation.setUserAgentOverride", ua),
        (
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": p.touch, "maxTouchPoints": if p.touch { 5 } else { 1 } }),
        ),
        (
            "Emulation.setEmitTouchEventsForMouse",
            json!({ "enabled": p.touch, "configuration": "mobile" }),
        ),
        (
            "Emulation.setScrollbarsHidden",
            json!({ "hidden": p.touch }),
        ),
        (
            "Emulation.setSafeAreaInsetsOverride",
            json!({ "insets": {
                "top": safe.top.round() as i64,
                "right": safe.right.round() as i64,
                "bottom": safe.bottom.round() as i64,
                "left": safe.left.round() as i64,
            } }),
        ),
    ]
}

/// Sætter enhedens egenskaber i appens webview.
#[cfg(windows)]
pub fn apply(content: &Webview, dev: &Device, layout: &Layout) {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;

    let calls: Vec<(HSTRING, HSTRING)> = cdp_calls(dev, layout)
        .into_iter()
        .map(|(m, p)| (HSTRING::from(m), HSTRING::from(p.to_string())))
        .collect();
    let _ = content.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        for (method, params) in &calls {
            let name = method.to_string();
            let handler =
                CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |res, _json| {
                    if let Err(err) = res {
                        // Ældre WebView2 kender ikke alle kommandoer – resten virker stadig.
                        eprintln!("DevTools {name}: {err}");
                    }
                    Ok(())
                }));
            let _ = core.CallDevToolsProtocolMethod(method, params, &handler);
        }
    });
}

#[cfg(not(windows))]
pub fn apply(_content: &Webview, _dev: &Device, _layout: &Layout) {}

#[cfg(test)]
mod tests {
    use super::super::device::{index_of, Mode, DEVICES};
    use super::*;

    #[test]
    fn profiles() {
        let iphone = &DEVICES[index_of("iphone-15-pro")];
        assert!(profile(iphone).user_agent.unwrap().contains("iPhone"));
        assert!(profile(iphone).touch);
        let desktop = &DEVICES[index_of("desktop")];
        assert_eq!(profile(desktop).user_agent, None);
        assert!(!profile(desktop).touch);
        let pixel = &DEVICES[index_of("pixel-8")];
        assert!(profile(pixel).user_agent.unwrap().contains("Android"));
    }

    #[test]
    fn statusbar_only_in_fullscreen_portrait_phone() {
        let iphone = &DEVICES[index_of("iphone-15-pro")];
        assert_eq!(statusbar_style(iphone, false, true), Some("island"));
        assert_eq!(statusbar_style(iphone, true, true), None);
        assert_eq!(statusbar_style(iphone, false, false), None);
        let se = &DEVICES[index_of("iphone-se")];
        assert_eq!(statusbar_style(se, false, true), Some("classic"));
        let ipad = &DEVICES[index_of("ipad-air")];
        assert_eq!(statusbar_style(ipad, true, true), Some("tablet"));
    }

    #[test]
    fn init_script_is_filled_in() {
        let iphone = &DEVICES[index_of("iphone-15-pro")];
        let js = init_script(PageConfig::new(iphone, false, true));
        assert!(!js.contains("__FONT__") && !js.contains("__CONFIG__"));
        assert!(js.contains(r#"var config = {"statusBar":"island","screen":[393.0,852.0]};"#));
        let desktop = &DEVICES[index_of("desktop")];
        assert_eq!(
            configure_js(PageConfig::new(desktop, false, false)),
            r#"window.__pwaLauncher && window.__pwaLauncher.configure({"statusBar":null,"screen":null})"#
        );
        // Landskab: bredde og højde byttes, og iPhone har ingen statuslinje.
        let l = PageConfig::new(iphone, true, true);
        assert_eq!((l.status_bar, l.screen), (None, Some((852.0, 393.0))));
    }

    #[test]
    fn safe_area_is_sent() {
        let iphone = &DEVICES[index_of("iphone-15-pro")];
        let mode = Mode {
            overlay: true,
            fullscreen: true,
        };
        let (w, h) = iphone.window_size(false, 1.0);
        let calls = cdp_calls(iphone, &iphone.layout(false, w, h, mode));
        let (_, safe) = calls
            .iter()
            .find(|(m, _)| *m == "Emulation.setSafeAreaInsetsOverride")
            .unwrap();
        assert_eq!(
            safe["insets"],
            json!({"top": 59, "right": 0, "bottom": 34, "left": 0})
        );
    }
}
