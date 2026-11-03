// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! What the interface shows about a timer, without any widget: the data comes
//! from the demo today and from systemd later. Pure, so it is tested.

use tempo_core::schedule::Schedule;

/// What the owner can ask of a timer Tempo wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    RunNow,
    Pause,
    Resume,
    TurnOff,
    TurnOn,
    Delete,
    /// Opens the form; it needs no systemd call.
    Edit,
}

/// Past runs drawn in the strip.
pub const STRIP_LEN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    Success,
    Failed,
    Skipped,
    /// It ran, but systemd no longer remembers how it ended.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Written by this app: fully editable.
    Yours,
    /// Written by hand in the owner's folder: shown, never changed.
    Personal,
    /// Provided by the system or an app: shown, never changed.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Waiting,
    Running,
    Paused,
}

/// The one-word summary that picks a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Ok,
    Failed,
    Running,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub result: RunResult,
    pub ago_secs: i64,
    pub duration_secs: u32,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerView {
    /// The timer unit: `clean-app-cache.timer`.
    pub unit: String,
    pub title: String,
    /// The service the timer starts: `clean.service`.
    pub service: String,
    /// What the unit says about itself, when it says it in a sentence.
    pub description: String,
    pub origin: Origin,
    pub state: State,
    /// Starts by itself with each session.
    pub enabled: bool,
    pub schedule: Schedule,
    pub next_in_secs: Option<i64>,
    /// Oldest first.
    pub runs: Vec<Run>,
    pub command: String,
    pub file: String,
}

impl TimerView {
    pub fn last_run(&self) -> Option<&Run> {
        self.runs.last()
    }

    pub fn health(&self) -> Health {
        match self.state {
            State::Running => Health::Running,
            State::Paused => Health::Idle,
            State::Waiting => match self.last_run().map(|run| run.result) {
                Some(RunResult::Failed) => Health::Failed,
                Some(RunResult::Success) => Health::Ok,
                Some(RunResult::Skipped | RunResult::Unknown) | None => Health::Idle,
            },
        }
    }

    /// The last [`STRIP_LEN`] runs, newest on the right; `None` for the empty
    /// places on the left.
    pub fn strip(&self) -> [Option<&Run>; STRIP_LEN] {
        let mut slots = [None; STRIP_LEN];
        let shown = self.runs.len().min(STRIP_LEN);
        for (slot, run) in slots[STRIP_LEN - shown..]
            .iter_mut()
            .zip(&self.runs[self.runs.len() - shown..])
        {
            *slot = Some(run);
        }
        slots
    }

    /// Succeeded, failed and skipped among the runs of the strip.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);
        for run in self.strip().into_iter().flatten() {
            match run.result {
                RunResult::Success => counts.0 += 1,
                RunResult::Failed => counts.1 += 1,
                RunResult::Skipped => counts.2 += 1,
                RunResult::Unknown => {}
            }
        }
        counts
    }
}

/// How far, in the words people use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Now,
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative {
    pub future: bool,
    pub span: Span,
}

/// `delta_secs` is positive for the future.
pub fn relative(delta_secs: i64) -> Relative {
    let secs = delta_secs.unsigned_abs();
    let span = if secs < 60 {
        Span::Now
    } else if secs < 3_600 {
        Span::Minutes(secs / 60)
    } else if secs < 86_400 {
        Span::Hours(secs / 3_600)
    } else {
        Span::Days(secs / 86_400)
    };
    Relative {
        future: delta_secs >= 0,
        span,
    }
}

