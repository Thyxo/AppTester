//! Metadata fra appens web manifest: navn, farver og ikon.
//!
//! Resultatet caches som JSON (inkl. ikonet som data-URL), så launcheren
//! også viser ikoner offline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::Url;

/// Ikoner større end dette ignoreres.
const MAX_ICON_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub name: Option<String>,
    pub theme_color: Option<String>,
    pub background_color: Option<String>,
    pub display: Option<String>,
    /// Ikon som `data:`-URL.
    pub icon: Option<String>,
    pub has_manifest: bool,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("PWA-Launcher/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP-klient kunne ikke oprettes")
}

// ---------- Cache ----------

fn cache_file(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join("meta").join(format!("{id}.json"))
}

pub fn load_cached(cache_dir: &Path, id: &str) -> Option<Meta> {
    let bytes = std::fs::read(cache_file(cache_dir, id)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_cached(cache_dir: &Path, id: &str, meta: &Meta) -> std::io::Result<()> {
    let path = cache_file(cache_dir, id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(meta)?)?;
    std::fs::rename(tmp, path)
}

// ---------- Hentning ----------

/// Henter forsiden, finder manifestet og det bedste ikon.
pub async fn fetch(client: &reqwest::Client, page: &Url) -> Result<Meta, String> {
    let resp = client.get(page.clone()).send().await.map_err(net_error)?;
    check_status(&resp)?;
    let base = resp.url().clone();
    let html = resp.text().await.map_err(net_error)?;
    let links = parse_links(&html);

    // 1. <link rel="manifest">, 2. gængse filnavne.
    let mut manifest_urls: Vec<Url> = links
        .iter()
        .filter(|l| l.has_rel("manifest"))
        .filter_map(|l| base.join(&l.href).ok())
        .collect();
    for fallback in ["/manifest.webmanifest", "/manifest.json"] {
        if let Ok(u) = base.join(fallback) {
            if !manifest_urls.contains(&u) {
                manifest_urls.push(u);
            }
        }
    }

    let mut found = None;
    for url in manifest_urls {
        if let Some(m) = fetch_manifest(client, &url).await {
            found = Some((url, m));
            break;
        }
    }

    let mut icon_urls: Vec<Url> = Vec::new();
    if let Some((murl, m)) = &found {
        icon_urls.extend(
            rank_icons(&m.icons)
                .into_iter()
                .filter_map(|s| murl.join(s).ok()),
        );
    }
    // Fallback: apple-touch-icon, <link rel="icon">, /favicon.ico.
    let mut page_icons: Vec<&Link> = links
        .iter()
        .filter(|l| l.has_rel("apple-touch-icon") || l.has_rel("icon"))
        .collect();
    page_icons.sort_by_key(|l| std::cmp::Reverse(largest_size(l.sizes.as_deref())));
    icon_urls.extend(page_icons.iter().filter_map(|l| base.join(&l.href).ok()));
    icon_urls.extend(base.join("/favicon.ico").ok());

    let mut icon = None;
    for url in icon_urls {
        if let Some(data) = fetch_icon(client, &url).await {
            icon = Some(data);
            break;
        }
    }

    let (has_manifest, m) = match found {
        Some((_, m)) => (true, m),
        None => (false, WebManifest::default()),
    };
    Ok(Meta {
        name: m.name.or(m.short_name).filter(|s| !s.trim().is_empty()),
        theme_color: m.theme_color,
        background_color: m.background_color,
        display: m.display,
        icon,
        has_manifest,
    })
}

#[derive(Debug, Default, Deserialize)]
struct WebManifest {
    name: Option<String>,
    short_name: Option<String>,
    theme_color: Option<String>,
    background_color: Option<String>,
    display: Option<String>,
    #[serde(default)]
    icons: Vec<ManifestIcon>,
}

#[derive(Debug, Deserialize)]
struct ManifestIcon {
    src: String,
    sizes: Option<String>,
    #[serde(rename = "type")]
    mime: Option<String>,
    purpose: Option<String>,
}

async fn fetch_manifest(client: &reqwest::Client, url: &Url) -> Option<WebManifest> {
    let resp = client.get(url.clone()).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    // Vercel-SPA'er svarer ofte index.html på ukendte stier – det fejler parsingen her.
    let bytes = resp.bytes().await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

async fn fetch_icon(client: &reqwest::Client, url: &Url) -> Option<String> {
    let resp = client.get(url.clone()).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let header_mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    let mime = header_mime
        .filter(|m| m.starts_with("image/"))
        .or_else(|| mime_from_path(url.path()).map(str::to_string))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

fn mime_from_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => return None,
    })
}

fn check_status(resp: &reqwest::Response) -> Result<(), String> {
    let status = resp.status();
    match status.as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err("Siden kræver login (fx Vercel Deployment Protection)".into()),
        404 => Err("Siden findes ikke (404) – tjek url i apps.json".into()),
        _ => Err(format!("Siden svarede med fejl {status}")),
    }
}

fn net_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "Siden svarede ikke i tide".into()
    } else if err.is_connect() {
        "Ingen forbindelse – er du offline?".into()
    } else {
        format!("Netværksfejl: {err}")
    }
}

// ---------- Ikonvalg ----------

