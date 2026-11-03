// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! What the owner's journal says about the timers' services, through
//! `journalctl --user --output=json` (no library, no root): the history of the
//! runs, and the lines of the logs.
//!
//! systemd writes one message when a job ends, with `JOB_RESULT`, and a
//! `USER_UNIT` that names the service; a run is one such message. The ids below
//! come from systemd's own catalogue and were read on a real journal.

use std::collections::HashMap;
use std::ffi::OsStr;

use gtk::gio;

use crate::view::{Run, RunResult, STRIP_LEN};

/// A job started: “Starting X…”.
const START_ID: &str = "7d4958e842da4a758f6c1cdc7b36dcc5";
/// The service's main process exited: `EXIT_STATUS`.
const EXIT_ID: &str = "98e322203f7a4ed290d09fe03c09fe15";
/// A job ended: “Finished X”, “Started X”, or “X skipped…”.
const DONE_ID: &str = "39f53479d3a045ac8e11786248231fbf";
/// A job ended in failure: “Failed to start X”.
const FAILED_ID: &str = "be02cf6855d2428ba40df7e9d022f03d";

/// How many job messages are read at once, for all the timers.
const HISTORY_LINES: &str = "2000";
/// How far back the history looks.
const HISTORY_SINCE: &str = "-60d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// Microseconds since the epoch.
    pub time_usec: u64,
    pub priority: u8,
    pub message: String,
    pub message_id: String,
    /// The user unit the line belongs to.
    pub unit: String,
    pub invocation: String,
    pub exit_status: Option<i32>,
}

impl Entry {
    /// Colours a line: the priority first, then a few words that programs put
    /// in their own output (which the journal files at the normal priority).
    pub fn severity(&self) -> Severity {
        if self.priority <= 3 {
            return Severity::Error;
        }
        if self.priority == 4 {
            return Severity::Warning;
        }
        // The lines systemd writes about a successful job say so itself.
        if self.message_id == DONE_ID || self.message_id == START_ID {
            return Severity::Info;
        }
        let lower = self.message.to_lowercase();
        if ["error", "fatal", "panic", "traceback"]
            .iter()
            .any(|word| lower.contains(word))
        {
            Severity::Error
        } else if lower.contains("warn") {
            Severity::Warning
        } else {
            Severity::Info
        }
    }
}

// ---- A flat JSON object, as journalctl writes it ------------------------

/// Reads one line of `journalctl -o json`: an object of strings (or, for text
/// that is not UTF-8, arrays of byte values), without anything nested deeper.
fn flat_object(text: &str) -> Option<HashMap<String, String>> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    parser.space();
    parser.expect(b'{')?;
    let mut map = HashMap::new();
    parser.space();
    if parser.peek() == Some(b'}') {
        return Some(map);
    }
    loop {
        parser.space();
        let key = parser.string()?;
        parser.space();
        parser.expect(b':')?;
        parser.space();
        let value = parser.value()?;
        map.insert(key, value);
        parser.space();
        match parser.next()? {
            b',' => {}
            b'}' => return Some(map),
            _ => return None,
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.at += 1;
        Some(byte)
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        (self.next()? == byte).then_some(())
    }

    fn space(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let mut value = 0;
        for _ in 0..4 {
            value = value * 16 + char::from(self.next()?).to_digit(16)?;
        }
        Some(value)
    }

    fn string(&mut self) -> Option<String> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.next()? {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let c = match self.next()? {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'/' => '/',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                // A pair: the next escape holds the low half.
                                self.expect(b'\\')?;
                                self.expect(b'u')?;
                                let second = self.hex4()?;
                                0x10000 + ((first - 0xD800) << 10) + second.checked_sub(0xDC00)?
                            } else {
                                first
                            };
                            char::from_u32(code)?
                        }
                        _ => return None,
                    };
                    let mut buffer = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
                byte => out.push(byte),
            }
        }
    }

    /// A string, an array of byte values (text that is not UTF-8), or a word
    /// such as `null` or a number, kept as written.
    fn value(&mut self) -> Option<String> {
        match self.peek()? {
            b'"' => self.string(),
            b'[' => {
                self.at += 1;
                let mut bytes = Vec::new();
                loop {
                    self.space();
                    if self.peek() == Some(b']') {
                        self.at += 1;
                        return Some(String::from_utf8_lossy(&bytes).into_owned());
                    }
                    let start = self.at;
                    while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                        self.at += 1;
                    }
                    bytes.push(
                        std::str::from_utf8(&self.bytes[start..self.at])
                            .ok()?
                            .parse()
                            .ok()?,
                    );
                    self.space();
                    if self.peek() == Some(b',') {
                        self.at += 1;
                    }
                }
            }
            _ => {
                let start = self.at;
                while self
                    .peek()
                    .is_some_and(|b| !matches!(b, b',' | b'}') && !b.is_ascii_whitespace())
                {
                    self.at += 1;
                }
                let word = std::str::from_utf8(&self.bytes[start..self.at]).ok()?;
                Some(if word == "null" {
                    String::new()
                } else {
                    word.to_owned()
                })
            }
        }
    }
}

