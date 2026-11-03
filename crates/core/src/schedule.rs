// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! When a timer fires: the forms the owner picks from, and what systemd needs.

use std::fmt;

/// "Every X minutes" is limited to 30 days, which is plenty.
pub const MAX_EVERY_MINUTES: u32 = 43_200;
/// Longest custom calendar expression.
pub const MAX_EXPRESSION: usize = 200;
/// First run of an "every X minutes" timer, at most this long after the
/// session starts: waiting a whole day for the first run would surprise.
const FIRST_RUN_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    /// The three-letter name systemd reads.
    pub fn short(self) -> &'static str {
        match self {
            Weekday::Mon => "Mon",
            Weekday::Tue => "Tue",
            Weekday::Wed => "Wed",
            Weekday::Thu => "Thu",
            Weekday::Fri => "Fri",
            Weekday::Sat => "Sat",
            Weekday::Sun => "Sun",
        }
    }

    fn from_short(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|day| day.short().eq_ignore_ascii_case(text))
    }
}

/// How the owner picks when a timer fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schedule {
    /// Every `minutes`, counted from the last run.
    Every {
        minutes: u32,
    },
    /// Every hour, at this minute.
    Hourly {
        minute: u8,
    },
    Daily {
        hour: u8,
        minute: u8,
    },
    Weekly {
        days: Vec<Weekday>,
        hour: u8,
        minute: u8,
    },
    /// On this day of the month, 1 to 28 so that no month skips it.
    Monthly {
        day: u8,
        hour: u8,
        minute: u8,
    },
    /// A systemd calendar expression, checked further by `systemd-analyze`.
    Custom {
        expression: String,
    },
}

/// What goes in the `[Timer]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Timing {
    /// `OnCalendar=`: a wall-clock time.
    Calendar(String),
    /// `OnBootSec=` then `OnUnitActiveSec=`, in seconds.
    Monotonic { first_secs: u64, repeat_secs: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    Interval(u32),
    Hour(u8),
    Minute(u8),
    Day(u8),
    NoWeekday,
    Expression(&'static str),
    Key(String),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleError::Interval(n) => {
                write!(f, "{n} minutes is not between 1 and {MAX_EVERY_MINUTES}")
            }
            ScheduleError::Hour(n) => write!(f, "hour {n} is not between 0 and 23"),
            ScheduleError::Minute(n) => write!(f, "minute {n} is not between 0 and 59"),
            ScheduleError::Day(n) => write!(f, "day {n} is not between 1 and 28"),
            ScheduleError::NoWeekday => write!(f, "pick at least one day of the week"),
            ScheduleError::Expression(reason) => write!(f, "calendar expression: {reason}"),
            ScheduleError::Key(text) => write!(f, "unreadable schedule “{text}”"),
        }
    }
}

impl std::error::Error for ScheduleError {}

fn check_time(hour: u8, minute: u8) -> Result<(), ScheduleError> {
    if hour > 23 {
        return Err(ScheduleError::Hour(hour));
    }
    check_minute(minute)
}

fn check_minute(minute: u8) -> Result<(), ScheduleError> {
    if minute > 59 {
        return Err(ScheduleError::Minute(minute));
    }
    Ok(())
}

fn check_expression(expression: &str) -> Result<(), ScheduleError> {
    let text = expression.trim();
    if text.is_empty() {
        return Err(ScheduleError::Expression("it is empty"));
    }
    if text.chars().count() > MAX_EXPRESSION {
        return Err(ScheduleError::Expression("it is too long"));
    }
    if text.chars().any(char::is_control) {
        return Err(ScheduleError::Expression("it has a control character"));
    }
    Ok(())
}

