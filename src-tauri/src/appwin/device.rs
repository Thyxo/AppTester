//! Enheder (iPhone, Pixel, iPad …) og layout-matematik for mockup-vinduet.
//!
//! Vinduet består af en værktøjslinje øverst og en telefonramme under den.
//! Rammen skaleres med vinduet (højst 100 %), og appens webview lægges præcis
//! over rammens skærm med zoom = skala, så appen altid ser enhedens viewport.

use serde::Serialize;

/// Værktøjslinjens højde (logiske px, skaleres ikke).
pub const TOOLBAR: f64 = 64.0;
/// Luft rundt om rammen (til sideknapper og skygge).
pub const PAD: f64 = 12.0;
/// Værktøjslinjen skal kunne være der.
pub const MIN_WIDTH: f64 = 340.0;
/// Mindste tilladte skala.
pub const MIN_SCALE: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Notch {
    None,
    /// Dynamic Island (iPhone 14 Pro og nyere).
    Island,
    /// Hulkamera (Android).
    Punch,
    /// Hjemknap (iPhone SE).
    HomeButton,
    /// Lille kameraprik i kanten (iPad).
    Camera,
}

#[derive(Debug)]
pub struct Device {
    pub id: &'static str,
    pub name: &'static str,
    /// Viewport i CSS-pixels (portræt).
    pub width: f64,
    pub height: f64,
    /// Rammens tykkelse (portræt).
    pub side: f64,
    pub top: f64,
    pub bottom: f64,
    /// Rammens hjørneradius.
    pub radius: f64,
    pub notch: Notch,
    pub can_rotate: bool,
}

pub const DEVICES: &[Device] = &[
    Device {
        id: "iphone-15",
        name: "iPhone 15",
        width: 393.0,
        height: 852.0,
        side: 14.0,
        top: 46.0,
        bottom: 26.0,
        radius: 62.0,
        notch: Notch::Island,
        can_rotate: true,
    },
    Device {
        id: "iphone-se",
        name: "iPhone SE",
        width: 375.0,
        height: 667.0,
        side: 18.0,
        top: 96.0,
        bottom: 96.0,
        radius: 56.0,
        notch: Notch::HomeButton,
        can_rotate: true,
    },
    Device {
        id: "pixel-8",
        name: "Pixel 8",
        width: 412.0,
        height: 915.0,
        side: 12.0,
        top: 38.0,
        bottom: 22.0,
        radius: 48.0,
        notch: Notch::Punch,
        can_rotate: true,
    },
    Device {
        id: "ipad",
        name: "iPad",
        width: 820.0,
        height: 1180.0,
        side: 26.0,
        top: 26.0,
        bottom: 26.0,
        radius: 40.0,
        notch: Notch::Camera,
        can_rotate: true,
    },
    Device {
        id: "desktop",
        name: "Desktop",
        width: 1280.0,
        height: 800.0,
        side: 0.0,
        top: 0.0,
        bottom: 0.0,
        radius: 10.0,
        notch: Notch::None,
        can_rotate: false,
    },
];

