// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Writes the unit files of a sample timer into a folder, to check them with
//! `systemd-analyze --user verify`:
//!
//! ```sh
//! cargo run -p tempo-core --example render -- /tmp/tempo-units /usr/bin/true
//! ```

use std::path::PathBuf;

use tempo_core::schedule::{Schedule, Weekday};
use tempo_core::spec::{Command, Condition, TimerSpec};
use tempo_core::unit::{render_service, render_timer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("usage: render <folder> <program>")?);
    let program = PathBuf::from(args.next().ok_or("usage: render <folder> <program>")?);
    std::fs::create_dir_all(&dir)?;

    let command = Command {
        program,
        args: vec!["my files".to_owned(), "50%".to_owned(), "$HOME".to_owned()],
        working_dir: Some(PathBuf::from("/tmp")),
    };
    let schedules = [
        ("every", Schedule::Every { minutes: 30 }),
        ("hourly", Schedule::Hourly { minute: 5 }),
        (
            "daily",
            Schedule::Daily {
                hour: 9,
                minute: 30,
            },
        ),
        (
            "weekly",
            Schedule::Weekly {
                days: vec![Weekday::Sat, Weekday::Mon],
                hour: 2,
                minute: 15,
            },
        ),
        (
            "monthly",
            Schedule::Monthly {
                day: 28,
                hour: 3,
                minute: 0,
            },
        ),
    ];
    for (name, schedule) in schedules {
        let spec = TimerSpec {
            name: format!("tempo-sample-{name}"),
            description: "Sample, 50% made up".to_owned(),
            command: command.clone(),
            catch_up: schedule.is_calendar(),
            schedule,
            random_delay_secs: 30,
            conditions: vec![
                Condition::ProcessNotRunning("stremio".to_owned()),
                Condition::OnAcPower,
                Condition::PathExists(PathBuf::from("/tmp")),
            ],
            low_priority: true,
            timeout_secs: Some(600),
            notify_failure: true,
        };
        std::fs::write(dir.join(spec.timer_unit()), render_timer(&spec)?)?;
        std::fs::write(dir.join(spec.service_unit()), render_service(&spec)?)?;
    }
    println!("written in {}", dir.display());
    Ok(())
}
