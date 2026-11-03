// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! From what systemd says about a timer to what the interface shows. Pure: the
//! D-Bus side (`systemd.rs`) fills a [`RawTimer`], this turns it into a
//! [`TimerView`].

use tempo_core::schedule::Schedule;
use tempo_core::unit;

use crate::unitfile;
use crate::view::{Origin, Run, RunResult, State, TimerView};

/// What the service a timer starts says, when systemd has it loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawService {
    pub fragment_path: Option<String>,
    pub active_state: String,
    pub result: String,
    pub exec_main_status: i32,
    /// Microseconds since the epoch; 0 when it never ran.
    pub start_usec: u64,
    pub exit_usec: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawTimer {
    pub unit: String,
    pub fragment_path: Option<String>,
    pub description: String,
    pub active_state: String,
    /// `enabled`, `disabled`, `static`…
    pub unit_file_state: String,
    /// Microseconds since the epoch; 0 when never.
    pub last_trigger_usec: u64,
    pub next_realtime_usec: u64,
    /// On the monotonic clock, in microseconds.
    pub next_monotonic_usec: u64,
    /// The unit it starts: `clean.service`.
    pub activates: String,
    pub service: Option<RawService>,
}

/// The text of the two unit files, when they could be read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts {
    pub timer: Option<String>,
    pub service: Option<String>,
}

/// The two clocks, read at the same moment, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clocks {
    pub realtime_usec: u64,
    pub monotonic_usec: u64,
}

const MICROS: i64 = 1_000_000;

fn base_name(unit: &str) -> &str {
    unit.strip_suffix(".timer").unwrap_or(unit)
}

fn secs_between(later: u64, earlier: u64) -> i64 {
    (i64::try_from(later).unwrap_or(i64::MAX) - i64::try_from(earlier).unwrap_or(i64::MAX)) / MICROS
}

fn result_of(result: &str) -> RunResult {
    match result {
        "success" => RunResult::Success,
        "skipped" => RunResult::Skipped,
        "" => RunResult::Unknown,
        _ => RunResult::Failed,
    }
}

fn last_run(raw: &RawTimer, clocks: Clocks) -> Option<Run> {
    if let Some(service) = &raw.service {
        let ended = if service.exit_usec > 0 {
            service.exit_usec
        } else {
            service.start_usec
        };
        if ended > 0 {
            let took = if service.exit_usec >= service.start_usec && service.start_usec > 0 {
                secs_between(service.exit_usec, service.start_usec)
            } else {
                0
            };
            return Some(Run {
                result: result_of(&service.result),
                ago_secs: secs_between(clocks.realtime_usec, ended),
                duration_secs: u32::try_from(took).unwrap_or(0),
                exit_code: (service.exit_usec > 0).then_some(service.exec_main_status),
            });
        }
    }
    // The service is gone from memory: only the moment is known.
    (raw.last_trigger_usec > 0).then(|| Run {
        result: RunResult::Unknown,
        ago_secs: secs_between(clocks.realtime_usec, raw.last_trigger_usec),
        duration_secs: 0,
        exit_code: None,
    })
}

fn next_in(raw: &RawTimer, clocks: Clocks) -> Option<i64> {
    if raw.active_state != "active" {
        return None;
    }
    let by_calendar = (raw.next_realtime_usec > 0)
        .then(|| secs_between(raw.next_realtime_usec, clocks.realtime_usec));
    let by_boot = (raw.next_monotonic_usec > 0)
        .then(|| secs_between(raw.next_monotonic_usec, clocks.monotonic_usec));
    [by_calendar, by_boot]
        .into_iter()
        .flatten()
        .map(|secs| secs.max(0))
        .min()
}

pub fn display_command(program: &std::path::Path, args: &[String]) -> String {
    let mut parts = vec![program.display().to_string()];
    parts.extend(args.iter().map(|arg| {
        if arg.is_empty() || arg.contains(char::is_whitespace) {
            format!("\"{arg}\"")
        } else {
            arg.clone()
        }
    }));
    parts.join(" ")
}

fn state_of(raw: &RawTimer) -> State {
    if raw.active_state != "active" {
        return State::Paused;
    }
    match raw.service.as_ref().map(|s| s.active_state.as_str()) {
        Some("active" | "activating" | "reloading") => State::Running,
        _ => State::Waiting,
    }
}

