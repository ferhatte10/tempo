// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The notification of a failed run. When a run of one of Tempo's timers
//! fails, systemd starts the notifier unit (`OnFailure=`), which runs
//! `tempo --notify-failed <service>`: it says which timer failed and why, and
//! a click on the notification opens Tempo. Tempo itself does not need to be
//! running.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::{self, Either};
use gettextrs::gettext;
use gtk::glib;
use zbus::{Connection, Variant, proxy};

use crate::systemd::Systemd;
use crate::words::fill;
use crate::{APP_ID, files};

/// How long the notification waits for a click before the notifier quits.
const WAIT: Duration = Duration::from_secs(15 * 60);

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Variant<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

/// `tempo --notify-failed <service>`.
pub fn run(instance: &str) -> glib::ExitCode {
    match glib::MainContext::default().block_on(notify(instance)) {
        Ok(()) => glib::ExitCode::SUCCESS,
        Err(err) => {
            glib::g_warning!("tempo", "Cannot tell that {instance} failed: {err}");
            glib::ExitCode::FAILURE
        }
    }
}

/// Why the run failed, from systemd's `Result` and the exit status.
fn reason(result: &str, status: i32) -> String {
    match result {
        "exit-code" => fill(
            &gettext("It stopped with the error code {code}."),
            &[("code", &status.to_string())],
        ),
        "timeout" => gettext("It ran longer than its time limit and was stopped."),
        "signal" | "core-dump" => gettext("It was stopped before the end."),
        _ => gettext("It did not finish as expected."),
    }
}

async fn notify(instance: &str) -> Result<(), String> {
    let failed = |err: zbus::Error| err.to_string();
    let name = tempo_core::unit::failed_timer(instance).ok_or("not the name of a service")?;
    let dir = files::user_dir().ok_or("no home folder")?;
    // Only Tempo's own timers: the template can be started by hand with any name.
    let (spec, _) = files::managed(&dir, name)?;
    let systemd = Systemd::connect().await.map_err(failed)?;
    let (result, status) = systemd.last_result(instance).await.unwrap_or_default();

    let conn = Connection::session().await.map_err(failed)?;
    let notifications = NotificationsProxy::new(&conn).await.map_err(failed)?;
    let mut clicks = notifications
        .receive_action_invoked()
        .await
        .map_err(failed)?;
    let mut closes = notifications
        .receive_notification_closed()
        .await
        .map_err(failed)?;
    let summary = fill(
        &gettext("“{title}” failed"),
        &[("title", &spec.description)],
    );
    let body = format!(
        "{} {}",
        reason(&result, status),
        gettext("Open Tempo to read its log.")
    );
    let open = gettext("Open Tempo");
    let hints = HashMap::from([
        ("desktop-entry", Variant::from(APP_ID)),
        ("urgency", Variant::from(1u8)),
    ]);
    let id = notifications
        .notify(
            "Tempo",
            0,
            APP_ID,
            &summary,
            &body,
            &["default", &open],
            hints,
            -1,
        )
        .await
        .map_err(failed)?;

    // The notifier stays a while so a click can open Tempo, then quits.
    let clicked = async {
        loop {
            match future::select(clicks.next(), closes.next()).await {
                Either::Left((Some(signal), _)) => {
                    if signal.args().is_ok_and(|args| args.id == id) {
                        return true;
                    }
                }
                Either::Right((Some(signal), _)) => {
                    if signal.args().is_ok_and(|args| args.id == id) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    };
    let deadline = glib::timeout_future(WAIT);
    futures_util::pin_mut!(clicked, deadline);
    if let Either::Left((true, _)) = future::select(clicked, deadline).await {
        open_tempo();
    }
    Ok(())
}

/// Opens Tempo outside the notifier's own unit, which systemd stops (with
/// everything it started) as soon as the notifier quits.
fn open_tempo() {
    let Ok(program) = std::env::current_exe() else {
        return;
    };
    let started = std::process::Command::new("systemd-run")
        .args(["--user", "--collect", "--quiet", "--"])
        .arg(program)
        .status();
    if let Err(err) = started {
        glib::g_warning!("tempo", "Cannot open Tempo: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_is_in_words() {
        assert!(reason("exit-code", 3).contains('3'));
        assert_ne!(reason("timeout", 0), reason("exit-code", 0));
        assert_eq!(reason("signal", 0), reason("core-dump", 0));
        assert!(!reason("start-limit-hit", 0).is_empty());
    }
}
