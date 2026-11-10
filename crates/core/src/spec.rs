// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! A whole timer, as the form shows it, and the rules that keep it writable.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::schedule::{Schedule, ScheduleError};

pub const MAX_NAME: usize = 64;
pub const MAX_DESCRIPTION: usize = 120;
/// The kernel cuts a process name at 15 bytes, so `pgrep -x` cannot see more.
pub const MAX_PROCESS_NAME: usize = 15;
pub const MAX_RANDOM_DELAY_SECS: u32 = 86_400;

/// What the timer runs. Systemd wants an absolute program and splits arguments
/// itself, so there is no shell unless the program is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
}

/// When a run is skipped (quietly, not as a failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// No process with exactly this name is running.
    ProcessNotRunning(String),
    /// The computer is on mains power.
    OnAcPower,
    /// This path exists.
    PathExists(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSpec {
    /// Unit name without suffix: `stremio-clean-cache`.
    pub name: String,
    pub description: String,
    pub command: Command,
    pub schedule: Schedule,
    /// Run once after a missed time when the computer was off or the session
    /// closed. Wall-clock schedules only.
    pub catch_up: bool,
    /// Spread the start over up to this many seconds.
    pub random_delay_secs: u32,
    pub conditions: Vec<Condition>,
    /// Low CPU and disk priority, for work that must not slow the computer.
    pub low_priority: bool,
    /// Stop the run after this many seconds.
    pub timeout_secs: Option<u32>,
    /// Show a desktop notification when a run fails (the notifier unit,
    /// [`crate::unit::render_notifier`]).
    pub notify_failure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
    Schedule(ScheduleError),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Invalid { field, reason } => write!(f, "{field}: {reason}"),
            SpecError::Schedule(err) => write!(f, "schedule: {err}"),
        }
    }
}

impl std::error::Error for SpecError {}

impl From<ScheduleError> for SpecError {
    fn from(err: ScheduleError) -> Self {
        SpecError::Schedule(err)
    }
}

fn invalid(field: &'static str, reason: &'static str) -> SpecError {
    SpecError::Invalid { field, reason }
}

fn plain_text(text: &str) -> bool {
    !text.chars().any(char::is_control)
}

fn absolute_path(field: &'static str, path: &Path) -> Result<(), SpecError> {
    let Some(text) = path.to_str() else {
        return Err(invalid(field, "it is not valid text"));
    };
    if !plain_text(text) {
        return Err(invalid(field, "it has a control character"));
    }
    if !path.is_absolute() {
        return Err(invalid(field, "it must be an absolute path"));
    }
    Ok(())
}

impl TimerSpec {
    pub fn timer_unit(&self) -> String {
        format!("{}.timer", self.name)
    }

    pub fn service_unit(&self) -> String {
        format!("{}.service", self.name)
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        self.validate_name()?;
        let description = self.description.as_str();
        if description.is_empty() || description != description.trim() {
            return Err(invalid(
                "description",
                "it must not be empty or start or end with a space",
            ));
        }
        if description.chars().count() > MAX_DESCRIPTION {
            return Err(invalid("description", "it is too long"));
        }
        if !plain_text(description) {
            return Err(invalid("description", "it must be one line of plain text"));
        }

        absolute_path("program", &self.command.program)?;
        if self.command.args.iter().any(|arg| !plain_text(arg)) {
            return Err(invalid("arguments", "an argument has a control character"));
        }
        if let Some(dir) = &self.command.working_dir {
            absolute_path("working directory", dir)?;
        }

        self.schedule.validate()?;
        if self.random_delay_secs > MAX_RANDOM_DELAY_SECS {
            return Err(invalid("random delay", "it is longer than a day"));
        }
        if self.timeout_secs == Some(0) {
            return Err(invalid("timeout", "it must be at least one second"));
        }
        for condition in &self.conditions {
            match condition {
                Condition::ProcessNotRunning(name) => validate_process_name(name)?,
                Condition::OnAcPower => {}
                Condition::PathExists(path) => absolute_path("condition path", path)?,
            }
        }
        Ok(())
    }

    fn validate_name(&self) -> Result<(), SpecError> {
        let name = self.name.as_str();
        if name.is_empty() || name.len() > MAX_NAME {
            return Err(invalid("name", "it must have 1 to 64 characters"));
        }
        let mut chars = name.chars();
        let first_ok = chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        let rest_ok = name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'));
        if !first_ok || !rest_ok {
            return Err(invalid(
                "name",
                "use lowercase letters, digits, - _ and . and start with a letter or digit",
            ));
        }
        if name.ends_with(".timer") || name.ends_with(".service") {
            return Err(invalid("name", "leave out .timer and .service"));
        }
        Ok(())
    }
}

