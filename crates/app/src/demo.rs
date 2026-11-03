// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Made-up timers for the preview and the screenshots. Nothing here is read
//! from the computer: names, paths and results are invented.

use gettextrs::gettext;
use tempo_core::schedule::{Schedule, Weekday};

use crate::journal::Entry;
use crate::view::{Origin, Run, RunResult, State, TimerView};

/// Builds a history from letters, oldest first: `s` succeeded, `f` failed,
/// `k` skipped. Runs are `every_secs` apart and the last one `last_ago` ago.
fn history(letters: &str, every_secs: i64, last_ago: i64) -> Vec<Run> {
    let count = letters.chars().count() as i64;
    letters
        .chars()
        .enumerate()
        .map(|(index, letter)| {
            let result = match letter {
                'f' => RunResult::Failed,
                'k' => RunResult::Skipped,
                _ => RunResult::Success,
            };
            Run {
                result,
                ago_secs: last_ago + (count - 1 - index as i64) * every_secs,
                duration_secs: 2 + (index as u32 * 7) % 9,
                exit_code: match result {
                    RunResult::Success => Some(0),
                    RunResult::Failed => Some(1),
                    RunResult::Skipped | RunResult::Unknown => None,
                },
            }
        })
        .collect()
}

pub fn timers() -> Vec<TimerView> {
    let home = "/home/user";
    let user_unit = |name: &str| format!("{home}/.config/systemd/user/{name}");
    let system_unit = |name: &str| format!("/usr/lib/systemd/user/{name}");
    vec![
        TimerView {
            unit: "clean-app-cache.timer".to_owned(),
            service: "clean-app-cache.service".to_owned(),
            title: gettext("Clean app cache"),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Every { minutes: 30 },
            next_in_secs: Some(12 * 60),
            runs: history("ssssksssss", 1_800, 18 * 60),
            command: format!("{home}/.local/bin/clean-app-cache.sh"),
            file: user_unit("clean-app-cache.timer"),
        },
        TimerView {
            unit: "sync-notes.timer".to_owned(),
            service: "sync-notes.service".to_owned(),
            title: gettext("Sync notes"),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Hourly { minute: 5 },
            next_in_secs: Some(41 * 60),
            runs: history("sssssssssf", 3_600, 3 * 60),
            command: format!("{home}/.local/bin/sync-notes.sh --quiet"),
            file: user_unit("sync-notes.timer"),
        },
        TimerView {
            unit: "index-photos.timer".to_owned(),
            service: "index-photos.service".to_owned(),
            title: gettext("Index photos"),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Running,
            enabled: true,
            schedule: Schedule::Every { minutes: 120 },
            next_in_secs: Some(2 * 3_600),
            runs: history("sssssssss", 7_200, 2 * 3_600),
            command: format!("{home}/.local/bin/index-photos"),
            file: user_unit("index-photos.timer"),
        },
        TimerView {
            unit: "clean-downloads.timer".to_owned(),
            service: "clean-downloads.service".to_owned(),
            title: gettext("Clean downloads folder"),
            description: String::new(),
            origin: Origin::Personal,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Daily { hour: 9, minute: 0 },
            next_in_secs: Some(17 * 3_600),
            runs: history("ssssssss", 86_400, 7 * 3_600),
            command: format!("{home}/.local/bin/clean-downloads"),
            file: user_unit("clean-downloads.timer"),
        },
        TimerView {
            unit: "back-up-documents.timer".to_owned(),
            service: "back-up-documents.service".to_owned(),
            title: gettext("Back up documents"),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Weekly {
                days: vec![Weekday::Sat],
                hour: 2,
                minute: 15,
            },
            next_in_secs: Some(2 * 86_400 + 5 * 3_600),
            runs: history("ssssssss", 604_800, 5 * 86_400),
            command: "/usr/bin/rsync -a /home/user/Documents/ /mnt/backup/".to_owned(),
            file: user_unit("back-up-documents.timer"),
        },
        TimerView {
            unit: "daily-weather-report.timer".to_owned(),
            service: "daily-weather-report.service".to_owned(),
            title: gettext("Daily weather report"),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Paused,
            enabled: true,
            schedule: Schedule::Daily {
                hour: 7,
                minute: 30,
            },
            next_in_secs: None,
            runs: history("sssssk", 86_400, 2 * 86_400),
            command: format!("{home}/.local/bin/weather-report"),
            file: user_unit("daily-weather-report.timer"),
        },
        TimerView {
            unit: "snap.firmware-updater.firmware-notifier.timer".to_owned(),
            service: "snap.firmware-updater.firmware-notifier.service".to_owned(),
            title: gettext("Firmware update check"),
            description: String::new(),
            origin: Origin::System,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Custom {
                expression: "*-*-* 00,06,12,18:00:00".to_owned(),
            },
            next_in_secs: Some(3 * 3_600),
            runs: history("ssssssssss", 21_600, 3 * 3_600),
            command: "/usr/bin/snap run firmware-updater.firmware-notifier".to_owned(),
            file: system_unit("snap.firmware-updater.firmware-notifier.timer"),
        },
        TimerView {
            unit: "launchpadlib-cache-clean.timer".to_owned(),
            service: "launchpadlib-cache-clean.service".to_owned(),
            title: gettext("Launchpad cache cleanup"),
            description: String::new(),
            origin: Origin::System,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Custom {
                expression: "daily".to_owned(),
            },
            next_in_secs: Some(22 * 3_600),
            runs: history("ssssssssss", 86_400, 2 * 3_600),
            command: "/usr/bin/python3 /usr/share/launchpadlib/cache-clean".to_owned(),
            file: system_unit("launchpadlib-cache-clean.timer"),
        },
        TimerView {
            unit: "ubuntu-insights-upload.timer".to_owned(),
            service: "ubuntu-insights-upload.service".to_owned(),
            title: gettext("Usage statistics upload"),
            description: String::new(),
            origin: Origin::System,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Custom {
                expression: "*-*-* *:00/30:00".to_owned(),
            },
            next_in_secs: None,
            runs: history("ssssksssss", 1_800, 63 * 60),
            command: "/usr/bin/ubuntu-insights upload".to_owned(),
            file: system_unit("ubuntu-insights-upload.timer"),
        },
    ]
}

