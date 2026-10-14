//! App-listen i `apps.json` i brugerens config-mappe.
//!
//! Første gang oprettes filen med standardlisten fra repo-roden. Derefter læses
//! den frisk ved hver brug, så ændringer slår igennem uden genstart eller ny build.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Url;

/// Standardlisten, som kopieres ud første gang.
const DEFAULT_APPS_JSON: &str = include_str!("../../apps.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEntry {
    /// Kort, stabilt id: små bogstaver, tal, `-` og `_`. Bruges til vindue og data-mappe,
    /// så appens storage følger id'et – omdøb det ikke, hvis data skal bevares.
    pub id: String,
    pub name: String,
    /// Production-adressen, som appen åbner på.
    pub url: Url,
    /// Valgfri preview-adresse (fx `https://calcplus-git-dev-thyxo.vercel.app/`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<Url>,
}

pub struct AppsFile {
    path: PathBuf,
    /// Serialiserer læs-ret-skriv, så to samtidige tilføjelser ikke overskriver hinanden.
    write_lock: Mutex<()>,
}

impl AppsFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            write_lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opretter filen med standardlisten, hvis den ikke findes.
    pub fn ensure_exists(&self) -> std::io::Result<()> {
        if self.path.exists() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.path, DEFAULT_APPS_JSON)
    }

    /// Alle apps, eller en læsbar fejl hvis filen mangler eller er ugyldig.
    pub fn load(&self) -> Result<Vec<AppEntry>, String> {
        let json = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("Kan ikke læse {}: {e}", self.path.display()))?;
        parse(&json).map_err(|e| format!("{e}\n({})", self.path.display()))
    }

    pub fn find(&self, id: &str) -> Result<AppEntry, String> {
        self.load()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("Ukendt app: {id} – er den fjernet fra apps.json?"))
    }

    /// Tilføjer en app ud fra en adresse. Id dannes af domænet; navnet er
    /// `name` (fx fra appens manifest) eller domænet med stort forbogstav.
    pub fn add(
        &self,
        url: &str,
        name: Option<&str>,
        preview: Option<&str>,
    ) -> Result<AppEntry, String> {
        let url = normalize_url(url)?;
        let preview = preview
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(normalize_url)
            .transpose()?;

        let _guard = self.write_lock.lock().unwrap();
        let mut apps = self.load_or_empty()?;
        if let Some(existing) = apps.iter().find(|a| same_address(&a.url, &url)) {
            return Err(format!("{} er allerede tilføjet", existing.name));
        }
        let id = unique_id(&base_id(&url), &apps);
        let name = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_name(&url));
        let entry = AppEntry {
            id,
            name,
            url,
            preview,
        };
        apps.push(entry.clone());
        self.save(&apps)?;
        Ok(entry)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _guard = self.write_lock.lock().unwrap();
        let mut apps = self.load()?;
        let before = apps.len();
        apps.retain(|a| a.id != id);
        if apps.len() == before {
            return Err(format!("Ukendt app: {id}"));
        }
        self.save(&apps)
    }

    /// Som `load`, men en manglende fil giver en tom liste.
    fn load_or_empty(&self) -> Result<Vec<AppEntry>, String> {
        if self.path.exists() {
            self.load()
        } else {
            Ok(Vec::new())
        }
    }

    fn save(&self, apps: &[AppEntry]) -> Result<(), String> {
        let write = || -> std::io::Result<()> {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut json = serde_json::to_string_pretty(apps)?;
            json.push('\n');
            let tmp = self.path.with_extension("json.tmp");
            std::fs::write(&tmp, json)?;
            std::fs::rename(tmp, &self.path)
        };
        write().map_err(|e| format!("Kunne ikke gemme {}: {e}", self.path.display()))
    }
}

