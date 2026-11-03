// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The form of a timer as the owner fills it, and its way to a [`TimerSpec`].
//! Every field is text or a number, like the widgets that hold it; nothing is
//! checked until [`Draft::to_spec`], which says which field is wrong.

use std::path::PathBuf;

use crate::analysis::Suggestion;
use crate::schedule::{Schedule, Weekday};
use crate::shellwords;
use crate::spec::{Command, Condition, SpecError, TimerSpec};

/// The kinds of schedule the form offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    Every,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Custom,
}

impl ScheduleKind {
    pub const ALL: [ScheduleKind; 6] = [
        ScheduleKind::Every,
        ScheduleKind::Hourly,
        ScheduleKind::Daily,
        ScheduleKind::Weekly,
        ScheduleKind::Monthly,
        ScheduleKind::Custom,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Title,
    Command,
    WorkingDir,
    Schedule,
    Spread,
    Timeout,
    NotRunning,
    RequirePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftError {
    pub field: Field,
    /// In English, from the core; the interface may say it its own way.
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    pub title: String,
    /// The program and its arguments, as typed.
    pub command: String,
    pub working_dir: String,
    pub kind: ScheduleKind,
    pub every_minutes: u32,
    pub hour: u8,
    pub minute: u8,
    pub days: Vec<Weekday>,
    pub month_day: u8,
    pub expression: String,
    pub catch_up: bool,
    pub spread_minutes: u32,
    pub low_priority: bool,
    pub timeout_minutes: Option<u32>,
    pub not_running: String,
    pub ac_power: bool,
    pub require_path: String,
}

impl Default for Draft {
    fn default() -> Self {
        Draft {
            name: String::new(),
            title: String::new(),
            command: String::new(),
            working_dir: String::new(),
            kind: ScheduleKind::Daily,
            every_minutes: 30,
            hour: 9,
            minute: 0,
            days: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            month_day: 1,
            expression: String::new(),
            catch_up: true,
            spread_minutes: 0,
            low_priority: false,
            timeout_minutes: None,
            not_running: String::new(),
            ac_power: false,
            require_path: String::new(),
        }
    }
}

fn path_text(path: &std::path::Path) -> String {
    path.display().to_string()
}

impl Draft {
    /// A new timer for what the analysis of a file found.
    pub fn from_suggestion(suggestion: &Suggestion) -> Draft {
        let mut words = vec![path_text(&suggestion.command.program)];
        words.extend(suggestion.command.args.iter().cloned());
        Draft {
            name: suggestion.name.clone(),
            title: suggestion.description.clone(),
            command: shellwords::join(&words),
            working_dir: suggestion
                .command
                .working_dir
                .as_deref()
                .map(path_text)
                .unwrap_or_default(),
            ..Draft::default()
        }
    }

    /// The form of a timer Tempo wrote, to change it.
    pub fn from_spec(spec: &TimerSpec) -> Draft {
        let mut draft = Draft {
            name: spec.name.clone(),
            title: spec.description.clone(),
            working_dir: spec
                .command
                .working_dir
                .as_deref()
                .map(path_text)
                .unwrap_or_default(),
            catch_up: spec.catch_up,
            spread_minutes: spec.random_delay_secs / 60,
            low_priority: spec.low_priority,
            timeout_minutes: spec.timeout_secs.map(|secs| secs.div_ceil(60)),
            ..Draft::default()
        };
        let mut words = vec![path_text(&spec.command.program)];
        words.extend(spec.command.args.iter().cloned());
        draft.command = shellwords::join(&words);
        match &spec.schedule {
            Schedule::Every { minutes } => {
                draft.kind = ScheduleKind::Every;
                draft.every_minutes = *minutes;
            }
            Schedule::Hourly { minute } => {
                draft.kind = ScheduleKind::Hourly;
                draft.minute = *minute;
            }
            Schedule::Daily { hour, minute } => {
                draft.kind = ScheduleKind::Daily;
                draft.hour = *hour;
                draft.minute = *minute;
            }
            Schedule::Weekly { days, hour, minute } => {
                draft.kind = ScheduleKind::Weekly;
                draft.days = days.clone();
                draft.hour = *hour;
                draft.minute = *minute;
            }
            Schedule::Monthly { day, hour, minute } => {
                draft.kind = ScheduleKind::Monthly;
                draft.month_day = *day;
                draft.hour = *hour;
                draft.minute = *minute;
            }
            Schedule::Custom { expression } => {
                draft.kind = ScheduleKind::Custom;
                draft.expression = expression.clone();
            }
        }
        for condition in &spec.conditions {
            match condition {
                Condition::ProcessNotRunning(name) => draft.not_running = name.clone(),
                Condition::OnAcPower => draft.ac_power = true,
                Condition::PathExists(path) => draft.require_path = path_text(path),
            }
        }
        draft
    }

    /// The schedule the form describes.
    pub fn schedule(&self) -> Schedule {
        match self.kind {
            ScheduleKind::Every => Schedule::Every {
                minutes: self.every_minutes,
            },
            ScheduleKind::Hourly => Schedule::Hourly {
                minute: self.minute,
            },
            ScheduleKind::Daily => Schedule::Daily {
                hour: self.hour,
                minute: self.minute,
            },
            ScheduleKind::Weekly => Schedule::Weekly {
                days: self.days.clone(),
                hour: self.hour,
                minute: self.minute,
            },
            ScheduleKind::Monthly => Schedule::Monthly {
                day: self.month_day,
                hour: self.hour,
                minute: self.minute,
            },
            ScheduleKind::Custom => Schedule::Custom {
                expression: self.expression.clone(),
            },
        }
    }

    /// What to run, as far as it is fit to be started: the command line and the
    /// folder. It lets the owner try a command before the rest of the form is
    /// done.
    pub fn command(&self) -> Result<Command, DraftError> {
        let error = |field, reason: &str| DraftError {
            field,
            reason: reason.to_owned(),
        };
        let words =
            shellwords::split(&self.command).map_err(|reason| error(Field::Command, reason))?;
        let Some((program, args)) = words.split_first() else {
            return Err(error(Field::Command, "enter the command to run"));
        };
        let command = Command {
            program: PathBuf::from(program),
            args: args.to_vec(),
            working_dir: Some(self.working_dir.trim())
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        };
        if !command.program.is_absolute() {
            return Err(error(Field::Command, "it must be an absolute path"));
        }
        if command
            .working_dir
            .as_ref()
            .is_some_and(|dir| !dir.is_absolute())
        {
            return Err(error(Field::WorkingDir, "it must be an absolute path"));
        }
        Ok(command)
    }

    /// The timer, or the first field that is wrong.
    pub fn to_spec(&self) -> Result<TimerSpec, DraftError> {
        let error = |field, reason: &str| DraftError {
            field,
            reason: reason.to_owned(),
        };
        let words =
            shellwords::split(&self.command).map_err(|reason| error(Field::Command, reason))?;
        let Some((program, args)) = words.split_first() else {
            return Err(error(Field::Command, "enter the command to run"));
        };
        let mut conditions = Vec::new();
        if !self.not_running.trim().is_empty() {
            conditions.push(Condition::ProcessNotRunning(
                self.not_running.trim().to_owned(),
            ));
        }
        if self.ac_power {
            conditions.push(Condition::OnAcPower);
        }
        if !self.require_path.trim().is_empty() {
            conditions.push(Condition::PathExists(PathBuf::from(
                self.require_path.trim(),
            )));
        }
        let spec = TimerSpec {
            name: self.name.trim().to_owned(),
            description: self.title.trim().to_owned(),
            command: Command {
                program: PathBuf::from(program),
                args: args.to_vec(),
                working_dir: Some(self.working_dir.trim())
                    .filter(|dir| !dir.is_empty())
                    .map(PathBuf::from),
            },
            schedule: self.schedule(),
            catch_up: self.catch_up && self.kind != ScheduleKind::Every,
            random_delay_secs: self.spread_minutes.saturating_mul(60),
            conditions,
            low_priority: self.low_priority,
            timeout_secs: self
                .timeout_minutes
                .map(|minutes| minutes.saturating_mul(60)),
        };
        spec.validate().map_err(|err| match err {
            SpecError::Schedule(err) => error(Field::Schedule, &err.to_string()),
            SpecError::Invalid { field, reason } => {
                let field = match field {
                    "name" => Field::Name,
                    "description" => Field::Title,
                    "program" | "arguments" => Field::Command,
                    "working directory" => Field::WorkingDir,
                    "random delay" => Field::Spread,
                    "timeout" => Field::Timeout,
                    "process name" => Field::NotRunning,
                    _ => Field::RequirePath,
                };
                error(field, reason)
            }
        })?;
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Suggestion;

    fn draft() -> Draft {
        Draft {
            name: "clean-cache".to_owned(),
            title: "Clean the cache".to_owned(),
            command: "/home/user/bin/clean.sh".to_owned(),
            ..Draft::default()
        }
    }

    #[test]
    fn a_filled_form_makes_a_valid_timer() {
        let spec = draft().to_spec().expect("valid");
        assert_eq!(spec.name, "clean-cache");
        assert_eq!(spec.schedule, Schedule::Daily { hour: 9, minute: 0 });
        assert!(spec.catch_up);
        assert_eq!(
            spec.command.program,
            PathBuf::from("/home/user/bin/clean.sh")
        );
    }

    #[test]
    fn the_command_alone_can_be_taken_before_the_form_is_done() {
        let mut d = Draft {
            command: "/bin/echo 'hi there'".to_owned(),
            working_dir: "/tmp".to_owned(),
            ..Draft::default()
        };
        let command = d.command().expect("fit to run");
        assert_eq!(command.args, vec!["hi there"]);
        assert_eq!(command.working_dir, Some(PathBuf::from("/tmp")));
        d.command = "echo hi".to_owned();
        assert_eq!(d.command().expect_err("refused").field, Field::Command);
        d.command = "/bin/echo".to_owned();
        d.working_dir = "x".to_owned();
        assert_eq!(d.command().expect_err("refused").field, Field::WorkingDir);
    }

    #[test]
    fn the_command_is_split_like_a_shell_line() {
        let mut d = draft();
        d.command = "/usr/bin/rsync -a '/home/user/My Docs/' /mnt/b".to_owned();
        let spec = d.to_spec().expect("valid");
        assert_eq!(
            spec.command.args,
            vec!["-a", "/home/user/My Docs/", "/mnt/b"]
        );
    }

    #[test]
    fn each_wrong_field_is_named() {
        type Change = dyn Fn(&mut Draft);
        let cases: [(&Change, Field); 9] = [
            (&|d| d.command = String::new(), Field::Command),
            (&|d| d.command = "ls 'x".to_owned(), Field::Command),
            (&|d| d.command = "clean.sh".to_owned(), Field::Command),
            (&|d| d.name = "Bad Name".to_owned(), Field::Name),
            (&|d| d.title = String::new(), Field::Title),
            (
                &|d| d.working_dir = "relative".to_owned(),
                Field::WorkingDir,
            ),
            (&|d| d.not_running = "a;b".to_owned(), Field::NotRunning),
            (&|d| d.require_path = "data".to_owned(), Field::RequirePath),
            (
                &|d| {
                    d.kind = ScheduleKind::Daily;
                    d.hour = 30;
                },
                Field::Schedule,
            ),
        ];
        for (change, field) in cases {
            let mut d = draft();
            change(&mut d);
            assert_eq!(d.to_spec().expect_err("refused").field, field);
        }
        let mut d = draft();
        d.timeout_minutes = Some(0);
        assert_eq!(d.to_spec().expect_err("refused").field, Field::Timeout);
        let mut d = draft();
        d.spread_minutes = 100_000;
        assert_eq!(d.to_spec().expect_err("refused").field, Field::Spread);
    }

    #[test]
    fn catching_up_is_dropped_for_every_x_minutes() {
        let mut d = draft();
        d.kind = ScheduleKind::Every;
        d.catch_up = true;
        assert!(!d.to_spec().expect("valid").catch_up);
    }

    #[test]
    fn conditions_and_options_reach_the_timer() {
        let mut d = draft();
        d.not_running = " stremio ".to_owned();
        d.ac_power = true;
        d.require_path = "/mnt/data".to_owned();
        d.low_priority = true;
        d.timeout_minutes = Some(10);
        d.spread_minutes = 5;
        let spec = d.to_spec().expect("valid");
        assert_eq!(
            spec.conditions,
            vec![
                Condition::ProcessNotRunning("stremio".to_owned()),
                Condition::OnAcPower,
                Condition::PathExists(PathBuf::from("/mnt/data")),
            ]
        );
        assert_eq!(
            (spec.timeout_secs, spec.random_delay_secs),
            (Some(600), 300)
        );
        assert!(spec.low_priority);
    }

    #[test]
    fn every_kind_of_schedule_is_built() {
        let mut d = draft();
        for (kind, expected) in [
            (ScheduleKind::Every, Schedule::Every { minutes: 30 }),
            (ScheduleKind::Hourly, Schedule::Hourly { minute: 0 }),
            (ScheduleKind::Daily, Schedule::Daily { hour: 9, minute: 0 }),
            (
                ScheduleKind::Weekly,
                Schedule::Weekly {
                    days: vec![
                        Weekday::Mon,
                        Weekday::Tue,
                        Weekday::Wed,
                        Weekday::Thu,
                        Weekday::Fri,
                    ],
                    hour: 9,
                    minute: 0,
                },
            ),
            (
                ScheduleKind::Monthly,
                Schedule::Monthly {
                    day: 1,
                    hour: 9,
                    minute: 0,
                },
            ),
        ] {
            d.kind = kind;
            assert_eq!(d.schedule(), expected);
        }
        d.kind = ScheduleKind::Custom;
        d.expression = "Mon *-*-* 02:00:00".to_owned();
        assert_eq!(
            d.to_spec().expect("valid").schedule,
            Schedule::Custom {
                expression: d.expression.clone()
            }
        );
        d.expression = String::new();
        assert_eq!(d.to_spec().expect_err("refused").field, Field::Schedule);
    }

    #[test]
    fn a_timer_goes_into_the_form_and_comes_back() {
        let mut d = draft();
        d.command = "/usr/bin/rsync -a '/home/user/My Docs/' /mnt/b".to_owned();
        d.working_dir = "/home/user".to_owned();
        d.kind = ScheduleKind::Weekly;
        d.days = vec![Weekday::Sat, Weekday::Sun];
        d.hour = 2;
        d.minute = 15;
        d.not_running = "rsync".to_owned();
        d.ac_power = true;
        d.require_path = "/mnt/b".to_owned();
        d.low_priority = true;
        d.timeout_minutes = Some(120);
        d.spread_minutes = 10;
        let spec = d.to_spec().expect("valid");
        let back = Draft::from_spec(&spec);
        assert_eq!(back.to_spec().expect("valid"), spec);
        assert_eq!(back.kind, ScheduleKind::Weekly);
        assert_eq!(back.timeout_minutes, Some(120));
    }

    #[test]
    fn a_suggestion_fills_the_form() {
        let suggestion = Suggestion {
            name: "job".to_owned(),
            description: "Job".to_owned(),
            command: Command {
                program: PathBuf::from("/usr/bin/bash"),
                args: vec!["/home/user/My Scripts/job.sh".to_owned()],
                working_dir: Some(PathBuf::from("/home/user/My Scripts")),
            },
        };
        let d = Draft::from_suggestion(&suggestion);
        assert_eq!(d.command, "/usr/bin/bash '/home/user/My Scripts/job.sh'");
        assert_eq!(d.working_dir, "/home/user/My Scripts");
        let spec = d.to_spec().expect("valid");
        assert_eq!(spec.command.args, vec!["/home/user/My Scripts/job.sh"]);
    }
}
