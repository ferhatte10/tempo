// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Reading what the interface needs from the text of a unit file that Tempo did
//! not write: the schedule of a timer, the command of a service. Plain forms
//! only; anything else is shown as written.

use tempo_core::schedule::{MAX_EVERY_MINUTES, Schedule, Weekday};

/// Every value of `key=` in a unit file, in order. Comments are skipped and
/// spaces around the key and the value are ignored, as systemd does.
pub fn values<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#') && !line.starts_with(';'))
        .filter_map(|line| line.split_once('='))
        .filter(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim())
        .collect()
}

pub fn first<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    values(text, key).into_iter().next()
}

/// A systemd time span such as `30min`, `1h 30min` or a bare number of
/// seconds, in seconds. Only the common units.
pub fn parse_span_secs(text: &str) -> Option<u64> {
    let mut total = 0_u64;
    let mut any = false;
    let mut tokens = text.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        let digits = token.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let (number, mut unit) = token.split_at(digits);
        // “5 min”: the unit may be the next word.
        if unit.is_empty()
            && let Some(next) = tokens.peek()
            && next.starts_with(|c: char| c.is_ascii_alphabetic())
        {
            unit = next;
            tokens.next();
        }
        let number: u64 = number.parse().ok()?;
        let factor = match unit {
            "" | "s" | "sec" | "second" | "seconds" => 1,
            "m" | "min" | "minute" | "minutes" => 60,
            "h" | "hr" | "hour" | "hours" => 3_600,
            "d" | "day" | "days" => 86_400,
            "w" | "week" | "weeks" => 604_800,
            _ => return None,
        };
        total = total.checked_add(number.checked_mul(factor)?)?;
        any = true;
    }
    any.then_some(total)
}

