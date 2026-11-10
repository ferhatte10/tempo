// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The `.timer` and `.service` files of a [`TimerSpec`], and the way back.
//!
//! Every file Tempo writes carries `X-Tempo-…` keys (systemd ignores keys that
//! start with `X-`) that hold the choices exactly as the form had them. Reading
//! a timer back never parses systemd's own directives: it reads those keys, so
//! only a file Tempo wrote can be edited by form, and [`edited_outside`] tells
//! when someone changed one by hand.

use std::fmt::{self, Write};
use std::path::{Path, PathBuf};

use crate::schedule::{Schedule, Timing};
use crate::spec::{Command, Condition, SpecError, TimerSpec};

const MANAGED: &str = "X-Tempo-Managed=1";
/// The template unit a failed run starts (`OnFailure=`): its instance is the
/// failed service, `tempo-notify@backup.service.service`.
pub const NOTIFIER: &str = "tempo-notify@.service";
/// Nice level of a low-priority run: the lowest priority.
const LOW_NICE: u8 = 19;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The files were not written by Tempo.
    NotManaged,
    Missing(&'static str),
    Unreadable(&'static str),
    Spec(SpecError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotManaged => write!(f, "these files were not written by Tempo"),
            ParseError::Missing(key) => write!(f, "{key} is missing"),
            ParseError::Unreadable(key) => write!(f, "{key} cannot be read"),
            ParseError::Spec(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<SpecError> for ParseError {
    fn from(err: SpecError) -> Self {
        ParseError::Spec(err)
    }
}

/// One word of an `ExecStart=` line, as systemd splits it: bare when it is
/// plain, otherwise in double quotes with `\`, `"`, `%` and `$` protected
/// (`%` starts a specifier and `$` a variable).
fn exec_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '/' | '.' | '_' | '+' | '=' | ':' | ',' | '@' | '-')
        });
    if plain {
        return word.to_owned();
    }
    if word == ";" {
        // A bare semicolon would end the command.
        return "\\;".to_owned();
    }
    let mut quoted = String::with_capacity(word.len() + 2);
    quoted.push('"');
    for c in word.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '%' => quoted.push_str("%%"),
            '$' => quoted.push_str("$$"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// A value on a line where `%` starts a specifier.
fn escape_percent(text: &str) -> String {
    text.replace('%', "%%")
}

fn text_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// The `.timer` file.
pub fn render_timer(spec: &TimerSpec) -> Result<String, SpecError> {
    spec.validate()?;
    let timing = spec.schedule.timing()?;
    let mut out = String::new();
    let line = |out: &mut String, key: &str, value: &str| {
        let _ = writeln!(out, "{key}={value}");
    };

    out.push_str("[Unit]\n");
    line(&mut out, "Description", &escape_percent(&spec.description));
    out.push_str(MANAGED);
    out.push('\n');
    line(&mut out, "X-Tempo-Schedule", &spec.schedule.to_key());
    line(&mut out, "X-Tempo-CatchUp", yes_no(spec.catch_up));
    line(
        &mut out,
        "X-Tempo-RandomDelay",
        &spec.random_delay_secs.to_string(),
    );

    out.push_str("\n[Timer]\n");
    match timing {
        Timing::Calendar(expression) => {
            line(&mut out, "OnCalendar", &expression);
            if spec.catch_up {
                line(&mut out, "Persistent", "true");
            }
        }
        Timing::Monotonic {
            first_secs,
            repeat_secs,
        } => {
            line(&mut out, "OnBootSec", &format!("{first_secs}s"));
            line(&mut out, "OnUnitActiveSec", &format!("{repeat_secs}s"));
        }
    }
    if spec.random_delay_secs > 0 {
        line(
            &mut out,
            "RandomizedDelaySec",
            &format!("{}s", spec.random_delay_secs),
        );
    }

    out.push_str("\n[Install]\nWantedBy=timers.target\n");
    Ok(out)
}

/// The `.service` file the timer starts.
pub fn render_service(spec: &TimerSpec) -> Result<String, SpecError> {
    spec.validate()?;
    let mut out = String::new();
    let line = |out: &mut String, key: &str, value: &str| {
        let _ = writeln!(out, "{key}={value}");
    };

    out.push_str("[Unit]\n");
    line(&mut out, "Description", &escape_percent(&spec.description));
    out.push_str(MANAGED);
    out.push('\n');
    line(&mut out, "X-Tempo-Program", &text_of(&spec.command.program));
    for arg in &spec.command.args {
        line(&mut out, "X-Tempo-Arg", arg);
    }
    if let Some(dir) = &spec.command.working_dir {
        line(&mut out, "X-Tempo-WorkDir", &text_of(dir));
    }
    for condition in &spec.conditions {
        line(&mut out, "X-Tempo-Condition", &condition_key(condition));
    }
    if spec.low_priority {
        line(&mut out, "X-Tempo-LowPriority", "yes");
    }
    if let Some(secs) = spec.timeout_secs {
        line(&mut out, "X-Tempo-Timeout", &secs.to_string());
    }
    if spec.notify_failure {
        line(&mut out, "X-Tempo-NotifyFailure", "yes");
        line(&mut out, "OnFailure", "tempo-notify@%n.service");
    }
    for condition in &spec.conditions {
        match condition {
            Condition::OnAcPower => line(&mut out, "ConditionACPower", "true"),
            Condition::PathExists(path) => line(
                &mut out,
                "ConditionPathExists",
                &escape_percent(&text_of(path)),
            ),
            Condition::ProcessNotRunning(_) => {}
        }
    }

    out.push_str("\n[Service]\nType=oneshot\n");
    if let Some(dir) = &spec.command.working_dir {
        line(&mut out, "WorkingDirectory", &escape_percent(&text_of(dir)));
    }
    if spec.low_priority {
        line(&mut out, "Nice", &LOW_NICE.to_string());
        line(&mut out, "IOSchedulingClass", "idle");
    }
    if let Some(secs) = spec.timeout_secs {
        line(&mut out, "TimeoutStartSec", &format!("{secs}s"));
    }
    for condition in &spec.conditions {
        if let Condition::ProcessNotRunning(name) = condition {
            // Exit 1 skips the run quietly; only 255 would count as a failure.
            let test = format!("! /usr/bin/pgrep -x '{name}'");
            line(
                &mut out,
                "ExecCondition",
                &format!("/bin/sh -c {}", exec_word(&test)),
            );
        }
    }
    let mut exec = exec_word(&text_of(&spec.command.program));
    for arg in &spec.command.args {
        exec.push(' ');
        exec.push_str(&exec_word(arg));
    }
    line(&mut out, "ExecStart", &exec);
    Ok(out)
}

fn condition_key(condition: &Condition) -> String {
    match condition {
        Condition::ProcessNotRunning(name) => format!("process-not-running:{name}"),
        Condition::OnAcPower => "ac-power".to_owned(),
        Condition::PathExists(path) => format!("path-exists:{}", text_of(path)),
    }
}

fn condition_from_key(key: &str) -> Option<Condition> {
    if key == "ac-power" {
        return Some(Condition::OnAcPower);
    }
    let (kind, value) = key.split_once(':')?;
    match kind {
        "process-not-running" => Some(Condition::ProcessNotRunning(value.to_owned())),
        "path-exists" => Some(Condition::PathExists(PathBuf::from(value))),
        _ => None,
    }
}

/// Every value of `key=` in a unit file, untrimmed.
fn values<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let prefix = format!("{key}=");
    text.lines()
        .filter_map(|line| line.strip_prefix(prefix.as_str()))
        .collect()
}

fn first<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    values(text, key).into_iter().next()
}

