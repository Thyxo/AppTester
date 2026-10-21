//! Enheder (iPhone, Pixel, iPad …) og layout-matematik for mockup-vinduet.
//!
//! Vinduet består af en værktøjslinje øverst og en enhed under den. Enheden
//! skaleres med vinduet (højst 100 %), og appens webview lægges præcis over
//! skærmens indholdsområde med zoom = skala, så appen altid ser enhedens viewport.
//!
//! Indholdsområdet er skærmen minus statuslinjen – som for en installeret PWA.
//! Kan appens webview klippes (`overlay`, Windows), går appen helt til bund med
//! runde hjørner, og hjem-indikatoren tegnes oven på den (som på telefonen).
//! Ellers reserveres plads til indikatoren, så appens firkantede hjørner holdes
//! væk fra skærmens runde hjørner.
//!
//! `fullscreen` efterligner en iOS-PWA med `black-translucent` statuslinje: appen
//! fylder hele skærmen, og statuslinjen ligger oven på den. Appen får skærmens
//! sikre afstande (`safe_area`) som `env(safe-area-inset-*)`.

use serde::Serialize;

/// Værktøjslinjens højde (logiske px, skaleres ikke).
pub const TOOLBAR: f64 = 64.0;
/// Luft rundt om enheden (til sideknapper og skygge).
pub const PAD: f64 = 14.0;
/// Værktøjslinjen skal kunne være der.
pub const MIN_WIDTH: f64 = 360.0;
/// Mindste tilladte skala.
pub const MIN_SCALE: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Phone,
    Tablet,
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Notch {
    None,
    /// Dynamic Island (iPhone 14 Pro og nyere).
    Island,
    /// Hulkamera (Android).
    Punch,
    /// Hjemknap, højttaler og kamera i rammen (iPhone SE).
    HomeButton,
    /// Lille kameraprik i rammen (iPad).
    Camera,
}

/// Afstande i logiske punkter (top, højre, bund, venstre).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Insets {
    const fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    fn scaled(self, s: f64) -> Self {
        Self::new(self.top * s, self.right * s, self.bottom * s, self.left * s)
    }
}

#[derive(Debug)]
pub struct Device {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    /// Hele skærmen i punkter (portræt).
    pub screen_w: f64,
    pub screen_h: f64,
    /// Fra enhedens yderkant til skærmen (portræt).
    pub side: f64,
    pub top: f64,
    pub bottom: f64,
    /// Den lyse metalkant yderst (resten af `side` er sort bezel).
    pub rim: f64,
    pub radius: f64,
    pub screen_radius: f64,
    pub notch: Notch,
    /// Statuslinje/hjem-indikator m.m., som appen ikke tegner i.
    pub portrait: Insets,
    pub landscape: Insets,
    /// iPhone-hjem-indikator (bredde, højde, afstand til bund). Med `overlay`
    /// tegnes den oven på appen i stedet for at tage plads fra den.
    pub home_indicator: Option<(f64, f64, f64)>,
    pub can_rotate: bool,
    /// iOS/iPadOS: kan køre i fuld skærm under statuslinjen (`fullscreen`).
    pub ios: bool,
}

