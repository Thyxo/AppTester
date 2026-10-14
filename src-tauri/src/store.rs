//! Lokal tilstand: favoritter, senest åbnet og vinduesplacering pr. app.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Logiske koordinater (uafhængige af skærmskalering).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeom {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub favorites: HashSet<String>,
    /// Unix-tid i millisekunder.
    pub last_opened: HashMap<String, u64>,
    pub windows: HashMap<String, WindowGeom>,
    /// Valgt enhed pr. app (fx "iphone-15") og om den er drejet.
    pub devices: HashMap<String, DeviceChoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceChoice {
    pub device: String,
    pub landscape: bool,
}

pub struct Store {
    path: PathBuf,
    state: Mutex<State>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let state = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                // Behold den ødelagte fil, så intet går tabt ved et uheld.
                eprintln!("{} er ugyldig ({err}) – starter forfra", path.display());
                let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                State::default()
            }),
            Err(_) => State::default(),
        };
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.state.lock().unwrap())
    }

    pub fn update<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.state.lock().unwrap();
        let out = f(&mut state);
        if let Err(err) = self.save(&state) {
            eprintln!("Kunne ikke gemme {}: {err}", self.path.display());
        }
        out
    }

    fn save(&self, state: &State) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
        std::fs::rename(tmp, &self.path)
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("pwa-launcher-test-{}", now_ms()));
        let path = dir.join("state.json");

        let store = Store::load(path.clone());
        store.update(|s| {
            s.favorites.insert("calcplus".into());
            s.windows.insert(
                "calcplus".into(),
                WindowGeom {
                    x: 10.0,
                    y: 20.0,
                    width: 400.0,
                    height: 800.0,
                },
            );
        });
        let again = Store::load(path.clone());
        assert!(again.read(|s| s.favorites.contains("calcplus")));
        assert_eq!(again.read(|s| s.windows["calcplus"].width), 400.0);

        std::fs::write(&path, "{ikke json").unwrap();
        let broken = Store::load(path.clone());
        assert!(broken.read(|s| s.favorites.is_empty()));
        assert!(path.with_extension("json.bak").exists());

        std::fs::remove_dir_all(dir).unwrap();
    }
}