/// "monizzz.vercel.app" → "https://monizzz.vercel.app/".
pub fn normalize_url(input: &str) -> Result<Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Indsæt en adresse".into());
    }
    let with_scheme = if input.contains("://") {
        input.to_string()
    } else {
        // Lokale udviklingsservere kører typisk uden TLS.
        let host = input.split(['/', ':']).next().unwrap_or("");
        let local = host == "localhost" || host.parse::<std::net::Ipv4Addr>().is_ok();
        format!("{}://{input}", if local { "http" } else { "https" })
    };
    let url =
        Url::parse(&with_scheme).map_err(|_| format!("\"{input}\" er ikke en gyldig adresse"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!("\"{input}\" skal være en http(s)-adresse"));
    }
    Ok(url)
}

fn same_address(a: &Url, b: &Url) -> bool {
    let norm = |u: &Url| {
        format!(
            "{}{}",
            u.origin().ascii_serialization(),
            u.path().trim_end_matches('/')
        )
    };
    norm(a) == norm(b)
}

/// Første del af domænet: "monizzz.vercel.app" → "monizzz", "www.calcplus.dk" → "calcplus".
fn base_id(url: &Url) -> String {
    let host = url.host_str().unwrap_or("app");
    let host = host.strip_prefix("www.").unwrap_or(host);
    let label = match url.host() {
        Some(url::Host::Domain(_)) if host != "localhost" => host.split('.').next().unwrap_or(host),
        _ => host,
    };
    let id: String = label
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        "app".into()
    } else {
        id
    }
}

fn unique_id(base: &str, apps: &[AppEntry]) -> String {
    let taken = |id: &str| apps.iter().any(|a| a.id == id);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .expect("uendelig række")
}

fn default_name(url: &Url) -> String {
    let base = base_id(url);
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => base,
    }
}