/// Ikon-src'er i prioriteret rækkefølge: "any"-formål før maskable,
/// og helst mindst 192 px (svg tæller som stor).
fn rank_icons(icons: &[ManifestIcon]) -> Vec<&str> {
    let mut scored: Vec<(i64, &str)> = icons
        .iter()
        .map(|icon| {
            let purpose = icon.purpose.as_deref().unwrap_or("any");
            let is_any = purpose.split_whitespace().any(|p| p == "any");
            let is_svg = icon.mime.as_deref() == Some("image/svg+xml")
                || icon.src.to_ascii_lowercase().ends_with(".svg");
            let size = if is_svg {
                512
            } else {
                largest_size(icon.sizes.as_deref())
            };
            let fit = if size >= 192 {
                1000 - (size - 192).min(800)
            } else {
                size
            };
            (fit + if is_any { 10_000 } else { 0 }, icon.src.as_str())
        })
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, src)| src).collect()
}

/// Største kant i en `sizes`-attribut som "48x48 192x192". "any" = 512.
fn largest_size(sizes: Option<&str>) -> i64 {
    sizes
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|s| {
            if s.eq_ignore_ascii_case("any") {
                return Some(512);
            }
            let (w, h) = s
                .to_ascii_lowercase()
                .split_once('x')
                .map(|(w, h)| (w.to_string(), h.to_string()))?;
            Some(w.parse::<i64>().ok()?.max(h.parse::<i64>().ok()?))
        })
        .max()
        .unwrap_or(0)
}

// ---------- Minimal HTML-parsing af <link>-tags ----------

#[derive(Debug)]
struct Link {
    rel: String,
    href: String,
    sizes: Option<String>,
}

impl Link {
    fn has_rel(&self, rel: &str) -> bool {
        self.rel
            .split_whitespace()
            .any(|r| r.eq_ignore_ascii_case(rel))
    }
}

fn parse_links(html: &str) -> Vec<Link> {
    // ASCII-lowercase bevarer byte-positioner, så indeks kan bruges på begge strenge.
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<link") {
        let start = pos + i + "<link".len();
        let end = lower[start..].find('>').map_or(lower.len(), |e| start + e);
        let attrs = parse_attrs(&html[start..end]);
        if let (Some(rel), Some(href)) = (attrs.get("rel"), attrs.get("href")) {
            out.push(Link {
                rel: rel.clone(),
                href: decode_entities(href),
                sizes: attrs.get("sizes").cloned(),
            });
        }
        pos = end;
    }
    out
}

fn parse_attrs(s: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let mut chars = s.char_indices().peekable();
    loop {
        while chars
            .next_if(|(_, c)| c.is_whitespace() || *c == '/')
            .is_some()
        {}
        let Some(&(name_start, _)) = chars.peek() else {
            break;
        };
        let mut name_end = s.len();
        while let Some(&(i, c)) = chars.peek() {
            if c.is_whitespace() || c == '=' || c == '/' {
                name_end = i;
                break;
            }
            chars.next();
        }
        let name = s[name_start..name_end].to_ascii_lowercase();
        while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}
        let mut value = String::new();
        if chars.next_if(|(_, c)| *c == '=').is_some() {
            while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}
            match chars.peek().map(|&(_, c)| c) {
                Some(q @ ('"' | '\'')) => {
                    chars.next();
                    for (_, c) in chars.by_ref() {
                        if c == q {
                            break;
                        }
                        value.push(c);
                    }
                }
                _ => {
                    while let Some((_, c)) = chars.next_if(|(_, c)| !c.is_whitespace()) {
                        value.push(c);
                    }
                }
            }
        }
        if !name.is_empty() {
            attrs.entry(name).or_insert(value);
        }
    }
    attrs
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

// ---------- Farver ----------

/// Parser "#rgb", "#rrggbb" og "#rrggbbaa".
pub fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.trim().strip_prefix('#')?;
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    match digits.len() {
        3 => Some((digits[0] * 17, digits[1] * 17, digits[2] * 17)),
        6 | 8 => Some((
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_manifest_and_icons_in_html() {
        let html = r#"<!doctype html><html><head>
            <LINK rel="manifest" href="/manifest.webmanifest">
            <link rel='icon' type="image/png" sizes="32x32" href="/fav-32.png"/>
            <link rel=apple-touch-icon sizes=180x180 href=/apple.png>
            <link rel="stylesheet" href="/a.css?x=1&amp;y=2">
        </head></html>"#;
        let links = parse_links(html);
        assert_eq!(links.len(), 4);
        assert!(links[0].has_rel("manifest"));
        assert_eq!(links[0].href, "/manifest.webmanifest");
        assert_eq!(links[1].sizes.as_deref(), Some("32x32"));
        assert!(links[2].has_rel("apple-touch-icon"));
        assert_eq!(links[2].href, "/apple.png");
        assert_eq!(links[3].href, "/a.css?x=1&y=2");
    }

    #[test]
    fn prefers_large_any_purpose_icons() {
        let manifest: WebManifest = serde_json::from_str(
            r#"{"icons":[
                {"src":"m512.png","sizes":"512x512","purpose":"maskable"},
                {"src":"i48.png","sizes":"48x48"},
                {"src":"i192.png","sizes":"192x192","purpose":"any maskable"},
                {"src":"i512.png","sizes":"512x512"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(
            rank_icons(&manifest.icons),
            vec!["i192.png", "i512.png", "i48.png", "m512.png"]
        );
    }

    #[test]
    fn parses_sizes_and_colors() {
        assert_eq!(largest_size(Some("16x16 256x256 48x48")), 256);
        assert_eq!(largest_size(Some("any")), 512);
        assert_eq!(largest_size(None), 0);
        assert_eq!(parse_hex_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("#0f172a"), Some((15, 23, 42)));
        assert_eq!(parse_hex_color("rebeccapurple"), None);
    }
}
