// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! A little app state, in `~/.config/tempo/settings` (`key=value` lines): the
//! update check's switch and memory, and the version that ran last.

use std::io;
use std::path::PathBuf;

const KEY_CHECK_UPDATES: &str = "check-for-updates";
/// Unix time of the last successful `latest.json` check.
const KEY_LAST_CHECK: &str = "last-update-check";
/// A newer version found by the last check, empty when none.
const KEY_AVAILABLE: &str = "update-available";
/// Version that ran last, to say “What's New” once after an update.
const KEY_LAST_VERSION: &str = "last-version";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub check_updates: bool,
    pub last_update_check: i64,
    pub update_available: String,
    pub last_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            check_updates: true,
            last_update_check: 0,
            update_available: String::new(),
            last_version: String::new(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| crate::systemd::home_dir().map(|home| home.join(".config")))?;
    Some(config.join("tempo/settings"))
}

impl Settings {
    /// The saved settings; defaults for anything missing or unreadable.
    pub fn load() -> Self {
        path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Self {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                KEY_CHECK_UPDATES => settings.check_updates = value != "false",
                KEY_LAST_CHECK => settings.last_update_check = value.parse().unwrap_or(0),
                KEY_AVAILABLE => settings.update_available = value.to_owned(),
                KEY_LAST_VERSION => settings.last_version = value.to_owned(),
                _ => {}
            }
        }
        settings
    }

    fn text(&self) -> String {
        format!(
            "{KEY_CHECK_UPDATES}={}\n{KEY_LAST_CHECK}={}\n{KEY_AVAILABLE}={}\n{KEY_LAST_VERSION}={}\n",
            self.check_updates, self.last_update_check, self.update_available, self.last_version
        )
    }

    pub fn save(&self) -> io::Result<()> {
        let path = path().ok_or_else(|| io::Error::other("no home folder"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_settings_read_back() {
        let settings = Settings {
            check_updates: false,
            last_update_check: 1_794_000_000,
            update_available: "0.2.0".to_owned(),
            last_version: "0.1.0".to_owned(),
        };
        assert_eq!(Settings::parse(&settings.text()), settings);
    }

    #[test]
    fn missing_or_damaged_values_fall_back_to_defaults() {
        assert_eq!(Settings::parse(""), Settings::default());
        let settings = Settings::parse("last-update-check=soon\nnonsense\nunknown=1\n");
        assert_eq!(settings, Settings::default());
        assert!(Settings::default().check_updates);
    }
}