/// Longest sentence that still serves as a title.
const MAX_TITLE: usize = 48;

/// The description without the quotes some units wrap it in.
fn description_of(raw: &RawTimer) -> String {
    raw.description
        .trim()
        .trim_matches(['"', '\''])
        .trim()
        .to_owned()
}

/// What a timer is called: its description when that is short, otherwise its
/// name made readable (`ubuntu-insights-upload` becomes “Ubuntu insights upload”).
fn title_of(description: &str, name: &str) -> String {
    if !description.is_empty() && description.chars().count() <= MAX_TITLE {
        return description.to_owned();
    }
    let spaced = name.replace(['-', '_', '.'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The view of one timer. `home` is the owner's home folder: a timer defined
/// under it, that Tempo did not write, is “personal”. `history` is what the
/// journal says about its runs; without it, the last run is read from the
/// service itself.
pub fn to_view(
    raw: &RawTimer,
    texts: &Texts,
    clocks: Clocks,
    home: &str,
    history: &[Run],
) -> TimerView {
    let name = base_name(&raw.unit);
    let file = raw.fragment_path.clone().unwrap_or_default();

    let managed = match (&texts.timer, &texts.service) {
        (Some(timer), Some(service)) => unit::parse(name, timer, service).ok(),
        _ => None,
    };
    let (origin, title, schedule, command) = match managed {
        Some(spec) => (
            Origin::Yours,
            spec.description.clone(),
            spec.schedule.clone(),
            display_command(&spec.command.program, &spec.command.args),
        ),
        None => {
            let origin = if !home.is_empty() && file.starts_with(&format!("{home}/")) {
                Origin::Personal
            } else {
                Origin::System
            };
            let title = title_of(&raw.description, name);
            let schedule = texts
                .timer
                .as_deref()
                .and_then(unitfile::schedule)
                .unwrap_or(Schedule::Custom {
                    expression: String::from("?"),
                });
            let command = texts
                .service
                .as_deref()
                .and_then(unitfile::exec_start)
                .unwrap_or_else(|| raw.activates.clone());
            (origin, title, schedule, command)
        }
    };

    // A Tempo timer's description is its title already.
    let description = if origin == Origin::Yours {
        String::new()
    } else {
        description_of(raw)
    };
    TimerView {
        unit: raw.unit.clone(),
        title,
        description,
        origin,
        state: state_of(raw),
        enabled: raw.unit_file_state.starts_with("enabled"),
        schedule,
        next_in_secs: next_in(raw, clocks),
        service: raw.activates.clone(),
        runs: if history.is_empty() {
            last_run(raw, clocks).into_iter().collect()
        } else {
            history.to_vec()
        },
        command,
        file,
    }
}

/// Failed ones first, then the soonest, then by name.
pub fn sort(timers: &mut [TimerView]) {
    timers.sort_by(|a, b| {
        let failed = |t: &TimerView| t.health() == crate::view::Health::Failed;
        failed(b)
            .cmp(&failed(a))
            .then_with(|| {
                let key = |t: &TimerView| t.next_in_secs.unwrap_or(i64::MAX);
                key(a).cmp(&key(b))
            })
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempo_core::spec::{Command, Condition, TimerSpec};

    const HOME: &str = "/home/user";
    const NOW: u64 = 1_800_000_000 * 1_000_000;
    const CLOCKS: Clocks = Clocks {
        realtime_usec: NOW,
        monotonic_usec: 10_000 * 1_000_000,
    };

    fn secs(n: u64) -> u64 {
        n * 1_000_000
    }

    fn system_timer() -> RawTimer {
        RawTimer {
            unit: "launchpadlib-cache-clean.timer".to_owned(),
            fragment_path: Some("/usr/lib/systemd/user/launchpadlib-cache-clean.timer".to_owned()),
            description: "Clean up old files".to_owned(),
            active_state: "active".to_owned(),
            unit_file_state: "enabled".to_owned(),
            last_trigger_usec: NOW - secs(7_200),
            next_realtime_usec: 0,
            next_monotonic_usec: CLOCKS.monotonic_usec + secs(3_600),
            activates: "launchpadlib-cache-clean.service".to_owned(),
            service: None,
        }
    }

    fn tempo_spec() -> TimerSpec {
        TimerSpec {
            name: "clean-cache".to_owned(),
            description: "Clean the cache".to_owned(),
            command: Command {
                program: PathBuf::from("/home/user/clean.sh"),
                args: vec!["my files".to_owned()],
                working_dir: None,
            },
            schedule: Schedule::Every { minutes: 30 },
            catch_up: false,
            random_delay_secs: 0,
            conditions: vec![Condition::OnAcPower],
            low_priority: false,
            timeout_secs: None,
        }
    }

    #[test]
    fn a_vendor_timer_is_system_and_read_from_its_files() {
        let texts = Texts {
            timer: Some("[Timer]\nOnBootSec=3min\nOnUnitActiveSec=1d\n".to_owned()),
            service: Some("[Service]\nExecStart=/usr/bin/cleaner --all\n".to_owned()),
        };
        let view = to_view(&system_timer(), &texts, CLOCKS, HOME, &[]);
        assert_eq!(view.origin, Origin::System);
        assert_eq!(view.title, "Clean up old files");
        assert_eq!(view.description, "Clean up old files");
        assert_eq!(view.schedule, Schedule::Every { minutes: 1_440 });
        assert_eq!(view.command, "/usr/bin/cleaner --all");
        assert_eq!(view.next_in_secs, Some(3_600));
        assert_eq!(view.state, State::Waiting);
    }

    #[test]
    fn a_long_or_quoted_description_gives_a_readable_name_instead() {
        let mut raw = system_timer();
        raw.unit = "ubuntu-insights-upload.timer".to_owned();
        raw.description =
            "\"Run the upload 8 mins after boot and every week relative to the activation\""
                .to_owned();
        let view = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]);
        assert_eq!(view.title, "Ubuntu insights upload");
        assert!(view.description.starts_with("Run the upload"));
        assert!(!view.description.contains('"'));
        raw.description = String::new();
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).title,
            "Ubuntu insights upload"
        );
    }

    #[test]
    fn a_hand_written_timer_in_the_home_folder_is_personal() {
        let mut raw = system_timer();
        raw.fragment_path = Some("/home/user/.config/systemd/user/mine.timer".to_owned());
        let view = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]);
        assert_eq!(view.origin, Origin::Personal);
        // Without the files, the schedule is unknown and the service is named.
        assert_eq!(view.command, "launchpadlib-cache-clean.service");
    }

    #[test]
    fn home_is_matched_as_a_folder_not_a_prefix() {
        let mut raw = system_timer();
        raw.fragment_path = Some("/home/username/x.timer".to_owned());
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).origin,
            Origin::System
        );
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, "", &[]).origin,
            Origin::System
        );
    }

    #[test]
    fn a_timer_written_by_tempo_is_yours() {
        let spec = tempo_spec();
        let texts = Texts {
            timer: Some(tempo_core::unit::render_timer(&spec).expect("renders")),
            service: Some(tempo_core::unit::render_service(&spec).expect("renders")),
        };
        let mut raw = system_timer();
        raw.unit = "clean-cache.timer".to_owned();
        raw.fragment_path = Some("/home/user/.config/systemd/user/clean-cache.timer".to_owned());
        let view = to_view(&raw, &texts, CLOCKS, HOME, &[]);
        assert_eq!(view.origin, Origin::Yours);
        assert_eq!(view.title, "Clean the cache");
        assert_eq!(view.schedule, Schedule::Every { minutes: 30 });
        assert_eq!(view.command, "/home/user/clean.sh \"my files\"");
    }

    #[test]
    fn a_stopped_timer_is_paused_and_has_no_next_run() {
        let mut raw = system_timer();
        raw.active_state = "inactive".to_owned();
        let view = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]);
        assert_eq!(view.state, State::Paused);
        assert_eq!(view.next_in_secs, None);
    }

    #[test]
    fn the_next_run_is_the_nearest_of_the_two_clocks() {
        let mut raw = system_timer();
        raw.next_realtime_usec = NOW + secs(600);
        let view = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]);
        assert_eq!(view.next_in_secs, Some(600));
        raw.next_realtime_usec = 0;
        raw.next_monotonic_usec = 0;
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).next_in_secs,
            None
        );
        // Overdue counts as now.
        raw.next_realtime_usec = NOW - secs(5);
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).next_in_secs,
            Some(0)
        );
    }

    fn service(result: &str, status: i32, start: u64, exit: u64) -> RawService {
        RawService {
            fragment_path: None,
            active_state: "inactive".to_owned(),
            result: result.to_owned(),
            exec_main_status: status,
            start_usec: start,
            exit_usec: exit,
        }
    }

    #[test]
    fn a_successful_run_is_read_from_the_service() {
        let mut raw = system_timer();
        raw.service = Some(service("success", 0, NOW - secs(130), NOW - secs(120)));
        let run = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).runs[0].clone();
        assert_eq!(
            run,
            Run {
                result: RunResult::Success,
                ago_secs: 120,
                duration_secs: 10,
                exit_code: Some(0)
            }
        );
    }

    #[test]
    fn results_are_put_in_three_words() {
        for (text, expected) in [
            ("success", RunResult::Success),
            ("skipped", RunResult::Skipped),
            ("exit-code", RunResult::Failed),
            ("timeout", RunResult::Failed),
            ("signal", RunResult::Failed),
            ("", RunResult::Unknown),
        ] {
            let mut raw = system_timer();
            raw.service = Some(service(text, 1, NOW - secs(5), NOW - secs(4)));
            let view = to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]);
            assert_eq!(view.runs[0].result, expected, "{text:?}");
        }
    }

    #[test]
    fn the_journal_history_wins_over_the_last_run_of_the_service() {
        let history = vec![
            Run {
                result: RunResult::Failed,
                ago_secs: 7_000,
                duration_secs: 1,
                exit_code: Some(2),
            },
            Run {
                result: RunResult::Success,
                ago_secs: 100,
                duration_secs: 1,
                exit_code: Some(0),
            },
        ];
        let view = to_view(&system_timer(), &Texts::default(), CLOCKS, HOME, &history);
        assert_eq!(view.runs, history);
        assert_eq!(view.service, "launchpadlib-cache-clean.service");
    }

    #[test]
    fn a_service_gone_from_memory_leaves_only_the_moment() {
        let view = to_view(&system_timer(), &Texts::default(), CLOCKS, HOME, &[]);
        assert_eq!(
            view.runs,
            vec![Run {
                result: RunResult::Unknown,
                ago_secs: 7_200,
                duration_secs: 0,
                exit_code: None
            }]
        );
        let mut never = system_timer();
        never.last_trigger_usec = 0;
        assert!(
            to_view(&never, &Texts::default(), CLOCKS, HOME, &[])
                .runs
                .is_empty()
        );
    }

    #[test]
    fn a_running_service_makes_the_timer_running() {
        let mut raw = system_timer();
        let mut running = service("success", 0, NOW - secs(60), 0);
        running.active_state = "activating".to_owned();
        raw.service = Some(running);
        assert_eq!(
            to_view(&raw, &Texts::default(), CLOCKS, HOME, &[]).state,
            State::Running
        );
    }

    #[test]
    fn failed_timers_come_first_then_the_soonest() {
        let make = |title: &str, next: Option<i64>, result: RunResult| TimerView {
            unit: format!("{title}.timer"),
            service: format!("{title}.service"),
            title: title.to_owned(),
            description: String::new(),
            origin: Origin::Yours,
            state: State::Waiting,
            enabled: true,
            schedule: Schedule::Every { minutes: 1 },
            next_in_secs: next,
            runs: vec![Run {
                result,
                ago_secs: 1,
                duration_secs: 1,
                exit_code: None,
            }],
            command: String::new(),
            file: String::new(),
        };
        let mut timers = vec![
            make("late", Some(900), RunResult::Success),
            make("none", None, RunResult::Success),
            make("bad", Some(5_000), RunResult::Failed),
            make("soon", Some(60), RunResult::Success),
        ];
        sort(&mut timers);
        let order: Vec<&str> = timers.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(order, vec!["bad", "soon", "late", "none"]);
    }
}