impl Schedule {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        match self {
            Schedule::Every { minutes } => {
                if (1..=MAX_EVERY_MINUTES).contains(minutes) {
                    Ok(())
                } else {
                    Err(ScheduleError::Interval(*minutes))
                }
            }
            Schedule::Hourly { minute } => check_minute(*minute),
            Schedule::Daily { hour, minute } => check_time(*hour, *minute),
            Schedule::Weekly { days, hour, minute } => {
                if days.is_empty() {
                    return Err(ScheduleError::NoWeekday);
                }
                check_time(*hour, *minute)
            }
            Schedule::Monthly { day, hour, minute } => {
                if !(1..=28).contains(day) {
                    return Err(ScheduleError::Day(*day));
                }
                check_time(*hour, *minute)
            }
            Schedule::Custom { expression } => check_expression(expression),
        }
    }

    /// What systemd is told.
    pub fn timing(&self) -> Result<Timing, ScheduleError> {
        self.validate()?;
        Ok(match self {
            Schedule::Every { minutes } => {
                let repeat_secs = u64::from(*minutes) * 60;
                Timing::Monotonic {
                    first_secs: repeat_secs.min(FIRST_RUN_SECS),
                    repeat_secs,
                }
            }
            Schedule::Hourly { minute } => Timing::Calendar(format!("*-*-* *:{minute:02}:00")),
            Schedule::Daily { hour, minute } => {
                Timing::Calendar(format!("*-*-* {hour:02}:{minute:02}:00"))
            }
            Schedule::Weekly { days, hour, minute } => {
                Timing::Calendar(format!("{} *-*-* {hour:02}:{minute:02}:00", day_list(days)))
            }
            Schedule::Monthly { day, hour, minute } => {
                Timing::Calendar(format!("*-*-{day:02} {hour:02}:{minute:02}:00"))
            }
            Schedule::Custom { expression } => Timing::Calendar(expression.trim().to_owned()),
        })
    }

    /// Whether `Persistent=` (catching up after the computer was off) means
    /// anything: it only does for wall-clock times.
    pub fn is_calendar(&self) -> bool {
        !matches!(self, Schedule::Every { .. })
    }

    /// The form stored in the unit files, to read the schedule back exactly.
    pub fn to_key(&self) -> String {
        match self {
            Schedule::Every { minutes } => format!("every {minutes}"),
            Schedule::Hourly { minute } => format!("hourly {minute}"),
            Schedule::Daily { hour, minute } => format!("daily {hour:02}:{minute:02}"),
            Schedule::Weekly { days, hour, minute } => {
                format!("weekly {} {hour:02}:{minute:02}", day_list(days))
            }
            Schedule::Monthly { day, hour, minute } => {
                format!("monthly {day} {hour:02}:{minute:02}")
            }
            Schedule::Custom { expression } => format!("custom {}", expression.trim()),
        }
    }

    pub fn from_key(key: &str) -> Result<Self, ScheduleError> {
        let bad = || ScheduleError::Key(key.to_owned());
        let (kind, rest) = key.split_once(' ').ok_or_else(bad)?;
        let schedule = match kind {
            "every" => Schedule::Every {
                minutes: rest.parse().map_err(|_| bad())?,
            },
            "hourly" => Schedule::Hourly {
                minute: rest.parse().map_err(|_| bad())?,
            },
            "daily" => {
                let (hour, minute) = parse_time(rest).ok_or_else(bad)?;
                Schedule::Daily { hour, minute }
            }
            "weekly" => {
                let (days, time) = rest.split_once(' ').ok_or_else(bad)?;
                let mut list = Vec::new();
                for name in days.split(',') {
                    list.push(Weekday::from_short(name).ok_or_else(bad)?);
                }
                list.sort();
                list.dedup();
                let (hour, minute) = parse_time(time).ok_or_else(bad)?;
                Schedule::Weekly {
                    days: list,
                    hour,
                    minute,
                }
            }
            "monthly" => {
                let (day, time) = rest.split_once(' ').ok_or_else(bad)?;
                let (hour, minute) = parse_time(time).ok_or_else(bad)?;
                Schedule::Monthly {
                    day: day.parse().map_err(|_| bad())?,
                    hour,
                    minute,
                }
            }
            "custom" => Schedule::Custom {
                expression: rest.to_owned(),
            },
            _ => return Err(bad()),
        };
        schedule.validate()?;
        Ok(schedule)
    }
}