pub const DEVICES: &[Device] = &[
    Device {
        id: "iphone-15-pro",
        name: "iPhone 15 Pro",
        kind: Kind::Phone,
        screen_w: 393.0,
        screen_h: 852.0,
        side: 13.0,
        top: 13.0,
        bottom: 13.0,
        rim: 4.0,
        radius: 60.0,
        screen_radius: 47.0,
        notch: Notch::Island,
        portrait: Insets::new(59.0, 0.0, 34.0, 0.0),
        landscape: Insets::new(0.0, 59.0, 21.0, 59.0),
        home_indicator: Some((134.0, 5.0, 8.0)),
        ios: true,
        can_rotate: true,
    },
    Device {
        id: "iphone-15-pro-max",
        name: "iPhone 15 Pro Max",
        kind: Kind::Phone,
        screen_w: 430.0,
        screen_h: 932.0,
        side: 13.0,
        top: 13.0,
        bottom: 13.0,
        rim: 4.0,
        radius: 61.0,
        screen_radius: 48.0,
        notch: Notch::Island,
        portrait: Insets::new(59.0, 0.0, 34.0, 0.0),
        landscape: Insets::new(0.0, 59.0, 21.0, 59.0),
        home_indicator: Some((134.0, 5.0, 8.0)),
        ios: true,
        can_rotate: true,
    },
    Device {
        id: "iphone-se",
        name: "iPhone SE",
        kind: Kind::Phone,
        screen_w: 375.0,
        screen_h: 667.0,
        side: 20.0,
        top: 92.0,
        bottom: 92.0,
        rim: 3.0,
        radius: 56.0,
        screen_radius: 0.0,
        notch: Notch::HomeButton,
        portrait: Insets::new(20.0, 0.0, 0.0, 0.0),
        landscape: Insets::new(0.0, 0.0, 0.0, 0.0),
        home_indicator: None,
        ios: true,
        can_rotate: true,
    },
    Device {
        id: "pixel-8",
        name: "Pixel 8",
        kind: Kind::Phone,
        screen_w: 412.0,
        screen_h: 915.0,
        side: 12.0,
        top: 12.0,
        bottom: 12.0,
        rim: 3.0,
        radius: 42.0,
        screen_radius: 30.0,
        notch: Notch::Punch,
        portrait: Insets::new(24.0, 0.0, 24.0, 0.0),
        landscape: Insets::new(24.0, 0.0, 24.0, 24.0),
        home_indicator: None,
        ios: false,
        can_rotate: true,
    },
    Device {
        id: "ipad-air",
        name: "iPad Air",
        kind: Kind::Tablet,
        screen_w: 820.0,
        screen_h: 1180.0,
        side: 23.0,
        top: 23.0,
        bottom: 23.0,
        rim: 3.0,
        radius: 41.0,
        screen_radius: 18.0,
        notch: Notch::Camera,
        portrait: Insets::new(24.0, 0.0, 20.0, 0.0),
        landscape: Insets::new(24.0, 0.0, 20.0, 0.0),
        home_indicator: Some((320.0, 5.0, 7.0)),
        ios: true,
        can_rotate: true,
    },
    Device {
        id: "desktop",
        name: "Desktop",
        kind: Kind::Desktop,
        screen_w: 1280.0,
        screen_h: 800.0,
        side: 1.0,
        top: 30.0,
        bottom: 1.0,
        rim: 1.0,
        radius: 10.0,
        screen_radius: 0.0,
        notch: Notch::None,
        portrait: Insets::new(0.0, 0.0, 0.0, 0.0),
        landscape: Insets::new(0.0, 0.0, 0.0, 0.0),
        home_indicator: None,
        ios: false,
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

/// Hvordan appen vises på skærmen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mode {
    /// Appens webview kan klippes (runde hjørner, huller til ø og indikator).
    pub overlay: bool,
    /// iOS `black-translucent`: appen fylder hele skærmen under statuslinjen.
    pub fullscreen: bool,
}

/// Alt hvad frame-siden og appens webview skal placeres efter (vindueskoordinater).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub device: &'static str,
    pub kind: Kind,
    pub landscape: bool,
    pub scale: f64,
    /// Appen fylder hele skærmen, og statuslinjen tegnes i appen.
    pub fullscreen: bool,
    /// Hele enheden (yderkant).
    pub phone: Rect,
    /// Hele skærmen.
    pub screen: Rect,
    /// Appens område (skærm minus `insets`).
    pub content: Rect,
    /// Skalerede afstande fra skærmkant til appens område.
    pub insets: Insets,
    /// Det appen får som `env(safe-area-inset-*)` (CSS-px, uskaleret).
    pub safe_area: Insets,
    pub radius: f64,
    pub screen_radius: f64,
    /// Radius på appens øverste/nederste hjørner (klippes på Windows), 0 = firkantet.
    pub content_radius_top: f64,
    pub content_radius: f64,
    /// Hjem-indikatoren oven på appen (kun med `overlay`).
    pub indicator: Option<Rect>,
    /// Dynamic Island. I fuld skærm skæres den ud af appen.
    pub island: Option<Rect>,
    pub rim: f64,
    pub notch: Notch,
    /// Viewport i CSS-pixels, som appen ser.
    pub viewport: (f64, f64),
}