/// The timers that will fire, soonest first. A paused timer or one with no
/// known next time is not in the list.
pub fn upcoming(timers: &[TimerView]) -> Vec<&TimerView> {
    let mut list: Vec<&TimerView> = timers
        .iter()
        .filter(|timer| timer.state != State::Paused && timer.next_in_secs.is_some())
        .collect();
    list.sort_by_key(|timer| timer.next_in_secs);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(result: RunResult) -> Run {
        Run {
            result,
            ago_secs: 60,
            duration_secs: 1,
            exit_code: None,
        }
    }

    fn timer(state: State, next: Option<i64>, runs: Vec<Run>) -> TimerView {
        TimerView {
            unit: "t.timer".to_owned(),
            title: "T".to_owned(),
            service: "t.service".to_owned(),
            description: String::new(),
            origin: Origin::Yours,
            state,
            enabled: true,
            schedule: Schedule::Every { minutes: 30 },
            next_in_secs: next,
            runs,
            command: "/bin/true".to_owned(),
            file: "/f".to_owned(),
        }
    }

    #[test]
    fn health_follows_the_last_run() {
        let ok = timer(
            State::Waiting,
            None,
            vec![run(RunResult::Failed), run(RunResult::Success)],
        );
        assert_eq!(ok.health(), Health::Ok);
        let bad = timer(
            State::Waiting,
            None,
            vec![run(RunResult::Success), run(RunResult::Failed)],
        );
        assert_eq!(bad.health(), Health::Failed);
        let skipped = timer(State::Waiting, None, vec![run(RunResult::Skipped)]);
        assert_eq!(skipped.health(), Health::Idle);
        assert_eq!(timer(State::Waiting, None, vec![]).health(), Health::Idle);
    }

    #[test]
    fn running_and_paused_win_over_the_last_run() {
        let failed = vec![run(RunResult::Failed)];
        assert_eq!(
            timer(State::Running, None, failed.clone()).health(),
            Health::Running
        );
        assert_eq!(timer(State::Paused, None, failed).health(), Health::Idle);
    }

    #[test]
    fn the_strip_keeps_the_newest_runs_on_the_right() {
        let few = timer(
            State::Waiting,
            None,
            vec![run(RunResult::Failed), run(RunResult::Success)],
        );
        let slots = few.strip();
        assert!(slots[..8].iter().all(Option::is_none));
        assert_eq!(slots[8].map(|r| r.result), Some(RunResult::Failed));
        assert_eq!(slots[9].map(|r| r.result), Some(RunResult::Success));

        let mut many = vec![run(RunResult::Failed); 3];
        many.extend(vec![run(RunResult::Success); STRIP_LEN]);
        let many = timer(State::Waiting, None, many);
        assert!(
            many.strip()
                .iter()
                .all(|slot| slot.map(|r| r.result) == Some(RunResult::Success))
        );
        assert_eq!(many.counts(), (STRIP_LEN, 0, 0));
    }

    #[test]
    fn counts_split_the_strip() {
        let runs = vec![
            run(RunResult::Success),
            run(RunResult::Failed),
            run(RunResult::Skipped),
            run(RunResult::Success),
        ];
        assert_eq!(timer(State::Waiting, None, runs).counts(), (2, 1, 1));
    }

    #[test]
    fn relative_times_use_the_biggest_fitting_unit() {
        let cases = [
            (0, true, Span::Now),
            (59, true, Span::Now),
            (60, true, Span::Minutes(1)),
            (12 * 60 + 30, true, Span::Minutes(12)),
            (3_599, true, Span::Minutes(59)),
            (3_600, true, Span::Hours(1)),
            (86_399, true, Span::Hours(23)),
            (86_400 * 3 + 5, true, Span::Days(3)),
            (-30, false, Span::Now),
            (-(3 * 60), false, Span::Minutes(3)),
            (-(2 * 86_400), false, Span::Days(2)),
        ];
        for (secs, future, span) in cases {
            assert_eq!(relative(secs), Relative { future, span }, "{secs}");
        }
    }

    #[test]
    fn upcoming_skips_paused_timers_and_sorts_by_time() {
        let timers = vec![
            timer(State::Waiting, Some(900), vec![]),
            timer(State::Paused, Some(10), vec![]),
            timer(State::Waiting, None, vec![]),
            timer(State::Running, Some(60), vec![]),
        ];
        let next: Vec<_> = upcoming(&timers).iter().map(|t| t.next_in_secs).collect();
        assert_eq!(next, vec![Some(60), Some(900)]);
    }
}
