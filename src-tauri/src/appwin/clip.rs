//! Windows: klipper appens webview med en vinduesregion – runde hjørner som
//! skærmen og pilleformede huller, hvor rammen under tegner hjem-indikatoren
//! og (i fuld skærm) Dynamic Island. Så kan appen gå helt ud til kanten, som på
//! telefonen.

use tauri::Webview;

use super::device::Layout;

/// Om platformen understøtter klipning (bestemmer layoutet i `device.rs`).
pub const SUPPORTED: bool = cfg!(windows);

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
    let (rt, rb) = (px(layout.content_radius_top), px(layout.content_radius));
    let pill = |r: &super::device::Rect| {
        (
            px(r.x - layout.content.x),
            px(r.y - layout.content.y),
            px(r.w),
            px(r.h).max(2),
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