impl Device {
    /// Enhedens ydre størrelse ved skala 1. Landskab = drejet 90° mod uret
    /// (toppen af enheden til venstre).
    pub fn frame_size(&self, landscape: bool) -> (f64, f64) {
        let w = self.side * 2.0 + self.screen_w;
        let h = self.top + self.screen_h + self.bottom;
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

    fn screen_size(&self, landscape: bool) -> (f64, f64) {
        if landscape {
            (self.screen_h, self.screen_w)
        } else {
            (self.screen_w, self.screen_h)
        }
    }

    /// Skærmens sikre afstande (statuslinje, ø, hjem-indikator).
    fn safe_insets(&self, landscape: bool) -> Insets {
        if landscape {
            self.landscape
        } else {
            self.portrait
        }
    }

    /// Fuld skærm virker kun på iOS-enheder, og kun når appen kan klippes.
    pub fn fullscreen(&self, mode: Mode) -> bool {
        mode.overlay && mode.fullscreen && self.ios
    }

    /// Den del af skærmen, appen ikke får (tegnes af rammen).
    pub fn insets(&self, landscape: bool, mode: Mode) -> Insets {
        if self.fullscreen(mode) {
            return Insets::new(0.0, 0.0, 0.0, 0.0);
        }
        let mut i = self.safe_insets(landscape);
        if mode.overlay && self.home_indicator.is_some() {
            i.bottom = 0.0;
        }
        i
    }

    /// `env(safe-area-inset-*)`: de sikre afstande, der ligger inden for appens område.
    pub fn safe_area(&self, landscape: bool, mode: Mode) -> Insets {
        let full = self.safe_insets(landscape);
        let used = self.insets(landscape, mode);
        Insets::new(
            full.top - used.top,
            full.right - used.right,
            full.bottom - used.bottom,
            full.left - used.left,
        )
    }

    pub fn viewport(&self, landscape: bool, mode: Mode) -> (f64, f64) {
        let (w, h) = self.screen_size(landscape);
        let i = self.insets(landscape, mode);
        (w - i.left - i.right, h - i.top - i.bottom)
    }

    /// Vinduesstørrelse, der passer præcis til enheden ved skala `scale`.
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

    /// Placering af enhed, skærm og app i et vindue af størrelsen `win_w` × `win_h`.
    pub fn layout(&self, landscape: bool, win_w: f64, win_h: f64, mode: Mode) -> Layout {
        let overlay = mode.overlay;
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
        let (sw, sh) = self.screen_size(landscape);
        let screen = Rect {
            x: (phone.x + ox * scale).round(),
            y: (phone.y + oy * scale).round(),
            w: (sw * scale).round(),
            h: (sh * scale).round(),
        };
        let insets = self.insets(landscape, mode);
        let (vw, vh) = self.viewport(landscape, mode);
        let content = Rect {
            x: (screen.x + insets.left * scale).round(),
            y: (screen.y + insets.top * scale).round(),
            w: (vw * scale).round(),
            h: (vh * scale).round(),
        };
        // Går appen helt ud i et af skærmens runde hjørner, skal den rundes.
        let r = self.screen_radius;
        let sides = insets.left < r || insets.right < r;
        let corner = |edge: f64| {
            if overlay && edge < r && sides {
                r * scale
            } else {
                0.0
            }
        };
        let (content_radius_top, content_radius) = (corner(insets.top), corner(insets.bottom));
        let island = (self.notch == Notch::Island).then(|| {
            let (iw, ih, gap) = (125.0 * scale, 37.0 * scale, 11.0 * scale);
            if landscape {
                Rect {
                    x: (screen.x + gap).round(),
                    y: (screen.y + (screen.h - iw) / 2.0).round(),
                    w: ih.round(),
                    h: iw.round(),
                }
            } else {
                Rect {
                    x: (screen.x + (screen.w - iw) / 2.0).round(),
                    y: (screen.y + gap).round(),
                    w: iw.round(),
                    h: ih.round(),
                }
            }
        });
        let indicator = match (overlay, self.home_indicator) {
            (true, Some((iw, ih, gap))) => Some(Rect {
                x: (screen.x + (screen.w - iw * scale) / 2.0).round(),
                y: (screen.y + screen.h - (gap + ih) * scale).round(),
                w: (iw * scale).round(),
                h: (ih * scale).round().max(2.0),
            }),
            _ => None,
        };
        Layout {
            device: self.id,
            kind: self.kind,
            landscape,
            scale,
            fullscreen: self.fullscreen(mode),
            phone,
            screen,
            content,
            insets: insets.scaled(scale),
            safe_area: self.safe_area(landscape, mode),
            radius: self.radius * scale,
            screen_radius: self.screen_radius * scale,
            content_radius_top,
            content_radius,
            indicator,
            island,
            rim: self.rim * scale,
            notch: self.notch,
            viewport: (vw, vh),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Mode = Mode {
        overlay: false,
        fullscreen: false,
    };
    const OVERLAY: Mode = Mode {
        overlay: true,
        fullscreen: false,
    };
    const FULL: Mode = Mode {
        overlay: true,
        fullscreen: true,
    };

    fn iphone() -> &'static Device {
        &DEVICES[index_of("iphone-15-pro")]
    }

    fn inside(inner: Rect, outer: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
    }

    #[test]
    fn full_size_window_gives_scale_one() {
        let d = iphone();
        let (w, h) = d.window_size(false, 1.0);
        let l = d.layout(false, w, h, PLAIN);
        assert!((l.scale - 1.0).abs() < 1e-9);
        assert_eq!((l.screen.w, l.screen.h), (393.0, 852.0));
        // Uden statuslinje (59) og hjem-indikator (34).
        assert_eq!(l.viewport, (393.0, 759.0));
        assert_eq!((l.content.w, l.content.h), (393.0, 759.0));
        assert!(inside(l.content, l.screen) && inside(l.screen, l.phone));
        assert_eq!(l.content_radius, 0.0);
        assert!(l.indicator.is_none());
    }

    /// Med overlay (Windows) går appen helt til bund som på telefonen:
    /// runde nederste hjørner, og indikatoren ligger ovenpå.
    #[test]
    fn overlay_reaches_bottom_with_rounded_corners() {
        let d = iphone();
        let (w, h) = d.window_size(false, 1.0);
        let l = d.layout(false, w, h, OVERLAY);
        assert_eq!(l.viewport, (393.0, 793.0));
        assert_eq!(l.content.y + l.content.h, l.screen.y + l.screen.h);
        assert_eq!(l.content_radius, 47.0);
        let ind = l.indicator.expect("indikator");
        assert!(inside(ind, l.content));
        assert_eq!((ind.w, ind.h), (134.0, 5.0));
        assert_eq!(ind.y + ind.h + 8.0, l.screen.y + l.screen.h);

        // Landskab: appen starter efter øen (59 > 47), så ingen afrunding.
        let (w, h) = d.window_size(true, 1.0);
        let l = d.layout(true, w, h, OVERLAY);
        assert_eq!(l.viewport, (734.0, 393.0));
        assert_eq!(l.content_radius, 0.0);
        assert!(l.indicator.is_some());

        // Enheder uden iPhone-indikator er uændrede.
        let se = &DEVICES[index_of("iphone-se")];
        let (w, h) = se.window_size(false, 1.0);
        assert_eq!(se.layout(false, w, h, OVERLAY).viewport, (375.0, 647.0));
    }

    /// iOS black-translucent: appen fylder hele skærmen, får de sikre afstande som
    /// safe-area, og øen skæres ud.
    #[test]
    fn fullscreen_covers_screen_with_safe_area() {
        let d = iphone();
        let (w, h) = d.window_size(false, 1.0);
        let l = d.layout(false, w, h, FULL);
        assert!(l.fullscreen);
        assert_eq!(l.viewport, (393.0, 852.0));
        assert_eq!(l.content, l.screen);
        assert_eq!(l.safe_area, Insets::new(59.0, 0.0, 34.0, 0.0));
        assert_eq!((l.content_radius_top, l.content_radius), (47.0, 47.0));
        let island = l.island.expect("ø");
        assert!(inside(island, l.content));
        assert_eq!((island.w, island.h), (125.0, 37.0));

        let (w, h) = d.window_size(true, 1.0);
        let l = d.layout(true, w, h, FULL);
        assert_eq!(l.viewport, (852.0, 393.0));
        assert_eq!(l.safe_area, Insets::new(0.0, 59.0, 21.0, 59.0));

        // Uden fuld skærm får appen kun den nederste afstand (indikatoren ligger ovenpå).
        let (w, h) = d.window_size(false, 1.0);
        assert_eq!(
            d.layout(false, w, h, OVERLAY).safe_area,
            Insets::new(0.0, 0.0, 34.0, 0.0)
        );
        assert_eq!(
            d.layout(false, w, h, PLAIN).safe_area,
            Insets::new(0.0, 0.0, 0.0, 0.0)
        );

        // Kun iOS, og kun hvis appen kan klippes.
        let pixel = &DEVICES[index_of("pixel-8")];
        let (w, h) = pixel.window_size(false, 1.0);
        assert!(!pixel.layout(false, w, h, FULL).fullscreen);
        let no_clip = Mode {
            overlay: false,
            fullscreen: true,
        };
        assert!(!d.layout(false, w, h, no_clip).fullscreen);

        // iPhone SE: statuslinjen (20) bliver safe-area.
        let se = &DEVICES[index_of("iphone-se")];
        let (w, h) = se.window_size(false, 1.0);
        let l = se.layout(false, w, h, FULL);
        assert_eq!(l.viewport, (375.0, 667.0));
        assert_eq!(l.safe_area.top, 20.0);
        assert_eq!(l.content_radius, 0.0);
    }

    #[test]
    fn half_size_keeps_viewport_and_ratio() {
        let d = iphone();
        let (w, h) = d.window_size(false, 0.5);
        let l = d.layout(false, w, h, PLAIN);
        assert!((l.scale - 0.5).abs() < 0.01);
        // Appen ser stadig samme viewport – den er bare zoomet ned.
        assert!((l.content.w / l.scale - 393.0).abs() < 2.0);
        assert!((l.content.h / l.scale - 759.0).abs() < 2.0);
    }

    #[test]
    fn too_wide_window_is_letterboxed_not_stretched() {
        let d = iphone();
        let (_, h) = d.window_size(false, 0.6);
        let l = d.layout(false, 2000.0, h, PLAIN);
        assert!((l.scale - 0.6).abs() < 0.01);
        let (fw, fh) = d.frame_size(false);
        assert!((l.phone.w / l.phone.h - fw / fh).abs() < 0.01);
    }

    #[test]
    fn never_larger_than_full_size() {
        let l = iphone().layout(false, 5000.0, 5000.0, OVERLAY);
        assert_eq!(l.scale, 1.0);
    }

    #[test]
    fn landscape_uses_landscape_insets() {
        let d = iphone();
        let (w, h) = d.window_size(true, 1.0);
        let l = d.layout(true, w, h, PLAIN);
        assert_eq!((l.screen.w, l.screen.h), (852.0, 393.0));
        assert_eq!(l.viewport, (852.0 - 118.0, 393.0 - 21.0));
        assert!(inside(l.content, l.screen));
    }

    /// Appens firkantede hjørner må højst stikke ~2 pt ud over skærmens runde hjørner
    /// (uden overlay; med overlay klippes hjørnerne).
    #[test]
    fn content_corners_stay_inside_rounded_screen() {
        for d in DEVICES {
            for landscape in [false, true] {
                let (sw, sh) = d.screen_size(landscape);
                let i = d.insets(landscape, PLAIN);
                let r = d.screen_radius;
                // Hjørnerne af indholdsområdet, målt fra skærmens hjørner.
                for (dx, dy) in [
                    (i.left, i.top),
                    (i.right, i.top),
                    (i.left, i.bottom),
                    (i.right, i.bottom),
                ] {
                    if dx >= r || dy >= r {
                        continue;
                    }
                    // Afstand fra hjørnecirklens centrum skal være ≤ r (+2 pt).
                    let (cx, cy) = (r - dx, r - dy);
                    let dist = (cx * cx + cy * cy).sqrt();
                    assert!(
                        dist <= r + 2.0,
                        "{} {}: hjørne stikker {:.1} pt ud ({sw}×{sh})",
                        d.name,
                        if landscape { "landskab" } else { "portræt" },
                        dist - r
                    );
                }
            }
        }
    }

    #[test]
    fn limits_and_fit() {
        let d = iphone();
        let ((min_w, min_h), (max_w, max_h)) = d.size_limits(false);
        assert!(min_w >= MIN_WIDTH && min_h < max_h && min_w <= max_w);
        // 1080p-skærm (1920×1040 logisk): iPhone 15 Pro kan ikke være i fuld størrelse.
        let s = d.fit_scale(false, 1920.0, 1040.0);
        assert!(s < 1.0 && s > 0.8);
        assert_eq!(index_of("findes-ikke"), 0);
    }
}
