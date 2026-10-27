//! Windows: klipper appens webview med en vinduesregion – runde hjørner og
//! pilleformede huller, hvor rammen under tegner hjem-indikatoren og (i fuld
//! skærm) Dynamic Island. Så kan appen gå helt ud til kanten, som på telefonen.
//!
//! En vinduesregion kantudglattes ikke. De synlige kanter tegnes derfor glat i
//! appens side (emulate.js), og regionen ligger lidt uden for dem, så dens
//! trappetrin skjules: hjørnerne med mindre radius, hullerne lidt mindre.

use tauri::Webview;

use super::device::Layout;

/// Om platformen understøtter klipning (bestemmer layoutet i `device.rs`).
pub const SUPPORTED: bool = cfg!(windows);

/// Regionens hjørneradius i forhold til skærmens.
#[cfg_attr(not(windows), allow(dead_code))]
const CORNER_FACTOR: f64 = 0.6;
/// Så meget mindre (logiske px) hullerne er end øen og indikatoren.
#[cfg_attr(not(windows), allow(dead_code))]
const HOLE_INSET: f64 = 1.5;

#[cfg(windows)]
pub fn apply(content: &Webview, layout: &Layout, scale_factor: f64) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HRGN, RGN_AND,
        RGN_DIFF, RGN_OR,
    };

    // Regionen er i fysiske pixels relativt til webviewets eget vindue.
    let px = |v: f64| (v * scale_factor).round() as i32;
    let (w, h) = (px(layout.content.w), px(layout.content.h));
    // Hjørnernes trappetrin skal ligge uden for den glatte bue (radius r).
    let corner = |r: f64| px(r * CORNER_FACTOR);
    let (rt, rb) = (
        corner(layout.content_radius_top),
        corner(layout.content_radius),
    );
    // Hullerne skal ligge inden for de glatte piller, appen tegner ovenpå.
    let inset = px(HOLE_INSET).max(1);
    let pill = |r: &super::device::Rect| {
        let (w, h) = (px(r.w) - 2 * inset, px(r.h) - 2 * inset);
        (
            px(r.x - layout.content.x) + inset,
            px(r.y - layout.content.y) + inset,
            w.max(2),
            h.max(2),
        )
    };
    let mut holes: Vec<(i32, i32, i32, i32)> = layout.indicator.iter().map(pill).collect();
    if layout.fullscreen {
        holes.extend(layout.island.iter().map(pill));
    }

    let _ = content.with_webview(move |webview| unsafe {
        // WebView2 ligger i wry's container-vindue; regionen klipper også det indlejrede browservindue.
        let mut hwnd = HWND::default();
        if webview.controller().ParentWindow(&mut hwnd).is_err() || hwnd.is_invalid() {
            return;
        }
        if rt == 0 && rb == 0 && holes.is_empty() {
            SetWindowRgn(hwnd, None, true);
            return;
        }

        // Øverste og nederste halvdel med hver sin hjørneradius.
        let half = |r: i32, y0: i32, y1: i32| -> HRGN {
            let shape = if r > 0 {
                CreateRoundRectRgn(0, 0, w + 1, h + 1, 2 * r, 2 * r)
            } else {
                CreateRectRgn(0, 0, w, h)
            };
            let band = CreateRectRgn(0, y0, w, y1);
            CombineRgn(Some(shape), Some(shape), Some(band), RGN_AND);
            let _ = DeleteObject(band.into());
            shape
        };
        let region = half(rt, 0, h / 2);
        let bottom = half(rb, h / 2, h);
        CombineRgn(Some(region), Some(region), Some(bottom), RGN_OR);
        let _ = DeleteObject(bottom.into());

        for (x, y, hw, hh) in holes {
            let r = hw.min(hh);
            let pill = CreateRoundRectRgn(x, y, x + hw + 1, y + hh + 1, r, r);
            CombineRgn(Some(region), Some(region), Some(pill), RGN_DIFF);
            let _ = DeleteObject(pill.into());
        }
        // Systemet overtager regionen.
        SetWindowRgn(hwnd, Some(region), true);
    });
}

#[cfg(not(windows))]
pub fn apply(_content: &Webview, _layout: &Layout, _scale_factor: f64) {}