/// Days in week order, without repeats: `Mon,Wed`.
fn day_list(days: &[Weekday]) -> String {
    let mut days = days.to_vec();
    days.sort();
    days.dedup();
    days.iter()
        .map(|day| day.short())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_time(text: &str) -> Option<(u8, u8)> {
    let (hour, minute) = text.split_once(':')?;
    Some((hour.parse().ok()?, minute.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar(schedule: &Schedule) -> String {
        match schedule.timing() {
            Ok(Timing::Calendar(text)) => text,
            other => panic!("not a calendar timing: {other:?}"),
        }
    }

    #[test]
    fn every_x_minutes_counts_from_the_last_run() {
        let timing = Schedule::Every { minutes: 30 }.timing();
        assert_eq!(
            timing,
            Ok(Timing::Monotonic {
                first_secs: 300,
                repeat_secs: 1800
            })
        );
        // A short interval does not wait longer than itself for the first run.
        assert_eq!(
            Schedule::Every { minutes: 2 }.timing(),
            Ok(Timing::Monotonic {
                first_secs: 120,
                repeat_secs: 120
            })
        );
    }

    #[test]
    fn calendar_forms() {
        assert_eq!(calendar(&Schedule::Hourly { minute: 5 }), "*-*-* *:05:00");
        assert_eq!(
            calendar(&Schedule::Daily {
                hour: 9,
                minute: 30
            }),
            "*-*-* 09:30:00"
        );
        assert_eq!(
            calendar(&Schedule::Monthly {
                day: 1,
                hour: 0,
                minute: 0
            }),
            "*-*-01 00:00:00"
        );
    }

    #[test]
    fn weekly_days_are_ordered_and_without_repeats() {
        let schedule = Schedule::Weekly {
            days: vec![Weekday::Fri, Weekday::Mon, Weekday::Fri],
            hour: 7,
            minute: 0,
        };
        assert_eq!(calendar(&schedule), "Mon,Fri *-*-* 07:00:00");
    }

    #[test]
    fn a_custom_expression_is_trimmed_and_kept() {
        let schedule = Schedule::Custom {
            expression: "  *-*-* 02:00:00  ".to_owned(),
        };
        assert_eq!(calendar(&schedule), "*-*-* 02:00:00");
    }

    #[test]
    fn out_of_range_values_are_refused() {
        assert_eq!(
            Schedule::Every { minutes: 0 }.validate(),
            Err(ScheduleError::Interval(0))
        );
        assert_eq!(
            Schedule::Every {
                minutes: MAX_EVERY_MINUTES + 1
            }
            .validate(),
            Err(ScheduleError::Interval(MAX_EVERY_MINUTES + 1))
        );
        assert_eq!(
            Schedule::Daily {
                hour: 24,
                minute: 0
            }
            .validate(),
            Err(ScheduleError::Hour(24))
        );
        assert_eq!(
            Schedule::Hourly { minute: 60 }.validate(),
            Err(ScheduleError::Minute(60))
        );
        assert_eq!(
            Schedule::Monthly {
                day: 29,
                hour: 0,
                minute: 0
            }
            .validate(),
            Err(ScheduleError::Day(29))
        );
        assert_eq!(
            Schedule::Weekly {
                days: vec![],
                hour: 0,
                minute: 0
            }
            .validate(),
            Err(ScheduleError::NoWeekday)
        );
    }

    #[test]
    fn bad_custom_expressions_are_refused() {
        for text in ["", "   ", "daily\nOnCalendar=hourly"] {
            let schedule = Schedule::Custom {
                expression: text.to_owned(),
            };
            assert!(
                matches!(schedule.validate(), Err(ScheduleError::Expression(_))),
                "{text:?}"
            );
        }
        let long = Schedule::Custom {
            expression: "x".repeat(MAX_EXPRESSION + 1),
        };
        assert!(long.validate().is_err());
    }

    #[test]
    fn keys_read_back_exactly() {
        let schedules = [
            Schedule::Every { minutes: 90 },
            Schedule::Hourly { minute: 15 },
            Schedule::Daily { hour: 9, minute: 5 },
            Schedule::Weekly {
                days: vec![Weekday::Mon, Weekday::Wed],
                hour: 18,
                minute: 45,
            },
            Schedule::Monthly {
                day: 28,
                hour: 3,
                minute: 0,
            },
            Schedule::Custom {
                expression: "Mon..Fri *-*-* 08:00:00".to_owned(),
            },
        ];
        for schedule in schedules {
            let key = schedule.to_key();
            assert_eq!(Schedule::from_key(&key), Ok(schedule), "{key}");
        }
    }

    #[test]
    fn unreadable_keys_are_refused() {
        for key in [
            "",
            "every",
            "every x",
            "daily 9",
            "weekly Xyz 09:00",
            "yearly 1",
            "daily 25:00",
        ] {
            assert!(Schedule::from_key(key).is_err(), "{key:?}");
        }
    }

    #[test]
    fn only_wall_clock_schedules_can_catch_up() {
        assert!(!Schedule::Every { minutes: 5 }.is_calendar());
        assert!(Schedule::Hourly { minute: 0 }.is_calendar());
    }
}