/// Made-up log lines for a timer, from its runs: what systemd writes when a
/// run starts and ends, and what the program prints in between.
pub fn logs(timer: &TimerView) -> Vec<Entry> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_micros()).unwrap_or(0));
    let service = &timer.service;
    let line = |ago: i64, priority: u8, message: String| Entry {
        time_usec: now.saturating_sub(u64::try_from(ago.max(0)).unwrap_or(0) * 1_000_000),
        priority,
        message,
        unit: service.clone(),
        ..Entry::default()
    };
    let mut lines = Vec::new();
    for run in &timer.runs {
        let start = run.ago_secs + i64::from(run.duration_secs);
        lines.push(line(start, 6, format!("Starting {service}...")));
        match run.result {
            RunResult::Success => {
                lines.push(line(start - 1, 6, "Done: 128 items processed".to_owned()));
                lines.push(line(run.ago_secs, 6, format!("Finished {service}.")));
            }
            RunResult::Failed => {
                lines.push(line(
                    start - 1,
                    6,
                    "ERROR: could not reach notes.example.com (timeout after 30 s)".to_owned(),
                ));
                lines.push(line(
                    run.ago_secs,
                    5,
                    format!("{service}: Main process exited, code=exited, status=1/FAILURE"),
                ));
                lines.push(line(
                    run.ago_secs,
                    4,
                    format!("{service}: Failed with result 'exit-code'."),
                ));
                lines.push(line(run.ago_secs, 3, format!("Failed to start {service}.")));
            }
            RunResult::Skipped | RunResult::Unknown => {
                lines.push(line(
                    run.ago_secs,
                    6,
                    format!("{service} skipped, unmet condition check."),
                ));
            }
        }
    }
    lines.sort_by_key(|entry| entry.time_usec);
    lines
}

/// Two made-up files for the review page: one with points to look at, one a
/// timer cannot run.
pub fn reviews() -> [(std::path::PathBuf, tempo_core::analysis::Analysis); 2] {
    use std::path::{Path, PathBuf};
    use tempo_core::analysis::{Env, FileFacts, analyze};

    let exists = |path: &Path| path == Path::new("/bin/bash");
    let find = |name: &str| (name == "bash").then(|| PathBuf::from("/usr/bin/bash"));
    let env = Env {
        home: Path::new("/home/user"),
        arch: "x86_64",
        exists: &exists,
        find: &find,
    };
    let file = |path: &str, mode: u32, text: &str| {
        let path = PathBuf::from(path);
        let facts = FileFacts {
            path: path.clone(),
            real_path: path.clone(),
            is_dir: false,
            size: text.len() as u64,
            mode,
            head: text.as_bytes().to_vec(),
        };
        (path, analyze(&facts, &env))
    };
    [
        file(
            "/home/user/Downloads/clean-cache.sh",
            0o644,
            "#!/bin/bash\ncd \"$HOME/.cache/app\"\nsource ./settings.sh\nsudo systemctl restart app\nnotify-send \"Cache cleaned\"\nrm -rf \"$CACHE_DIR\"/*\n",
        ),
        file(
            "/home/user/bin/sync-notes.sh",
            0o755,
            "#!/bin/bash\r\necho syncing\r\n",
        ),
    ]
}
