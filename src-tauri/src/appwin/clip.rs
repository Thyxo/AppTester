//! Windows: klipper appens webview med en vinduesregion – runde nederste hjørner
//! som skærmen og et pilleformet hul, hvor rammen under tegner hjem-indikatoren.
//! Så kan appen gå helt til bund, som på telefonen.

use tauri::Webview;

use super::device::Layout;

/// Om platformen understøtter klipning (bestemmer layoutet i `device.rs`).
pub const SUPPORTED: bool = cfg!(windows);

#[cfg(windows)]
pub fn apply(content: &Webview, layout: &Layout, scale_factor: f64) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, RGN_DIFF, RGN_OR,
    };

    // Regionen er i fysiske pixels relativt til webviewets eget vindue.
    let px = |v: f64| (v * scale_factor).round() as i32;
    let (w, h) = (px(layout.content.w), px(layout.content.h));
    let r = px(layout.content_radius);
    let hole = layout.indicator.map(|i| {
        (
            px(i.x - layout.content.x),
            px(i.y - layout.content.y),
            px(i.w),
            px(i.h).max(2),
        )
    });

    let _ = content.with_webview(move |webview| unsafe {
        // WebView2 ligger i wry's container-vindue; regionen klipper også det indlejrede browservindue.
        let mut hwnd = HWND::default();
        if webview.controller().ParentWindow(&mut hwnd).is_err() || hwnd.is_invalid() {
            return;
        }
        if r == 0 && hole.is_none() {
            SetWindowRgn(hwnd, None, true);
            return;
        }

        let region = if r > 0 {
            // Firkantet top + afrundet bund.
            let region = CreateRectRgn(0, 0, w, h - r);
            let rounded = CreateRoundRectRgn(0, 0, w + 1, h + 1, 2 * r, 2 * r);
            CombineRgn(Some(region), Some(region), Some(rounded), RGN_OR);
            let _ = DeleteObject(rounded.into());
            region
        } else {
            CreateRectRgn(0, 0, w, h)
        };
        if let Some((x, y, hw, hh)) = hole {
            let pill = CreateRoundRectRgn(x, y, x + hw + 1, y + hh + 1, hh, hh);
            CombineRgn(Some(region), Some(region), Some(pill), RGN_DIFF);
            let _ = DeleteObject(pill.into());
        }
        // Systemet overtager regionen.
        SetWindowRgn(hwnd, Some(region), true);
    });
}

#[cfg(not(windows))]
pub fn apply(_content: &Webview, _layout: &Layout, _scale_factor: f64) {}
