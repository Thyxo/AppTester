//! Viewport-presets: ændrer vinduets indre størrelse (CSS-pixels).

use tauri::{LogicalSize, WebviewWindow};

pub struct Preset {
    pub name: &'static str,
    pub width: f64,
    pub height: f64,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "iPhone SE",
        width: 375.0,
        height: 667.0,
    },
    Preset {
        name: "iPhone 15",
        width: 393.0,
        height: 852.0,
    },
    Preset {
        name: "Pixel 8",
        width: 412.0,
        height: 915.0,
    },
    Preset {
        name: "iPad",
        width: 820.0,
        height: 1180.0,
    },
    Preset {
        name: "Desktop",
        width: 1280.0,
        height: 800.0,
    },
];

pub fn apply(win: &WebviewWindow, preset: &Preset) {
    resize(win, preset.width, preset.height);
}

/// Bytter bredde og højde (portræt ↔ landskab).
pub fn rotate(win: &WebviewWindow) {
    let (Ok(size), Ok(scale)) = (win.inner_size(), win.scale_factor()) else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    resize(win, size.height, size.width);
}

fn resize(win: &WebviewWindow, width: f64, height: f64) {
    let _ = win.unmaximize();
    if let Err(err) = win.set_size(LogicalSize::new(width, height)) {
        eprintln!("Kunne ikke ændre vinduesstørrelse: {err}");
    }
}