/// The first command of the first `ExecStart=`, as written, without the
/// characters that change how systemd runs it (`-`, `@`, `+`, `!`, `:`).
pub fn exec_start(text: &str) -> Option<String> {
    values(text, "ExecStart")
        .into_iter()
        .map(|line| line.trim_start_matches(['-', '@', '+', '!', ':']).trim())
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

fn clock(text: &str) -> Option<(u8, u8)> {
    let mut parts = text.split(':');
    let hour: u8 = parts.next()?.parse().ok()?;
    let minute: u8 = parts.next()?.parse().ok()?;
    match parts.next() {
        None | Some("0" | "00") => {}
        Some(_) => return None,
    }
    parts.next().is_none().then_some((hour, minute))
}

/// A calendar expression in the words of the schedules Tempo knows, when it
/// is one of the plain forms; otherwise as written.
pub fn calendar(expression: &str) -> Schedule {
    let text = expression.trim();
    match text {
        "minutely" => return Schedule::Every { minutes: 1 },
        "hourly" => return Schedule::Hourly { minute: 0 },
        "daily" => return Schedule::Daily { hour: 0, minute: 0 },
        "weekly" => {
            return Schedule::Weekly {
                days: vec![Weekday::Mon],
                hour: 0,
                minute: 0,
            };
        }
        "monthly" => {
            return Schedule::Monthly {
                day: 1,
                hour: 0,
                minute: 0,
            };
        }
        _ => {}
    }
    if let Some(time) = text.strip_prefix("*-*-* ") {
        if let Some(minute) = time.strip_prefix("*:") {
            if let Some((_, minute)) = clock(&format!("0:{minute}")) {
                return Schedule::Hourly { minute };
            }
        } else if let Some((hour, minute)) = clock(time) {
            return Schedule::Daily { hour, minute };
        }
    }
    Schedule::Custom {
        expression: text.to_owned(),
    }
}

/// When a timer fires, read from its `[Timer]` section.
pub fn schedule(text: &str) -> Option<Schedule> {
    let calendars = values(text, "OnCalendar");
    if let [one] = calendars.as_slice() {
        return Some(calendar(one));
    }
    if !calendars.is_empty() {
        return Some(Schedule::Custom {
            expression: calendars.join(", "),
        });
    }
    if let Some(secs) = first(text, "OnUnitActiveSec").and_then(parse_span_secs) {
        let minutes = secs / 60;
        if secs % 60 == 0 && (1..=u64::from(MAX_EVERY_MINUTES)).contains(&minutes) {
            return Some(Schedule::Every {
                minutes: minutes as u32,
            });
        }
    }
    for key in [
        "OnUnitActiveSec",
        "OnUnitInactiveSec",
        "OnBootSec",
        "OnStartupSec",
        "OnActiveSec",
    ] {
        if let Some(value) = first(text, key) {
            return Some(Schedule::Custom {
                expression: format!("{key} {value}"),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_skip_comments_and_spaces() {
        let text = "[Timer]\n# OnCalendar=never\nOnCalendar = daily\n  OnCalendar=weekly \n;OnCalendar=x\nOther=1\n";
        assert_eq!(values(text, "OnCalendar"), vec!["daily", "weekly"]);
        assert_eq!(first(text, "Other"), Some("1"));
        assert_eq!(first(text, "Missing"), None);
    }

    #[test]
    fn time_spans() {
        for (text, secs) in [
            ("30", 30),
            ("30s", 30),
            ("30min", 1_800),
            ("1h 30min", 5_400),
            ("1d", 86_400),
            ("2 weeks", 1_209_600),
            ("5 minutes", 300),
        ] {
            assert_eq!(parse_span_secs(text), Some(secs), "{text}");
        }
        for text in ["", "min", "5 fortnights", "1.5h", "-3"] {
            assert_eq!(parse_span_secs(text), None, "{text:?}");
        }
    }

    #[test]
    fn exec_start_loses_its_prefixes() {
        let text = "[Service]\nExecStart=\nExecStart=-/usr/bin/x --flag\nExecStart=/other\n";
        assert_eq!(exec_start(text).as_deref(), Some("/usr/bin/x --flag"));
        assert_eq!(exec_start("[Service]\nType=oneshot\n"), None);
    }

    #[test]
    fn plain_calendars_are_put_in_words() {
        assert_eq!(calendar("daily"), Schedule::Daily { hour: 0, minute: 0 });
        assert_eq!(calendar("hourly"), Schedule::Hourly { minute: 0 });
        assert_eq!(calendar("minutely"), Schedule::Every { minutes: 1 });
        assert_eq!(
            calendar("*-*-* 06:30:00"),
            Schedule::Daily {
                hour: 6,
                minute: 30
            }
        );
        assert_eq!(
            calendar("*-*-* 06:30"),
            Schedule::Daily {
                hour: 6,
                minute: 30
            }
        );
        assert_eq!(calendar("*-*-* *:15:00"), Schedule::Hourly { minute: 15 });
    }

    #[test]
    fn other_calendars_stay_as_written() {
        for text in [
            "Mon..Fri *-*-* 09:00:00",
            "*-*-* *:00/30:00",
            "*-*-* 06:30:15",
            "*-*-* 25:00:00x",
        ] {
            assert_eq!(
                calendar(text),
                Schedule::Custom {
                    expression: text.to_owned()
                },
                "{text}"
            );
        }
    }

    #[test]
    fn schedules_of_timer_files() {
        assert_eq!(
            schedule("[Timer]\nOnCalendar=daily\n"),
            Some(Schedule::Daily { hour: 0, minute: 0 })
        );
        assert_eq!(
            schedule("[Timer]\nOnBootSec=5min\nOnUnitActiveSec=2h\n"),
            Some(Schedule::Every { minutes: 120 })
        );
        assert_eq!(
            schedule("[Timer]\nOnCalendar=daily\nOnCalendar=weekly\n"),
            Some(Schedule::Custom {
                expression: "daily, weekly".to_owned()
            })
        );
        assert_eq!(
            schedule("[Timer]\nOnBootSec=15min\n"),
            Some(Schedule::Custom {
                expression: "OnBootSec 15min".to_owned()
            })
        );
        assert_eq!(schedule("[Timer]\nPersistent=true\n"), None);
    }
}