fn parse(json: &str) -> Result<Vec<AppEntry>, String> {
    // Notesblok gemmer gerne med BOM.
    let json = json.trim_start_matches('\u{feff}');

    // Tilgivende: flere lister efter hinanden ("[...] [...]") eller enkelte
    // objekter accepteres og lægges sammen. Filen gemmes pænt ved næste ændring.
    let mut apps: Vec<AppEntry> = Vec::new();
    for value in serde_json::Deserializer::from_str(json).into_iter::<serde_json::Value>() {
        let value = value.map_err(|e| format!("apps.json kan ikke læses: {e}"))?;
        let chunk: Vec<AppEntry> = if value.is_array() {
            serde_json::from_value(value)
        } else {
            serde_json::from_value(value).map(|a| vec![a])
        }
        .map_err(|e| format!("apps.json kan ikke læses: {e}"))?;
        apps.extend(chunk);
    }

    let mut seen = HashSet::new();
    for app in &apps {
        let valid_id = !app.id.is_empty()
            && app
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !valid_id {
            return Err(format!(
                "apps.json: id \"{}\" må kun indeholde a-z, 0-9, - og _",
                app.id
            ));
        }
        if !seen.insert(app.id.as_str()) {
            return Err(format!("apps.json: id \"{}\" står der flere gange", app.id));
        }
        let urls = std::iter::once(&app.url).chain(app.preview.as_ref());
        if urls
            .into_iter()
            .any(|u| !matches!(u.scheme(), "http" | "https"))
        {
            return Err(format!(
                "apps.json: \"{}\" skal have http(s)-adresser",
                app.id
            ));
        }
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_apps_json_is_valid() {
        let apps = parse(DEFAULT_APPS_JSON).expect("apps.json skal være gyldig");
        assert!(!apps.is_empty());
    }

    #[test]
    fn file_is_created_and_reread() {
        let dir = std::env::temp_dir().join(format!("pwa-launcher-apps-{}", std::process::id()));
        let file = AppsFile::new(dir.join("apps.json"));
        file.ensure_exists().unwrap();
        assert_eq!(
            file.load().unwrap().len(),
            parse(DEFAULT_APPS_JSON).unwrap().len()
        );

        // Ændringer slår igennem med det samme – også med BOM fra Notesblok.
        std::fs::write(
            file.path(),
            "\u{feff}[{\"id\":\"ny\",\"name\":\"Ny\",\"url\":\"https://ny.dk\"}]",
        )
        .unwrap();
        assert_eq!(file.find("ny").unwrap().name, "Ny");
        assert!(file.find("stopwatchplus").is_err());

        // ensure_exists overskriver ikke brugerens fil.
        file.ensure_exists().unwrap();
        assert!(file.find("ny").is_ok());

        std::fs::write(file.path(), "[{").unwrap();
        assert!(file.load().unwrap_err().contains("apps.json"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_bad_entries() {
        let dup = r#"[{"id":"a","name":"A","url":"https://a.dk"},{"id":"a","name":"B","url":"https://b.dk"}]"#;
        assert!(parse(dup).unwrap_err().contains("flere gange"));

        let bad_id = r#"[{"id":"Penge Trick","name":"P","url":"https://p.dk"}]"#;
        assert!(parse(bad_id).unwrap_err().contains("a-z"));

        let bad_url = r#"[{"id":"x","name":"X","url":"ftp://x.dk"}]"#;
        assert!(parse(bad_url).unwrap_err().contains("http"));

        let bad_preview = r#"[{"id":"x","name":"X","url":"https://x.dk","preview":"file:///x"}]"#;
        assert!(parse(bad_preview).unwrap_err().contains("http"));

        let ok = r#"[{"id":"x","name":"X","url":"https://x.dk","preview":"https://x-git-dev.vercel.app"}]"#;
        assert!(parse(ok).unwrap()[0].preview.is_some());

        let typo = r#"[{"id":"x","name":"X","ulr":"https://x.dk"}]"#;
        assert!(parse(typo).is_err());
    }

    #[test]
    fn accepts_several_lists_in_a_row() {
        // Præcis sådan en fil, som man nemt får ved at indsætte to eksempler.
        let json = r#"[
  { "id": "stopwatchplus", "name": "Stopwatch+", "url": "https://stopwatchplus.vercel.app/" }
]

[
  { "id": "monizzz", "name": "Monizzz", "url": "https://monizzz.vercel.app/" }
]"#;
        let apps = parse(json).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[1].id, "monizzz");

        let single = r#"{ "id": "a", "name": "A", "url": "https://a.dk" }"#;
        assert_eq!(parse(single).unwrap().len(), 1);
        assert!(parse("[] [").is_err());
    }

    #[test]
    fn add_and_remove_from_a_link() {
        let dir = std::env::temp_dir().join(format!("pwa-launcher-add-{}", std::process::id()));
        let file = AppsFile::new(dir.join("apps.json"));

        let a = file.add("monizzz.vercel.app", None, None).unwrap();
        assert_eq!(a.id, "monizzz");
        assert_eq!(a.name, "Monizzz");
        assert_eq!(a.url.as_str(), "https://monizzz.vercel.app/");

        // Samme adresse igen afvises; samme første domænedel får nyt id.
        assert!(file
            .add("https://monizzz.vercel.app", None, None)
            .unwrap_err()
            .contains("allerede"));
        let b = file
            .add(
                "https://monizzz.dk/",
                Some("Monizzz DK"),
                Some("monizzz-git-dev.vercel.app"),
            )
            .unwrap();
        assert_eq!(b.id, "monizzz-2");
        assert_eq!(b.name, "Monizzz DK");
        assert!(b.preview.is_some());

        assert_eq!(file.load().unwrap().len(), 2);
        file.remove("monizzz").unwrap();
        assert_eq!(file.load().unwrap()[0].id, "monizzz-2");
        assert!(file.remove("findes-ikke").is_err());

        assert!(file.add("   ", None, None).is_err());
        assert!(file.add("ftp://x.dk", None, None).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ids_from_addresses() {
        let id = |s: &str| base_id(&normalize_url(s).unwrap());
        assert_eq!(id("https://www.calcplus.dk/app"), "calcplus");
        assert_eq!(id("penge-trick.vercel.app"), "penge-trick");
        assert_eq!(id("http://localhost:5173"), "localhost");
        assert_eq!(id("http://127.0.0.1:8766/"), "127-0-0-1");
        assert_eq!(
            normalize_url("localhost:5173").unwrap().as_str(),
            "http://localhost:5173/"
        );
        assert_eq!(normalize_url("192.168.1.20:3000").unwrap().scheme(), "http");
        assert_eq!(
            normalize_url("monizzz.vercel.app").unwrap().scheme(),
            "https"
        );
    }
}