fn is_managed(text: &str) -> bool {
    text.lines().any(|line| line == MANAGED)
}

/// Reads back what Tempo wrote. `name` is the unit's name without suffix.
pub fn parse(name: &str, timer: &str, service: &str) -> Result<TimerSpec, ParseError> {
    if !is_managed(timer) || !is_managed(service) {
        return Err(ParseError::NotManaged);
    }
    let missing = ParseError::Missing;
    let unreadable = ParseError::Unreadable;

    let description = first(service, "Description")
        .ok_or(missing("Description"))?
        .replace("%%", "%");
    let schedule =
        Schedule::from_key(first(timer, "X-Tempo-Schedule").ok_or(missing("X-Tempo-Schedule"))?)
            .map_err(SpecError::from)?;
    let catch_up = match first(timer, "X-Tempo-CatchUp") {
        Some("yes") => true,
        Some("no") => false,
        Some(_) => return Err(unreadable("X-Tempo-CatchUp")),
        None => return Err(missing("X-Tempo-CatchUp")),
    };
    let random_delay_secs = first(timer, "X-Tempo-RandomDelay")
        .ok_or(missing("X-Tempo-RandomDelay"))?
        .parse()
        .map_err(|_| unreadable("X-Tempo-RandomDelay"))?;

    let program = first(service, "X-Tempo-Program").ok_or(missing("X-Tempo-Program"))?;
    let args = values(service, "X-Tempo-Arg")
        .into_iter()
        .map(str::to_owned)
        .collect();
    let working_dir = first(service, "X-Tempo-WorkDir").map(PathBuf::from);
    let mut conditions = Vec::new();
    for key in values(service, "X-Tempo-Condition") {
        conditions.push(condition_from_key(key).ok_or(unreadable("X-Tempo-Condition"))?);
    }
    let low_priority = first(service, "X-Tempo-LowPriority") == Some("yes");
    let notify_failure = first(service, "X-Tempo-NotifyFailure") == Some("yes");
    let timeout_secs = match first(service, "X-Tempo-Timeout") {
        Some(text) => Some(text.parse().map_err(|_| unreadable("X-Tempo-Timeout"))?),
        None => None,
    };

    let spec = TimerSpec {
        name: name.to_owned(),
        description,
        command: Command {
            program: PathBuf::from(program),
            args,
            working_dir,
        },
        schedule,
        catch_up,
        random_delay_secs,
        conditions,
        low_priority,
        timeout_secs,
        notify_failure,
    };
    spec.validate()?;
    Ok(spec)
}

