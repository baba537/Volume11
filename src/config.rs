//! Persisted configuration.
//!
//! The old version used EF Core + SQLite for what amounts to a few dozen key/value
//! pairs, which cost a native dependency and a noticeable chunk of startup time.
//! This is a plain JSON file, written atomically.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Bumped only when the on-disk shape changes incompatibly.
const CURRENT_VERSION: u32 = 1;

/// What happens to an application that starts playing and has no saved entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UnknownAppPolicy {
    /// Leave it alone. The old version silently forced every new app to 20 %,
    /// which was surprising; this is the sane default.
    #[default]
    LeaveAlone,
    /// Apply [`Settings::default_volume`].
    ApplyDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// 0..=100.
    pub volume: u8,
    pub muted: bool,
    /// Label last seen for this executable, so the settings list stays readable
    /// even when the application is not running.
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Re-apply the saved level automatically when an application starts playing.
    pub auto_apply: bool,
    pub unknown_app_policy: UnknownAppPolicy,
    pub default_volume: u8,
    pub always_on_top: bool,
    pub start_with_windows: bool,
    /// Accent colour as `#rrggbb`.
    pub accent: String,
    /// Pure black background for OLED panels.
    pub oled_black: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_apply: true,
            unknown_app_policy: UnknownAppPolicy::default(),
            default_volume: 50,
            always_on_top: false,
            start_with_windows: false,
            accent: "#9AA3AD".to_string(),
            oled_black: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    /// Keyed by lower-cased executable name (`firefox.exe`) or `@master` / `@system`.
    /// A `BTreeMap` keeps the file diff-friendly across saves.
    pub apps: BTreeMap<String, AppEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            settings: Settings::default(),
            apps: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn get(&self, key: &str) -> Option<&AppEntry> {
        self.apps.get(key)
    }

    pub fn set(&mut self, key: &str, volume: u8, muted: bool, label: &str) {
        self.apps.insert(
            key.to_string(),
            AppEntry {
                volume: volume.min(100),
                muted,
                label: label.to_string(),
            },
        );
    }

    pub fn remove(&mut self, key: &str) {
        self.apps.remove(key);
    }

    /// Everything saved, in the shape the audio engine expects for `ApplyAll`.
    pub fn as_apply_list(&self) -> Vec<(String, u8, bool)> {
        self.apps
            .iter()
            .map(|(key, entry)| (key.clone(), entry.volume, entry.muted))
            .collect()
    }

    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        // Notepad and PowerShell's `Out-File -Encoding utf8` both prepend a byte
        // order mark, which serde_json rejects as a syntax error. Editing the
        // file by hand must not silently cost the user their settings.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

        match serde_json::from_str::<Config>(text) {
            Ok(config) => config,
            Err(error) => {
                // A corrupt file must not cost the user their whole setup silently,
                // so keep a copy next to it before starting over.
                let backup = path.with_extension("json.broken");
                let _ = std::fs::rename(path, &backup);
                eprintln!("config unreadable ({error}), moved to {}", backup.display());
                Self::default()
            }
        }
    }

    /// Write via a temporary file and rename, so a crash mid-write cannot leave
    /// a truncated configuration behind.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let text = serde_json::to_string_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;

        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, path)?;

        Ok(())
    }
}

/// `%APPDATA%\Volume11\config.json`, unless `VOLUME11_CONFIG` names another file.
///
/// The override makes portable installs possible (keep the configuration next to
/// the executable on a stick) and lets tests run against a throwaway file.
///
/// The old version wrote to `%LOCALAPPDATA%\App1\`, a leftover placeholder name.
pub fn config_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("VOLUME11_CONFIG")
        && !override_path.trim().is_empty()
    {
        return PathBuf::from(override_path);
    }

    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    base.join("Volume11").join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut config = Config::default();
        config.set("firefox.exe", 42, true, "Firefox");
        config.settings.always_on_top = true;

        let text = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&text).unwrap();

        assert_eq!(parsed, config);
        assert_eq!(parsed.get("firefox.exe").unwrap().volume, 42);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.settings, Settings::default());
        assert!(parsed.apps.is_empty());
    }

    #[test]
    fn a_byte_order_mark_is_tolerated() {
        let dir = std::env::temp_dir().join(format!("volume11-bom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        std::fs::write(
            &path,
            "\u{feff}{\"settings\":{\"default_volume\":35},\"apps\":{}}",
        )
        .unwrap();

        let loaded = Config::load(&path);
        assert_eq!(loaded.settings.default_volume, 35);
        // A tolerated file must not be moved aside as broken.
        assert!(!path.with_extension("json.broken").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_fields_do_not_break_loading() {
        let parsed: Config =
            serde_json::from_str(r#"{"version":1,"somethingNew":true,"apps":{}}"#).unwrap();
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn volume_is_clamped_on_write() {
        let mut config = Config::default();
        config.set("x.exe", 250, false, "X");
        assert_eq!(config.get("x.exe").unwrap().volume, 100);
    }

    #[test]
    fn save_then_load_matches() {
        let dir = std::env::temp_dir().join(format!("volume11-test-{}", std::process::id()));
        let path = dir.join("config.json");

        let mut config = Config::default();
        config.set("vlc.exe", 30, false, "VLC");
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        assert_eq!(loaded, config);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
