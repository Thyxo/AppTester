//! App-listen. Redigeres i `apps.json` i repo-roden og bygges ind i programmet.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::Url;

const APPS_JSON: &str = include_str!("../../apps.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEntry {
    /// Kort, stabilt id: små bogstaver, tal, `-` og `_`. Bruges til vindue og data-mappe,
    /// så appens storage følger id'et – omdøb det ikke, hvis data skal bevares.
    pub id: String,
    pub name: String,
    pub url: Url,
}

/// Alle apps fra `apps.json`, eller en læsbar fejl hvis filen er ugyldig.
pub fn all() -> Result<&'static [AppEntry], String> {
    static APPS: OnceLock<Result<Vec<AppEntry>, String>> = OnceLock::new();
    APPS.get_or_init(|| parse(APPS_JSON))
        .as_deref()
        .map_err(Clone::clone)
}

pub fn find(id: &str) -> Result<&'static AppEntry, String> {
    all()?
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("Ukendt app: {id}"))
}

fn parse(json: &str) -> Result<Vec<AppEntry>, String> {
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
        if !matches!(app.url.scheme(), "http" | "https") {
            return Err(format!(
                "apps.json: \"{}\" skal have en http(s)-adresse",
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
        let apps = parse(APPS_JSON).expect("apps.json skal være gyldig");
        assert!(!apps.is_empty());
    }

    #[test]
    fn rejects_bad_entries() {
        let dup = r#"[{"id":"a","name":"A","url":"https://a.dk"},{"id":"a","name":"B","url":"https://b.dk"}]"#;
        assert!(parse(dup).unwrap_err().contains("flere gange"));

        let bad_id = r#"[{"id":"Penge Trick","name":"P","url":"https://p.dk"}]"#;
        assert!(parse(bad_id).unwrap_err().contains("a-z"));

        let bad_url = r#"[{"id":"x","name":"X","url":"ftp://x.dk"}]"#;
        assert!(parse(bad_url).unwrap_err().contains("http"));

        let typo = r#"[{"id":"x","name":"X","ulr":"https://x.dk"}]"#;
        assert!(parse(typo).is_err());
    }
}
