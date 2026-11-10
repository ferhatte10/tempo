// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Update notices. Tempo never replaces its own files: it tells the owner that
//! a new version exists and how to get it, which depends on how it was
//! installed:
//!
//! - by the website's `install.sh` (in `~/.local/share/tempo`): run the same
//!   command again;
//! - from a `.deb` or `.rpm` (`/usr/bin`): download the new package;
//! - built from source: nothing (the developer pulls the code).
//!
//! The first two learn about new versions from the website's `latest.json`, at
//! most once a day, and only while the switch in the main menu allows it.
//! A version installed while the old one still runs asks for a restart.

use std::path::Path;
use std::sync::OnceLock;

use gtk::glib;
use gtk::glib::translate::IntoGlib;
use soup::prelude::*;

const LATEST_URL: &str = "https://tempo.ferhatsn.fr/latest.json";
/// Where a package finds its newer version.
pub const DOWNLOAD_URL: &str = "https://tempo.ferhatsn.fr/install/";
/// What updates an install made by `install.sh`.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh";
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// Unpacked in the home folder by the website's `install.sh`.
    Home,
    /// A `.deb` or `.rpm`.
    Package,
    Source,
}

impl Install {
    /// How this copy of Tempo was installed. `TEMPO_INSTALL` (`home`,
    /// `package`, `source`) overrides it, to try the notices from a
    /// development build.
    pub fn current() -> Self {
        static INSTALL: OnceLock<Install> = OnceLock::new();
        *INSTALL.get_or_init(|| {
            let forced = std::env::var("TEMPO_INSTALL").ok();
            match forced.as_deref() {
                Some("home") => return Install::Home,
                Some("package") => return Install::Package,
                Some("source") => return Install::Source,
                _ => {}
            }
            if cfg!(debug_assertions) {
                return Install::Source;
            }
            let data = std::env::var_os("XDG_DATA_HOME")
                .filter(|dir| !dir.is_empty())
                .map(std::path::PathBuf::from)
                .or_else(|| crate::systemd::home_dir().map(|home| home.join(".local/share")));
            match std::env::current_exe() {
                Ok(exe) => classify(&exe, data.as_deref()),
                Err(_) => Install::Source,
            }
        })
    }

    /// True when new versions are learned from the website's `latest.json`.
    pub fn checks_website(self) -> bool {
        matches!(self, Install::Home | Install::Package)
    }
}

/// `install.sh` unpacks in `<data>/tempo`; packages put Tempo in `/usr/bin`.
fn classify(exe: &Path, data: Option<&Path>) -> Install {
    if data.is_some_and(|data| exe.starts_with(data.join("tempo"))) {
        Install::Home
    } else if exe.starts_with("/usr/bin") {
        Install::Package
    } else {
        Install::Source
    }
}

/// One HTTP session for the whole app.
pub fn new_session() -> soup::Session {
    soup::Session::builder()
        .timeout(10)
        .user_agent(concat!("Tempo/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// The newest version the website announces.
pub async fn fetch_latest(session: &soup::Session) -> Result<String, String> {
    let url = std::env::var("TEMPO_UPDATE_URL").unwrap_or_else(|_| LATEST_URL.to_owned());
    let message = soup::Message::new("GET", &url).map_err(|err| err.to_string())?;
    if let Some(headers) = message.request_headers() {
        headers.append("Accept", "application/json");
    }
    let body = session
        .send_and_read_future(&message, glib::Priority::DEFAULT)
        .await
        .map_err(|err| err.to_string())?;
    let status = message.status().into_glib() as u32;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let text = std::str::from_utf8(&body).map_err(|err| err.to_string())?;
    tempo_core::version::latest_version(text).ok_or_else(|| "no version in latest.json".to_owned())
}

/// True when a newer version was installed while this one runs: the running
/// program was replaced, so a restart finishes the update.
pub fn installed_while_running(install: Install) -> bool {
    if install == Install::Source {
        return false;
    }
    let Ok(target) = std::fs::read_link("/proc/self/exe") else {
        return false;
    };
    let target = target.to_string_lossy();
    // Only when a new program took its place, not when it was removed.
    target
        .strip_suffix(" (deleted)")
        .is_some_and(|path| Path::new(path).exists())
}

/// Version and notes (AppStream markup) of the newest release in
/// `CHANGELOG.md`.
pub fn release_notes() -> Option<(String, String)> {
    tempo_core::version::release_notes(CHANGELOG)
}

/// Seconds since 1970, for the once-a-day check.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_method_from_where_the_program_is() {
        let data = Path::new("/home/me/.local/share");
        assert_eq!(
            classify(
                Path::new("/home/me/.local/share/tempo/bin/tempo"),
                Some(data)
            ),
            Install::Home
        );
        assert_eq!(
            classify(Path::new("/usr/bin/tempo"), Some(data)),
            Install::Package
        );
        assert_eq!(
            classify(Path::new("/home/me/tempo/target/release/tempo"), Some(data)),
            Install::Source
        );
        assert_eq!(
            classify(Path::new("/usr/local/bin/tempo"), None),
            Install::Source
        );
        assert!(Install::Home.checks_website() && !Install::Source.checks_website());
    }

    #[test]
    fn whats_new_is_this_version() {
        let (version, _) = release_notes().expect("the changelog has a release");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }
}
