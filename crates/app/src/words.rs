// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The interface's sentences: a schedule, a time, a result, in plain words and
//! in the language of the system. Without a translation they are English, which
//! is what the tests read.

use gettextrs::{gettext, ngettext};
use tempo_core::schedule::{Schedule, Weekday};

use crate::view::{Relative, RunResult, Span};

/// Replaces the `{name}` placeholders of a translated sentence.
pub fn fill(text: &str, values: &[(&str, &str)]) -> String {
    let mut text = text.to_owned();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

fn count(text: String, n: u64) -> String {
    fill(&text, &[("n", &n.to_string())])
}

fn n32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// “30 min”, “2 h”, “1 day”: a length of time.
pub fn duration(secs: u64) -> String {
    let minutes = secs / 60;
    if minutes < 1 {
        count(gettext("{n} s"), secs)
    } else if secs.is_multiple_of(86_400) {
        count(
            ngettext("{n} day", "{n} days", n32(secs / 86_400)),
            secs / 86_400,
        )
    } else if secs.is_multiple_of(3_600) {
        count(gettext("{n} h"), secs / 3_600)
    } else {
        count(gettext("{n} min"), minutes)
    }
}

/// “in 12 min”, “3 h ago”.
pub fn when(relative: Relative) -> String {
    match (relative.future, relative.span) {
        (true, Span::Now) => gettext("in a moment"),
        (false, Span::Now) => gettext("just now"),
        (true, Span::Minutes(n)) => count(gettext("in {n} min"), n),
        (false, Span::Minutes(n)) => count(gettext("{n} min ago"), n),
        (true, Span::Hours(n)) => count(gettext("in {n} h"), n),
        (false, Span::Hours(n)) => count(gettext("{n} h ago"), n),
        (true, Span::Days(n)) => count(ngettext("in {n} day", "in {n} days", n32(n)), n),
        (false, Span::Days(n)) => count(ngettext("{n} day ago", "{n} days ago", n32(n)), n),
    }
}

pub fn result(result: RunResult) -> String {
    match result {
        RunResult::Success => gettext("Succeeded"),
        RunResult::Failed => gettext("Failed"),
        RunResult::Skipped => gettext("Skipped"),
        RunResult::Unknown => gettext("Result unknown"),
    }
}

pub fn weekday(day: Weekday) -> String {
    match day {
        Weekday::Mon => gettext("Mon"),
        Weekday::Tue => gettext("Tue"),
        Weekday::Wed => gettext("Wed"),
        Weekday::Thu => gettext("Thu"),
        Weekday::Fri => gettext("Fri"),
        Weekday::Sat => gettext("Sat"),
        Weekday::Sun => gettext("Sun"),
    }
}

fn time(hour: u8, minute: u8) -> String {
    format!("{hour:02}:{minute:02}")
}

/// “Every 30 minutes”, “Mon, Wed at 18:45”.
pub fn schedule(schedule: &Schedule) -> String {
    match schedule {
        Schedule::Every { minutes } => {
            let minutes = u64::from(*minutes);
            if minutes.is_multiple_of(1_440) {
                count(
                    ngettext("Every day", "Every {n} days", n32(minutes / 1_440)),
                    minutes / 1_440,
                )
            } else if minutes.is_multiple_of(60) {
                count(
                    ngettext("Every hour", "Every {n} hours", n32(minutes / 60)),
                    minutes / 60,
                )
            } else {
                count(
                    ngettext("Every minute", "Every {n} minutes", n32(minutes)),
                    minutes,
                )
            }
        }
        Schedule::Hourly { minute } => fill(
            &gettext("Every hour at :{minute}"),
            &[("minute", &format!("{minute:02}"))],
        ),
        Schedule::Daily { hour, minute } => fill(
            &gettext("Every day at {time}"),
            &[("time", &time(*hour, *minute))],
        ),
        Schedule::Weekly { days, hour, minute } => {
            let mut days = days.clone();
            days.sort();
            days.dedup();
            let names: Vec<String> = days.into_iter().map(weekday).collect();
            fill(
                &gettext("{days} at {time}"),
                &[("days", &names.join(", ")), ("time", &time(*hour, *minute))],
            )
        }
        Schedule::Monthly { day, hour, minute } => fill(
            &gettext("Day {day} of each month at {time}"),
            &[("day", &day.to_string()), ("time", &time(*hour, *minute))],
        ),
        Schedule::Custom { expression } => expression.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::relative;

    #[test]
    fn times_in_words() {
        assert_eq!(when(relative(12 * 60)), "in 12 min");
        assert_eq!(when(relative(-3 * 3_600)), "3 h ago");
        assert_eq!(when(relative(86_400)), "in 1 day");
        assert_eq!(when(relative(86_400 * 2)), "in 2 days");
        assert_eq!(when(relative(-86_400 * 2)), "2 days ago");
        assert_eq!(when(relative(5)), "in a moment");
        assert_eq!(when(relative(-5)), "just now");
    }

    #[test]
    fn lengths_of_time() {
        assert_eq!(duration(30), "30 s");
        assert_eq!(duration(300), "5 min");
        assert_eq!(duration(1_800), "30 min");
        assert_eq!(duration(7_200), "2 h");
        assert_eq!(duration(5_400), "90 min");
        assert_eq!(duration(86_400), "1 day");
        assert_eq!(duration(172_800), "2 days");
    }

    #[test]
    fn schedules_in_words() {
        let words = |s: Schedule| schedule(&s);
        assert_eq!(words(Schedule::Every { minutes: 1 }), "Every minute");
        assert_eq!(words(Schedule::Every { minutes: 30 }), "Every 30 minutes");
        assert_eq!(words(Schedule::Every { minutes: 60 }), "Every hour");
        assert_eq!(words(Schedule::Every { minutes: 120 }), "Every 2 hours");
        assert_eq!(words(Schedule::Every { minutes: 1_440 }), "Every day");
        assert_eq!(words(Schedule::Hourly { minute: 5 }), "Every hour at :05");
        assert_eq!(
            words(Schedule::Daily {
                hour: 7,
                minute: 30
            }),
            "Every day at 07:30"
        );
        assert_eq!(
            words(Schedule::Weekly {
                days: vec![Weekday::Wed, Weekday::Mon],
                hour: 18,
                minute: 45
            }),
            "Mon, Wed at 18:45"
        );
        assert_eq!(
            words(Schedule::Monthly {
                day: 1,
                hour: 3,
                minute: 0
            }),
            "Day 1 of each month at 03:00"
        );
        assert_eq!(
            words(Schedule::Custom {
                expression: "daily".to_owned()
            }),
            "daily"
        );
    }

    #[test]
    fn results_in_words() {
        assert_eq!(result(RunResult::Success), "Succeeded");
        assert_eq!(result(RunResult::Failed), "Failed");
        assert_eq!(result(RunResult::Unknown), "Result unknown");
    }
}