/// One line of `journalctl -o json` as an [`Entry`].
pub fn parse_line(line: &str) -> Option<Entry> {
    let fields = flat_object(line)?;
    let get = |key: &str| fields.get(key).map_or("", String::as_str);
    let unit = [get("USER_UNIT"), get("_SYSTEMD_USER_UNIT")]
        .into_iter()
        .find(|unit| !unit.is_empty() && *unit != "init.scope")
        .unwrap_or_default();
    Some(Entry {
        time_usec: get("__REALTIME_TIMESTAMP").parse().ok()?,
        priority: get("PRIORITY").parse().unwrap_or(6),
        message: get("MESSAGE").to_owned(),
        message_id: get("MESSAGE_ID").to_owned(),
        unit: unit.to_owned(),
        invocation: get("USER_INVOCATION_ID").to_owned(),
        exit_status: get("EXIT_STATUS").parse().ok(),
    })
}

// ---- The history of the runs ---------------------------------------------

/// The runs of one service, oldest first, at most [`STRIP_LEN`], from its job
/// messages (`entries` oldest first).
pub fn runs(entries: &[Entry], service: &str, now_usec: u64) -> Vec<Run> {
    let mut started: HashMap<&str, u64> = HashMap::new();
    let mut exits: HashMap<&str, i32> = HashMap::new();
    let mut found = Vec::new();
    for entry in entries.iter().filter(|entry| entry.unit == service) {
        let invocation = entry.invocation.as_str();
        match entry.message_id.as_str() {
            START_ID if !invocation.is_empty() => {
                started.insert(invocation, entry.time_usec);
            }
            EXIT_ID if !invocation.is_empty() => {
                if let Some(code) = entry.exit_status {
                    exits.insert(invocation, code);
                }
            }
            DONE_ID | FAILED_ID => {
                let result = if entry.message_id == FAILED_ID {
                    RunResult::Failed
                } else if entry.message.contains("skipped") {
                    RunResult::Skipped
                } else {
                    RunResult::Success
                };
                let took = started.get(invocation).map_or(0, |start| {
                    entry.time_usec.saturating_sub(*start) / 1_000_000
                });
                found.push(Run {
                    result,
                    ago_secs: i64::try_from(now_usec.saturating_sub(entry.time_usec) / 1_000_000)
                        .unwrap_or(i64::MAX),
                    duration_secs: u32::try_from(took).unwrap_or(u32::MAX),
                    exit_code: match result {
                        RunResult::Success => Some(0),
                        RunResult::Failed => exits.get(invocation).copied(),
                        RunResult::Skipped | RunResult::Unknown => None,
                    },
                });
            }
            _ => {}
        }
    }
    let skip = found.len().saturating_sub(STRIP_LEN);
    found.split_off(skip)
}

/// The arguments of the one `journalctl` call that reads the job messages of
/// all these services.
fn history_args(services: &[String]) -> Vec<String> {
    let mut args: Vec<String> = [
        "--user",
        "--output=json",
        "--no-pager",
        "--quiet",
        "-n",
        HISTORY_LINES,
        "--since",
        HISTORY_SINCE,
    ]
    .map(str::to_owned)
    .to_vec();
    // Matches on one field are alternatives, those on two fields are both
    // required: (one of these units) and (one of these messages).
    args.extend(
        services
            .iter()
            .map(|service| format!("USER_UNIT={service}")),
    );
    args.extend([START_ID, EXIT_ID, DONE_ID, FAILED_ID].map(|id| format!("MESSAGE_ID={id}")));
    args
}

/// Any program as a process whose output can be read.
pub fn spawn_program(
    program: &str,
    args: &[String],
    flags: gio::SubprocessFlags,
) -> Result<gio::Subprocess, String> {
    let mut words: Vec<&OsStr> = vec![OsStr::new(program)];
    words.extend(args.iter().map(OsStr::new));
    gio::Subprocess::newv(&words, flags).map_err(|err| err.to_string())
}