/// The notifier template ([`NOTIFIER`]): runs `program --notify-failed` with
/// the name of the service that failed. Carries no `X-Tempo-Managed` key: it
/// is not a timer and is never listed or edited.
pub fn render_notifier(program: &Path) -> String {
    format!(
        "[Unit]\nDescription=Tempo: tell that %i failed\n\n[Service]\nType=oneshot\nExecStart={} --notify-failed %i\n",
        exec_word(&text_of(program))
    )
}

/// The timer's name from the notifier's instance: `backup.service` → `backup`.
pub fn failed_timer(instance: &str) -> Option<&str> {
    instance
        .strip_suffix(".service")
        .filter(|name| !name.is_empty() && !name.contains('/'))
}

/// True when the files on disk are not exactly what Tempo would write for this
/// spec: someone edited them by hand, and a save from the form would erase it.
pub fn edited_outside(spec: &TimerSpec, timer: &str, service: &str) -> bool {
    match (render_timer(spec), render_service(spec)) {
        (Ok(expected_timer), Ok(expected_service)) => {
            expected_timer != timer || expected_service != service
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::Weekday;
    use crate::spec::tests::stremio;

    const STREMIO_TIMER: &str = "\
[Unit]
Description=Clean Stremio's cache
X-Tempo-Managed=1
X-Tempo-Schedule=every 30
X-Tempo-CatchUp=no
X-Tempo-RandomDelay=0

[Timer]
OnBootSec=300s
OnUnitActiveSec=1800s

[Install]
WantedBy=timers.target
";

    const STREMIO_SERVICE: &str = "\
[Unit]
Description=Clean Stremio's cache
X-Tempo-Managed=1
X-Tempo-Program=/home/user/.local/bin/clean-stremio-cache.sh
X-Tempo-Condition=process-not-running:stremio
X-Tempo-LowPriority=yes
X-Tempo-Timeout=600

[Service]
Type=oneshot
Nice=19
IOSchedulingClass=idle
TimeoutStartSec=600s
ExecCondition=/bin/sh -c \"! /usr/bin/pgrep -x 'stremio'\"
ExecStart=/home/user/.local/bin/clean-stremio-cache.sh
";

    #[test]
    fn the_example_timer_file() {
        assert_eq!(render_timer(&stremio()).as_deref(), Ok(STREMIO_TIMER));
    }

    #[test]
    fn the_example_service_file() {
        assert_eq!(render_service(&stremio()).as_deref(), Ok(STREMIO_SERVICE));
    }

    #[test]
    fn a_calendar_timer_can_catch_up_and_be_spread() {
        let mut spec = stremio();
        spec.schedule = Schedule::Daily {
            hour: 9,
            minute: 30,
        };
        spec.catch_up = true;
        spec.random_delay_secs = 120;
        let text = render_timer(&spec).expect("renders");
        assert!(text.contains("OnCalendar=*-*-* 09:30:00\nPersistent=true\n"));
        assert!(text.contains("RandomizedDelaySec=120s\n"));
        assert!(!text.contains("OnBootSec"));
    }

    #[test]
    fn catching_up_means_nothing_for_every_x_minutes() {
        let mut spec = stremio();
        spec.catch_up = true;
        let text = render_timer(&spec).expect("renders");
        assert!(!text.contains("Persistent"));
    }

    #[test]
    fn exec_words_are_protected() {
        assert_eq!(exec_word("/usr/bin/x"), "/usr/bin/x");
        assert_eq!(exec_word("--flag=value"), "--flag=value");
        assert_eq!(exec_word("two words"), "\"two words\"");
        assert_eq!(exec_word(""), "\"\"");
        assert_eq!(exec_word("50%"), "\"50%%\"");
        assert_eq!(exec_word("$HOME"), "\"$$HOME\"");
        assert_eq!(exec_word("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(exec_word("back\\slash"), "\"back\\\\slash\"");
        assert_eq!(exec_word(";"), "\\;");
    }

    #[test]
    fn arguments_directory_and_conditions_reach_the_service() {
        let mut spec = stremio();
        spec.command.args = vec!["--keep".to_owned(), "my files".to_owned()];
        spec.command.working_dir = Some(PathBuf::from("/home/user/my 100%"));
        spec.conditions = vec![
            Condition::OnAcPower,
            Condition::PathExists(PathBuf::from("/mnt/data")),
        ];
        let text = render_service(&spec).expect("renders");
        assert!(text.contains("ConditionACPower=true\n"));
        assert!(text.contains("ConditionPathExists=/mnt/data\n"));
        assert!(text.contains("WorkingDirectory=/home/user/my 100%%\n"));
        assert!(text.contains(
            "ExecStart=/home/user/.local/bin/clean-stremio-cache.sh --keep \"my files\"\n"
        ));
        assert!(!text.contains("ExecCondition"));
    }

    #[test]
    fn a_percent_in_the_description_is_protected() {
        let mut spec = stremio();
        spec.description = "Free 50% of the disk".to_owned();
        let timer = render_timer(&spec).expect("renders");
        assert!(timer.contains("Description=Free 50%% of the disk\n"));
    }

    #[test]
    fn an_invalid_spec_writes_nothing() {
        let mut spec = stremio();
        spec.name = "Bad Name".to_owned();
        assert!(render_timer(&spec).is_err());
        assert!(render_service(&spec).is_err());
    }

    fn rich() -> TimerSpec {
        TimerSpec {
            name: "weekly-backup".to_owned(),
            description: "Back up 100% of ~/Documents".to_owned(),
            command: Command {
                program: PathBuf::from("/usr/bin/rsync"),
                args: vec![
                    "-a".to_owned(),
                    "/home/user/My Documents/".to_owned(),
                    "/mnt/backup/$(date)".to_owned(),
                    String::new(),
                ],
                working_dir: Some(PathBuf::from("/home/user")),
            },
            schedule: Schedule::Weekly {
                days: vec![Weekday::Sat, Weekday::Sun],
                hour: 2,
                minute: 15,
            },
            catch_up: true,
            random_delay_secs: 300,
            conditions: vec![
                Condition::OnAcPower,
                Condition::PathExists(PathBuf::from("/mnt/backup")),
                Condition::ProcessNotRunning("rsync".to_owned()),
            ],
            low_priority: true,
            timeout_secs: Some(7200),
            notify_failure: true,
        }
    }

    #[test]
    fn a_failure_starts_the_notifier() {
        let service = render_service(&rich()).expect("renders");
        assert!(service.contains("\nOnFailure=tempo-notify@%n.service\n"));
        assert!(service.contains("\nX-Tempo-NotifyFailure=yes\n"));
        let quiet = render_service(&crate::spec::tests::stremio()).expect("renders");
        assert!(!quiet.contains("OnFailure"));
    }

    #[test]
    fn the_notifier_runs_tempo_with_the_failed_unit() {
        let unit = render_notifier(Path::new("/opt/my apps/tempo"));
        assert!(unit.contains("\nExecStart=\"/opt/my apps/tempo\" --notify-failed %i\n"));
        assert!(!unit.contains("X-Tempo-Managed"));
        assert_eq!(failed_timer("backup.service"), Some("backup"));
        for wrong in ["backup.timer", ".service", "a/b.service"] {
            assert_eq!(failed_timer(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn what_is_written_reads_back_exactly() {
        for spec in [stremio(), rich()] {
            let timer = render_timer(&spec).expect("renders");
            let service = render_service(&spec).expect("renders");
            assert_eq!(parse(&spec.name, &timer, &service), Ok(spec));
        }
    }

    #[test]
    fn files_not_written_by_tempo_are_not_read() {
        let foreign = "[Unit]\nDescription=Vendor\n\n[Timer]\nOnCalendar=daily\n";
        assert_eq!(
            parse("vendor", foreign, "[Service]\nExecStart=/bin/true\n"),
            Err(ParseError::NotManaged)
        );
        assert_eq!(
            parse("x", STREMIO_TIMER, "[Service]\nExecStart=/bin/true\n"),
            Err(ParseError::NotManaged)
        );
    }

    #[test]
    fn damaged_files_say_what_is_missing() {
        let timer = STREMIO_TIMER.replace("X-Tempo-Schedule=every 30\n", "");
        assert_eq!(
            parse("x", &timer, STREMIO_SERVICE),
            Err(ParseError::Missing("X-Tempo-Schedule"))
        );
        let service = STREMIO_SERVICE.replace("X-Tempo-Timeout=600", "X-Tempo-Timeout=soon");
        assert_eq!(
            parse("x", STREMIO_TIMER, &service),
            Err(ParseError::Unreadable("X-Tempo-Timeout"))
        );
        let service = STREMIO_SERVICE.replace("process-not-running:stremio", "cloudy");
        assert_eq!(
            parse("x", STREMIO_TIMER, &service),
            Err(ParseError::Unreadable("X-Tempo-Condition"))
        );
    }

    #[test]
    fn a_hand_edit_is_noticed() {
        let spec = stremio();
        assert!(!edited_outside(&spec, STREMIO_TIMER, STREMIO_SERVICE));
        let edited = STREMIO_TIMER.replace("1800s", "60s");
        assert!(edited_outside(&spec, &edited, STREMIO_SERVICE));
        let edited = format!("{STREMIO_SERVICE}Environment=A=b\n");
        assert!(edited_outside(&spec, STREMIO_TIMER, &edited));
    }
}