/// A process name as `pgrep -x` matches it, safe to put in a shell command.
pub fn validate_process_name(name: &str) -> Result<(), SpecError> {
    if name.is_empty() {
        return Err(invalid("process name", "it is empty"));
    }
    if name.len() > MAX_PROCESS_NAME {
        return Err(invalid(
            "process name",
            "the system cuts process names at 15 characters",
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
    {
        return Err(invalid(
            "process name",
            "use letters, digits and . _ + - only",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The example this app started from.
    pub fn stremio() -> TimerSpec {
        TimerSpec {
            name: "stremio-clean-cache".to_owned(),
            description: "Clean Stremio's cache".to_owned(),
            command: Command {
                program: PathBuf::from("/home/user/.local/bin/clean-stremio-cache.sh"),
                args: Vec::new(),
                working_dir: None,
            },
            schedule: Schedule::Every { minutes: 30 },
            catch_up: false,
            random_delay_secs: 0,
            conditions: vec![Condition::ProcessNotRunning("stremio".to_owned())],
            low_priority: true,
            timeout_secs: Some(600),
            notify_failure: false,
        }
    }

    fn reason(spec: &TimerSpec) -> String {
        spec.validate().expect_err("should be refused").to_string()
    }

    #[test]
    fn the_example_is_valid() {
        assert_eq!(stremio().validate(), Ok(()));
        assert_eq!(stremio().timer_unit(), "stremio-clean-cache.timer");
        assert_eq!(stremio().service_unit(), "stremio-clean-cache.service");
    }

    #[test]
    fn names_are_unit_safe() {
        for name in [
            "",
            "Upper",
            "-dash",
            "a b",
            "a/b",
            "a.timer",
            "a.service",
            &"x".repeat(65),
        ] {
            let spec = TimerSpec {
                name: name.to_owned(),
                ..stremio()
            };
            assert!(spec.validate().is_err(), "{name:?}");
        }
        for name in ["a", "1st", "backup_home.daily", "x-y"] {
            let spec = TimerSpec {
                name: name.to_owned(),
                ..stremio()
            };
            assert_eq!(spec.validate(), Ok(()), "{name:?}");
        }
    }

    #[test]
    fn description_is_one_trimmed_line() {
        for text in ["", " lead", "trail ", "two\nlines"] {
            let spec = TimerSpec {
                description: text.to_owned(),
                ..stremio()
            };
            assert!(spec.validate().is_err(), "{text:?}");
        }
        let spec = TimerSpec {
            description: "d".repeat(MAX_DESCRIPTION + 1),
            ..stremio()
        };
        assert!(reason(&spec).contains("too long"));
    }

    #[test]
    fn the_program_must_be_an_absolute_path() {
        let mut spec = stremio();
        spec.command.program = PathBuf::from("clean.sh");
        assert!(reason(&spec).contains("absolute"));
        spec.command.program = PathBuf::from("/bin/x\ny");
        assert!(reason(&spec).contains("control"));
    }

    #[test]
    fn arguments_and_directory_are_checked() {
        let mut spec = stremio();
        spec.command.args = vec!["fine".to_owned(), "bad\nline".to_owned()];
        assert!(reason(&spec).contains("argument"));
        spec.command.args.clear();
        spec.command.working_dir = Some(PathBuf::from("relative"));
        assert!(reason(&spec).contains("working directory"));
    }

    #[test]
    fn numbers_have_limits() {
        let mut spec = stremio();
        spec.timeout_secs = Some(0);
        assert!(reason(&spec).contains("timeout"));
        spec.timeout_secs = None;
        spec.random_delay_secs = MAX_RANDOM_DELAY_SECS + 1;
        assert!(reason(&spec).contains("random delay"));
    }

    #[test]
    fn a_bad_schedule_is_reported_as_such() {
        let mut spec = stremio();
        spec.schedule = Schedule::Hourly { minute: 99 };
        assert_eq!(
            spec.validate(),
            Err(SpecError::Schedule(ScheduleError::Minute(99)))
        );
    }

    #[test]
    fn process_names_are_safe_for_a_shell() {
        for name in ["", "a;b", "a b", "$(x)", "a'b", "sixteen-chars-xxx"] {
            assert!(validate_process_name(name).is_err(), "{name:?}");
        }
        for name in ["stremio", "Xorg", "gnome-shell", "c++", "a.b_c"] {
            assert_eq!(validate_process_name(name), Ok(()), "{name:?}");
        }
    }

    #[test]
    fn condition_paths_must_be_absolute() {
        let mut spec = stremio();
        spec.conditions = vec![Condition::PathExists(PathBuf::from("data"))];
        assert!(reason(&spec).contains("condition path"));
    }
}