/// `journalctl` as a process whose output can be read as it comes.
pub fn spawn(args: &[String], flags: gio::SubprocessFlags) -> Result<gio::Subprocess, String> {
    let mut words: Vec<&OsStr> = vec![OsStr::new("journalctl")];
    words.extend(args.iter().map(OsStr::new));
    gio::Subprocess::newv(&words, flags).map_err(|err| err.to_string())
}

/// The runs of each service, from the journal. A journal that cannot be read
/// gives nothing: the interface then keeps what systemd itself remembers.
pub async fn history(services: &[String], now_usec: u64) -> HashMap<String, Vec<Run>> {
    if services.is_empty() {
        return HashMap::new();
    }
    let process = match spawn(
        &history_args(services),
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    ) {
        Ok(process) => process,
        Err(err) => {
            gtk::glib::g_warning!("tempo", "Cannot start journalctl: {err}");
            return HashMap::new();
        }
    };
    let Ok((Some(output), _)) = process.communicate_utf8_future(None::<String>).await else {
        return HashMap::new();
    };
    let mut entries: Vec<Entry> = output.lines().filter_map(parse_line).collect();
    entries.sort_by_key(|entry| entry.time_usec);
    services
        .iter()
        .map(|service| (service.clone(), runs(&entries, service, now_usec)))
        .collect()
}

