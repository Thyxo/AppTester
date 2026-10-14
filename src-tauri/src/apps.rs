//! App-listen i `apps.json` i brugerens config-mappe.
//!
//! Første gang oprettes filen med standardlisten fra repo-roden. Derefter læses
//! den frisk ved hver brug, så ændringer slår igennem uden genstart eller ny build.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
}

impl AppsFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
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
}

fn parse(json: &str) -> Result<Vec<AppEntry>, String> {
    // Notesblok gemmer gerne med BOM.
    let json = json.trim_start_matches('\u{feff}');
    let apps: Vec<AppEntry> =
        serde_json::from_str(json).map_err(|e| format!("apps.json kan ikke læses: {e}"))?;

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
}
