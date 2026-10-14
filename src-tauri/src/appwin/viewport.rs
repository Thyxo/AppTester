//! Viewport-presets: giver siden præcis den valgte viewport (CSS-pixels).
//!
//! Vinduets indre størrelse er ikke altid lig viewporten – på Linux ligger
//! menulinjen inde i vinduet. Derfor måles `innerWidth`/`innerHeight` efter
//! størrelsesskiftet, og vinduet justeres med forskellen.

use std::thread;
use std::time::Duration;

use tauri::{LogicalSize, WebviewWindow};

use super::status::parse_eval;

const MEASURE_JS: &str = "JSON.stringify([innerWidth, innerHeight])";

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

/// Bytter viewportens bredde og højde (portræt ↔ landskab).
pub fn rotate(win: &WebviewWindow) {
    let target = win.clone();
    let _ = win.eval_with_callback(MEASURE_JS, move |raw| {
        if let Some([w, h]) = parse_eval::<[f64; 2]>(&raw) {
            resize(&target, h, w);
        }
    });
}

fn resize(win: &WebviewWindow, width: f64, height: f64) {
    let _ = win.unmaximize();
    if let Err(err) = win.set_size(LogicalSize::new(width, height)) {
        eprintln!("Kunne ikke ændre vinduesstørrelse: {err}");
        return;
    }
    // Mål viewporten, når siden har nået at reagere på den nye størrelse, og ret til.
    let win = win.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        let target = win.clone();
        let _ = win.eval_with_callback(MEASURE_JS, move |raw| {
            let Some([vw, vh]) = parse_eval::<[f64; 2]>(&raw) else {
                return;
            };
            if let Some((w, h)) = corrected(width, height, vw, vh) {
                let _ = target.set_size(LogicalSize::new(w, h));
            }
        });
    });
}

/// Ny vinduesstørrelse, hvis viewporten afviger fra målet med mindst 1 px.
fn corrected(width: f64, height: f64, vw: f64, vh: f64) -> Option<(f64, f64)> {
    let (dw, dh) = (width - vw, height - vh);
    (dw.abs() >= 1.0 || dh.abs() >= 1.0).then_some((width + dw, height + dh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_for_menu_bar() {
        // iPhone SE med 25 px menulinje inde i vinduet.
        assert_eq!(corrected(375.0, 667.0, 375.0, 642.0), Some((375.0, 692.0)));
        assert_eq!(corrected(375.0, 667.0, 375.0, 667.0), None);
    }
}