/// The arguments that read the lines of a timer and of its service: the last
/// ones, and the new ones as they come when `follow` is set.
pub fn log_args(timer: &str, service: &str, follow: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "--user",
        "--output=json",
        "--no-pager",
        "--quiet",
        "-n",
        "300",
    ]
    .map(str::to_owned)
    .to_vec();
    if follow {
        args.push("--follow".to_owned());
    }
    args.extend(["-u", timer, "-u", service].map(str::to_owned));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000_000_000;

    fn line(time_offset_secs: i64, fields: &[(&str, &str)]) -> String {
        let time = NOW as i64 + time_offset_secs * 1_000_000;
        let mut text = format!("{{\"__REALTIME_TIMESTAMP\":\"{time}\"");
        for (key, value) in fields {
            text.push_str(&format!(",\"{key}\":\"{value}\""));
        }
        text.push('}');
        text
    }

    fn entry(offset: i64, id: &str, inv: &str, message: &str, extra: &[(&str, &str)]) -> Entry {
        let mut fields = vec![
            ("USER_UNIT", "t.service"),
            ("MESSAGE_ID", id),
            ("USER_INVOCATION_ID", inv),
            ("MESSAGE", message),
        ];
        fields.extend_from_slice(extra);
        parse_line(&line(offset, &fields)).expect("parses")
    }

    #[test]
    fn a_json_line_becomes_an_entry() {
        let text = line(
            0,
            &[
                ("MESSAGE", "Finished t.service."),
                ("PRIORITY", "6"),
                ("USER_UNIT", "t.service"),
                ("MESSAGE_ID", DONE_ID),
                ("EXIT_STATUS", "1"),
            ],
        );
        let entry = parse_line(&text).expect("parses");
        assert_eq!(entry.time_usec, NOW);
        assert_eq!(entry.message, "Finished t.service.");
        assert_eq!(entry.unit, "t.service");
        assert_eq!(entry.exit_status, Some(1));
    }

    #[test]
    fn the_user_unit_falls_back_to_the_unit_that_wrote_the_line() {
        let text = line(0, &[("_SYSTEMD_USER_UNIT", "x.service"), ("MESSAGE", "hi")]);
        assert_eq!(parse_line(&text).expect("parses").unit, "x.service");
        let text = line(
            0,
            &[("_SYSTEMD_USER_UNIT", "init.scope"), ("MESSAGE", "hi")],
        );
        assert_eq!(parse_line(&text).expect("parses").unit, "");
    }

    #[test]
    fn json_escapes_and_unicode_are_read() {
        let text = r#"{"__REALTIME_TIMESTAMP":"5","MESSAGE":"a\"b\\c\né 😀 é"}"#;
        assert_eq!(parse_line(text).expect("parses").message, "a\"b\\c\né 😀 é");
    }

    #[test]
    fn text_that_is_not_utf8_comes_as_bytes() {
        let text = r#"{"__REALTIME_TIMESTAMP":"5","MESSAGE":[104,105,255,33],"X":null}"#;
        let entry = parse_line(text).expect("parses");
        assert_eq!(entry.message, "hi\u{fffd}!");
    }

    #[test]
    fn broken_lines_are_skipped() {
        for text in [
            "",
            "not json",
            "{",
            "{\"a\":",
            "{\"a\":\"b\"",
            "[1,2]",
            "{\"MESSAGE\":\"x\"}",
        ] {
            assert_eq!(parse_line(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_successful_run_has_its_duration() {
        let entries = [
            entry(-100, START_ID, "a", "Starting t.service...", &[]),
            entry(
                -93,
                DONE_ID,
                "a",
                "Finished t.service.",
                &[("JOB_RESULT", "done")],
            ),
        ];
        let runs = runs(&entries, "t.service", NOW);
        assert_eq!(
            runs,
            vec![Run {
                result: RunResult::Success,
                ago_secs: 93,
                duration_secs: 7,
                exit_code: Some(0)
            }]
        );
    }

    #[test]
    fn a_failed_run_keeps_its_exit_code() {
        let entries = [
            entry(-50, START_ID, "b", "Starting t.service...", &[]),
            entry(
                -49,
                EXIT_ID,
                "b",
                "Main process exited",
                &[("EXIT_STATUS", "3")],
            ),
            entry(
                -49,
                FAILED_ID,
                "b",
                "Failed to start t.service.",
                &[("JOB_RESULT", "failed")],
            ),
        ];
        let run = &runs(&entries, "t.service", NOW)[0];
        assert_eq!(run.result, RunResult::Failed);
        assert_eq!(run.exit_code, Some(3));
        assert_eq!(run.duration_secs, 1);
    }

    #[test]
    fn a_run_skipped_by_a_condition_is_a_skip() {
        let entries = [
            entry(
                -30,
                DONE_ID,
                "",
                "t.service skipped, unmet condition check ConditionPathExists=/x",
                &[],
            ),
            entry(-20, START_ID, "c", "Starting t.service...", &[]),
            entry(
                -20,
                DONE_ID,
                "c",
                "Condition check resulted in t.service being skipped.",
                &[],
            ),
        ];
        let runs = runs(&entries, "t.service", NOW);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().all(|run| run.result == RunResult::Skipped));
        assert!(runs.iter().all(|run| run.exit_code.is_none()));
    }

    #[test]
    fn other_services_and_other_messages_are_left_out() {
        let mut other = entry(-10, DONE_ID, "d", "Finished other.service.", &[]);
        other.unit = "other.service".to_owned();
        let noise = entry(
            -9,
            "0e4284a0caca4bfc81c0bb6786972673",
            "e",
            "Skipped due to exec-condition",
            &[],
        );
        assert!(runs(&[other, noise], "t.service", NOW).is_empty());
    }

    #[test]
    fn only_the_last_ten_runs_are_kept_oldest_first() {
        let entries: Vec<Entry> = (0..14)
            .map(|n| entry(-1_000 + n * 10, DONE_ID, "", "Finished t.service.", &[]))
            .collect();
        let kept = runs(&entries, "t.service", NOW);
        assert_eq!(kept.len(), STRIP_LEN);
        assert!(
            kept.windows(2)
                .all(|pair| pair[0].ago_secs > pair[1].ago_secs)
        );
        assert_eq!(kept[0].ago_secs, 1_000 - 40);
    }

    #[test]
    fn lines_get_a_severity() {
        let make = |priority: u8, id: &str, message: &str| Entry {
            priority,
            message_id: id.to_owned(),
            message: message.to_owned(),
            ..Entry::default()
        };
        assert_eq!(make(3, "", "x").severity(), Severity::Error);
        assert_eq!(make(4, "", "x").severity(), Severity::Warning);
        assert_eq!(
            make(6, DONE_ID, "Finished failed-to-clean.service").severity(),
            Severity::Info
        );
        assert_eq!(
            make(6, "", "2026/11/03 ERROR failed to upload").severity(),
            Severity::Error
        );
        assert_eq!(make(6, "", "WARN retrying").severity(), Severity::Warning);
        assert_eq!(make(6, "", "all good").severity(), Severity::Info);
    }

    #[test]
    fn the_history_call_matches_units_and_messages() {
        let args = history_args(&["a.service".to_owned(), "b.service".to_owned()]);
        assert!(args.contains(&"USER_UNIT=a.service".to_owned()));
        assert!(args.contains(&"USER_UNIT=b.service".to_owned()));
        assert_eq!(
            args.iter().filter(|a| a.starts_with("MESSAGE_ID=")).count(),
            4
        );
        assert_eq!(args[0], "--user");
    }

    #[test]
    fn the_log_call_follows_only_when_asked() {
        let args = log_args("t.timer", "t.service", false);
        assert!(!args.contains(&"--follow".to_owned()));
        assert!(log_args("t.timer", "t.service", true).contains(&"--follow".to_owned()));
        assert!(args.windows(2).any(|w| w == ["-u", "t.timer"]));
    }
}