pub fn index_of(id: &str) -> usize {
    DEVICES.iter().position(|d| d.id == id).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Alt hvad frame-siden og appens webview skal placeres efter.
#[derive(Debug, Clone, Serialize)]
pub struct Layout {
    pub device: &'static str,
    pub landscape: bool,
    pub scale: f64,
    /// Hele rammen (telefonens yderkant).
    pub phone: Rect,
    /// Skærmen, hvor appens webview ligger.
    pub screen: Rect,
    pub radius: f64,
    pub notch: Notch,
    /// Viewport i CSS-pixels, som appen ser.
    pub viewport: (f64, f64),
}

impl Device {
    /// Rammens ydre størrelse ved skala 1. Landskab = drejet 90° mod uret
    /// (toppen af telefonen til venstre).
    pub fn frame_size(&self, landscape: bool) -> (f64, f64) {
        let w = self.side * 2.0 + self.width;
        let h = self.top + self.height + self.bottom;
        if landscape {
            (h, w)
        } else {
            (w, h)
        }
    }

    fn screen_offset(&self, landscape: bool) -> (f64, f64) {
        if landscape {
            (self.top, self.side)
        } else {
            (self.side, self.top)
        }
    }

    pub fn viewport(&self, landscape: bool) -> (f64, f64) {
        if landscape {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// Vinduesstørrelse, der passer præcis til rammen ved skala `scale`.
    pub fn window_size(&self, landscape: bool, scale: f64) -> (f64, f64) {
        let (fw, fh) = self.frame_size(landscape);
        (
            (fw * scale + 2.0 * PAD).max(MIN_WIDTH).round(),
            (TOOLBAR + fh * scale + PAD).round(),
        )
    }

    /// (min, max) vinduesstørrelse: mellem MIN_SCALE og 100 %.
    pub fn size_limits(&self, landscape: bool) -> ((f64, f64), (f64, f64)) {
        (
            self.window_size(landscape, MIN_SCALE),
            self.window_size(landscape, 1.0),
        )
    }

    /// Største skala (≤ 1), der kan være på en skærm af den givne logiske størrelse.
    pub fn fit_scale(&self, landscape: bool, screen_w: f64, screen_h: f64) -> f64 {
        let (fw, fh) = self.frame_size(landscape);
        let s = ((screen_w * 0.92 - 2.0 * PAD) / fw).min((screen_h * 0.9 - TOOLBAR - PAD) / fh);
        s.clamp(MIN_SCALE, 1.0)
    }

    /// Placering af ramme og skærm i et vindue af størrelsen `win_w` × `win_h`.
    pub fn layout(&self, landscape: bool, win_w: f64, win_h: f64) -> Layout {
        let (fw, fh) = self.frame_size(landscape);
        let avail_w = (win_w - 2.0 * PAD).max(1.0);
        let avail_h = (win_h - TOOLBAR - PAD).max(1.0);
        let scale = (avail_w / fw).min(avail_h / fh).clamp(0.05, 1.0);

        let phone = Rect {
            x: ((win_w - fw * scale) / 2.0).round(),
            y: (TOOLBAR + (avail_h - fh * scale) / 2.0).round(),
            w: (fw * scale).round(),
            h: (fh * scale).round(),
        };
        let (ox, oy) = self.screen_offset(landscape);
        let (vw, vh) = self.viewport(landscape);
        let screen = Rect {
            x: (phone.x + ox * scale).round(),
            y: (phone.y + oy * scale).round(),
            w: (vw * scale).round(),
            h: (vh * scale).round(),
        };
        Layout {
            device: self.id,
            landscape,
            scale,
            phone,
            screen,
            radius: self.radius * scale,
            notch: self.notch,
            viewport: (vw, vh),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iphone() -> &'static Device {
        &DEVICES[index_of("iphone-15")]
    }

    #[test]
    fn full_size_window_gives_scale_one() {
        let d = iphone();
        let (w, h) = d.window_size(false, 1.0);
        let l = d.layout(false, w, h);
        assert!((l.scale - 1.0).abs() < 1e-9);
        assert_eq!((l.screen.w, l.screen.h), (393.0, 852.0));
        assert_eq!(l.viewport, (393.0, 852.0));
        // Skærmen ligger inde i rammen.
        assert!(l.screen.x >= l.phone.x && l.screen.y >= l.phone.y);
        assert!(l.screen.x + l.screen.w <= l.phone.x + l.phone.w);
        assert!(l.screen.y + l.screen.h <= l.phone.y + l.phone.h);
    }

    #[test]
    fn half_size_keeps_viewport_and_ratio() {
        let d = iphone();
        let (w, h) = d.window_size(false, 0.5);
        let l = d.layout(false, w, h);
        assert!((l.scale - 0.5).abs() < 0.01);
        // Appen ser stadig 393×852 – skærmen er bare zoomet ned.
        assert!((l.screen.w / l.scale - 393.0).abs() < 2.0);
        assert!((l.screen.h / l.scale - 852.0).abs() < 2.0);
    }

    #[test]
    fn too_wide_window_is_letterboxed_not_stretched() {
        let d = iphone();
        let (_, h) = d.window_size(false, 0.6);
        let l = d.layout(false, 2000.0, h);
        assert!((l.scale - 0.6).abs() < 0.01);
        // Rammen centreres; forholdet bevares.
        let (fw, fh) = d.frame_size(false);
        assert!((l.phone.w / l.phone.h - fw / fh).abs() < 0.01);
    }

    #[test]
    fn never_larger_than_full_size() {
        let l = iphone().layout(false, 5000.0, 5000.0);
        assert_eq!(l.scale, 1.0);
    }

    #[test]
    fn landscape_swaps_viewport() {
        let d = iphone();
        let (w, h) = d.window_size(true, 1.0);
        let l = d.layout(true, w, h);
        assert_eq!(l.viewport, (852.0, 393.0));
        assert_eq!((l.screen.w, l.screen.h), (852.0, 393.0));
    }

    #[test]
    fn limits_and_fit() {
        let d = iphone();
        let ((min_w, min_h), (max_w, max_h)) = d.size_limits(false);
        assert!(min_w >= MIN_WIDTH && min_h < max_h && min_w <= max_w);
        // 1080p-skærm (1920×1040 logisk): iPhone 15 kan ikke være i fuld størrelse.
        let s = d.fit_scale(false, 1920.0, 1040.0);
        assert!(s < 1.0 && s > 0.8);
        assert_eq!(index_of("findes-ikke"), 0);
    }
}
