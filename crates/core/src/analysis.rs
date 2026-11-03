// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Looks at a file the owner wants to run on a schedule, without running it:
//! what it is, whether a timer can start it, what may go wrong, and the command
//! that would start it. Pure: the caller reads the file and tells what the
//! system has ([`FileFacts`], [`Env`]).
//!
//! The checks are codes ([`Id`]) with a [`Level`], a word of detail and a fix:
//! the interface writes the sentences, in the owner's language.

use std::path::{Path, PathBuf};

use crate::spec::Command;

/// Bytes of the file that are read: enough for a header and the top of a script.
pub const HEAD_BYTES: usize = 64 * 1024;

/// What the caller read of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    /// Absolute path, as the timer would run it.
    pub path: PathBuf,
    /// The path with symbolic links followed, for judging where the file lives.
    pub real_path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    /// Unix permission bits.
    pub mode: u32,
    /// The first [`HEAD_BYTES`] bytes.
    pub head: Vec<u8>,
}

/// What the system has.
pub struct Env<'a> {
    pub home: &'a Path,
    /// `x86_64`, `aarch64`…
    pub arch: &'a str,
    /// Whether this absolute path exists.
    pub exists: &'a dyn Fn(&Path) -> bool,
    /// Where a program name is found in `PATH`.
    pub find: &'a dyn Fn(&str) -> Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Directory,
    /// A compiled program; the architecture as the system names it, when known.
    Program {
        arch: Option<String>,
        appimage: bool,
    },
    /// Text that an interpreter runs, by its name (`bash`, `python3`…).
    Script {
        interpreter: String,
        shebang: bool,
    },
    /// Text that no interpreter is known for.
    Text,
    /// Data that is neither.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    /// A timer cannot run it like this.
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Id {
    /// What kind of file it is; the check's `word` is the interpreter or the
    /// architecture.
    Kind,
    /// Windows line endings after `#!`: the interpreter is not found.
    LineEndings,
    Interpreter,
    Executable,
    Architecture,
    /// Where the file lives; the word is `temporary`, `downloads`, `removable`,
    /// `trash` or `stable`.
    Location,
    /// Paths relative to the file's folder.
    RelativePaths,
    /// `sudo` and the like: no terminal to ask a password in.
    Privileges,
    /// Waits for the keyboard.
    Input,
    /// Calls a desktop tool; the word is its name.
    Desktop,
    /// `rm -r` with a variable or a wildcard.
    RecursiveDelete,
}

/// What can be done about a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// A command to copy and run in a terminal.
    Command(String),
    /// Tempo starts the file from its own folder.
    WorkingDirectory,
    /// Tempo can keep a copy of the file in its own folder.
    KeepCopy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: Id,
    pub level: Level,
    /// The one word the sentence needs (see [`Id`]).
    pub word: String,
    pub fix: Option<Fix>,
}

/// What the form of the timer starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// A valid unit name made from the file's name.
    pub name: String,
    /// A short title made from the file's name.
    pub description: String,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub kind: Kind,
    pub checks: Vec<Check>,
    pub suggestion: Suggestion,
}

impl Analysis {
    /// The worst level among the checks.
    pub fn worst(&self) -> Level {
        self.checks
            .iter()
            .map(|check| check.level)
            .fold(Level::Ok, |worst, level| match (worst, level) {
                (Level::Problem, _) | (_, Level::Problem) => Level::Problem,
                (Level::Warn, _) | (_, Level::Warn) => Level::Warn,
                _ => Level::Ok,
            })
    }
}

fn check(id: Id, level: Level, word: &str, fix: Option<Fix>) -> Check {
    Check {
        id,
        level,
        word: word.to_owned(),
        fix,
    }
}

/// A path as a word for a shell, in single quotes.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

// ---- Kind ---------------------------------------------------------------

fn elf_arch(head: &[u8]) -> Option<&'static str> {
    // `e_machine` is two bytes at offset 18, in the file's byte order.
    let low = *head.get(18)?;
    let high = *head.get(19)?;
    let machine = if head.get(5) == Some(&2) {
        u16::from_be_bytes([low, high])
    } else {
        u16::from_le_bytes([low, high])
    };
    Some(match machine {
        0x03 => "x86",
        0x28 => "arm",
        0x3E => "x86_64",
        0xB7 => "aarch64",
        0xF3 => "riscv64",
        0x15 => "powerpc64",
        _ => return None,
    })
}

/// The interpreter named by the `#!` line: `(name, absolute path if given)`.
/// `#!/usr/bin/env -S python3 -u` names `python3`.
fn shebang(head: &[u8]) -> Option<(String, Option<PathBuf>)> {
    let line_end = head.iter().position(|&b| b == b'\n').unwrap_or(head.len());
    let line = String::from_utf8_lossy(&head[..line_end]);
    let line = line.strip_prefix("#!")?.trim();
    let mut words = line.split_whitespace();
    let first = words.next()?;
    let base = Path::new(first).file_name()?.to_str()?;
    if base == "env" {
        let name = words.find(|word| !word.starts_with('-'))?;
        return Some((name.to_owned(), None));
    }
    Some((base.to_owned(), Some(PathBuf::from(first))))
}

fn interpreter_for_extension(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "sh" | "bash" => Some("bash"),
        "py" => Some("python3"),
        "js" | "mjs" => Some("node"),
        "pl" => Some("perl"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        _ => None,
    }
}

fn is_text(head: &[u8]) -> bool {
    !head.contains(&0)
        && String::from_utf8_lossy(head)
            .chars()
            .filter(|&c| c == '\u{fffd}')
            .count()
            * 50
            < head.len().max(1)
}

// ---- Reading a script ---------------------------------------------------

/// The commands of a script as lists of words: a line is cut at `; | & ( ) { }`,
/// and comments are left out. Quotes stay: this is a look, not a parse.
fn commands(text: &str) -> Vec<Vec<&str>> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .flat_map(|line| line.split([';', '|', '&', '(', ')', '{', '}']))
        .map(|command| command.split_whitespace().collect::<Vec<_>>())
        .filter(|words| !words.is_empty())
        .collect()
}

/// Words that run the next one: `sudo rm -rf x` is an `rm`.
const WRAPPERS: [&str; 8] = [
    "sudo", "doas", "xargs", "exec", "command", "nice", "ionice", "time",
];

/// A command without the words that wrap it.
fn bare<'a, 'b>(command: &'b [&'a str]) -> &'b [&'a str] {
    let skip = command
        .iter()
        .take_while(|word| WRAPPERS.contains(word))
        .count();
    &command[skip..]
}

const PRIVILEGE_TOOLS: [&str; 3] = ["sudo", "doas", "pkexec"];
const DESKTOP_TOOLS: [&str; 10] = [
    "notify-send",
    "zenity",
    "kdialog",
    "xdg-open",
    "xdotool",
    "xclip",
    "xsel",
    "wl-copy",
    "wl-paste",
    "gsettings",
];

fn first_word_in<'a>(lines: &[Vec<&'a str>], tools: &[&str]) -> Option<&'a str> {
    lines
        .iter()
        .flatten()
        .copied()
        .find(|word| tools.contains(word))
}

fn waits_for_input(lines: &[Vec<&str>], interpreter: &str, text: &str) -> bool {
    if interpreter.starts_with("python") {
        return text.contains("input(");
    }
    lines.iter().any(|line| bare(line).first() == Some(&"read"))
}

fn deletes_recursively(lines: &[Vec<&str>]) -> bool {
    lines.iter().any(|line| {
        let command = bare(line);
        if command.first() != Some(&"rm") {
            return false;
        }
        let rest = &command[1..];
        let recursive = rest.iter().any(|word| {
            (word.starts_with('-')
                && !word.starts_with("--")
                && word[1..].chars().any(|c| c == 'r' || c == 'R'))
                || *word == "--recursive"
        });
        recursive
            && rest
                .iter()
                .any(|word| word.contains('$') || word.contains('*'))
    })
}

fn uses_relative_paths(lines: &[Vec<&str>]) -> bool {
    lines.iter().flatten().any(|word| {
        let word = word.trim_matches(['"', '\'']);
        word.starts_with("./") || word.starts_with("../")
    })
}

// ---- Where the file lives -----------------------------------------------

fn place(real: &Path, home: &Path) -> &'static str {
    let starts = |prefix: &str| real.starts_with(prefix);
    if starts("/tmp") || starts("/var/tmp") || starts("/dev/shm") {
        return "temporary";
    }
    if starts("/media") || starts("/run/media") || starts("/mnt") {
        return "removable";
    }
    if let Ok(inside) = real.strip_prefix(home) {
        let mut parts = inside.components().filter_map(|c| c.as_os_str().to_str());
        let top = parts.next().unwrap_or_default().to_lowercase();
        if ["downloads", "téléchargements", "descargas", "scaricati"].contains(&top.as_str()) {
            return "downloads";
        }
        if top == ".local" && parts.next() == Some("share") && parts.next() == Some("Trash") {
            return "trash";
        }
    }
    "stable"
}

// ---- The suggestion -----------------------------------------------------

/// A unit name from a file name: lowercase letters, digits and `-`.
pub fn slug(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);
    let mut name = String::new();
    for c in stem.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_matches('-');
    let name: String = name.chars().take(60).collect();
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "my-timer".to_owned()
    } else {
        name.to_owned()
    }
}

/// “clean-stremio_cache.sh” becomes “Clean stremio cache”.
pub fn title(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);
    let spaced: String = stem
        .chars()
        .map(|c| if matches!(c, '-' | '_' | '.') { ' ' } else { c })
        .collect();
    let spaced = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = spaced.chars();
    let title = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
        None => String::new(),
    };
    if title.is_empty() || title.chars().count() > crate::spec::MAX_DESCRIPTION {
        "My timer".to_owned()
    } else {
        title
    }
}

// ---- The analysis --------------------------------------------------------

/// Looks at the file. Nothing is run and nothing is written.
pub fn analyze(facts: &FileFacts, env: &Env<'_>) -> Analysis {
    let file_name = facts
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("timer")
        .to_owned();
    let mut checks = Vec::new();
    let executable = facts.mode & 0o111 != 0;
    let text = String::from_utf8_lossy(&facts.head).into_owned();

    // What it is.
    let mut interpreter_path: Option<PathBuf> = None;
    let kind = if facts.is_dir {
        checks.push(check(Id::Kind, Level::Problem, "directory", None));
        Kind::Directory
    } else if facts.head.starts_with(b"\x7fELF") {
        let arch = elf_arch(&facts.head).map(str::to_owned);
        let appimage = facts
            .head
            .get(8..11)
            .is_some_and(|m| m == b"AI\x02" || m == b"AI\x01");
        checks.push(check(
            Id::Kind,
            Level::Ok,
            arch.as_deref().unwrap_or("unknown"),
            None,
        ));
        if let Some(arch) = &arch
            && arch != env.arch
        {
            checks.push(check(Id::Architecture, Level::Problem, arch, None));
        }
        if !executable {
            let fix = format!(
                "chmod +x {}",
                shell_quote(&facts.path.display().to_string())
            );
            checks.push(check(
                Id::Executable,
                Level::Problem,
                "program",
                Some(Fix::Command(fix)),
            ));
        }
        Kind::Program { arch, appimage }
    } else if let Some((name, given)) = shebang(&facts.head) {
        checks.push(check(Id::Kind, Level::Ok, &name, None));
        // The interpreter must be there, or the timer fails at once.
        interpreter_path = match &given {
            Some(path) if (env.exists)(path) => Some(path.clone()),
            Some(_) => None,
            None => (env.find)(&name),
        };
        if interpreter_path.is_none() {
            checks.push(check(Id::Interpreter, Level::Problem, &name, None));
        }
        let first_line = text.lines().next().unwrap_or_default();
        let crlf = facts.head.windows(2).take(256).any(|pair| pair == b"\r\n");
        if crlf && !first_line.is_empty() {
            let fix = format!(
                "sed -i 's/\\r$//' {}",
                shell_quote(&facts.path.display().to_string())
            );
            checks.push(check(
                Id::LineEndings,
                Level::Problem,
                &name,
                Some(Fix::Command(fix)),
            ));
        }
        Kind::Script {
            interpreter: name,
            shebang: true,
        }
    } else if is_text(&facts.head) {
        match interpreter_for_extension(&facts.path) {
            Some(name) => {
                checks.push(check(Id::Kind, Level::Ok, name, None));
                interpreter_path = (env.find)(name);
                if interpreter_path.is_none() {
                    checks.push(check(Id::Interpreter, Level::Problem, name, None));
                }
                Kind::Script {
                    interpreter: name.to_owned(),
                    shebang: false,
                }
            }
            None => {
                checks.push(check(Id::Kind, Level::Problem, "text", None));
                Kind::Text
            }
        }
    } else {
        checks.push(check(Id::Kind, Level::Problem, "data", None));
        Kind::Unknown
    };

    // A script with an interpreter but no executable bit still runs, through
    // its interpreter: say so rather than ask the owner to change the file.
    if let Kind::Script { interpreter, .. } = &kind
        && !executable
        && interpreter_path.is_some()
    {
        checks.push(check(Id::Executable, Level::Ok, interpreter, None));
    }

    // Where it lives.
    if !facts.is_dir {
        let spot = place(&facts.real_path, env.home);
        let (level, fix) = if spot == "stable" {
            (Level::Ok, None)
        } else {
            (Level::Warn, Some(Fix::KeepCopy))
        };
        checks.push(check(Id::Location, level, spot, fix));
    }

    // What a script does.
    let mut relative = false;
    if let Kind::Script { interpreter, .. } = &kind {
        let lines = commands(&text);
        if uses_relative_paths(&lines) {
            relative = true;
            checks.push(check(
                Id::RelativePaths,
                Level::Warn,
                "",
                Some(Fix::WorkingDirectory),
            ));
        }
        if let Some(tool) = first_word_in(&lines, &PRIVILEGE_TOOLS) {
            checks.push(check(Id::Privileges, Level::Warn, tool, None));
        }
        if waits_for_input(&lines, interpreter, &text) {
            checks.push(check(Id::Input, Level::Warn, "", None));
        }
        if let Some(tool) = first_word_in(&lines, &DESKTOP_TOOLS) {
            checks.push(check(Id::Desktop, Level::Warn, tool, None));
        }
        if deletes_recursively(&lines) {
            checks.push(check(Id::RecursiveDelete, Level::Warn, "", None));
        }
    }

    // How it would be started.
    let path = facts.path.clone();
    let (program, args) = match &kind {
        Kind::Program { .. } => (path.clone(), Vec::new()),
        Kind::Script { shebang: true, .. } if executable => (path.clone(), Vec::new()),
        Kind::Script { .. } => match &interpreter_path {
            Some(interpreter) => (interpreter.clone(), vec![path.display().to_string()]),
            None => (path.clone(), Vec::new()),
        },
        _ => (path.clone(), Vec::new()),
    };
    let working_dir = relative
        .then(|| facts.path.parent().map(Path::to_path_buf))
        .flatten();

    Analysis {
        kind,
        checks,
        suggestion: Suggestion {
            name: slug(&file_name),
            description: title(&file_name),
            command: Command {
                program,
                args,
                working_dir,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(path: &str, mode: u32, head: &[u8]) -> FileFacts {
        FileFacts {
            path: PathBuf::from(path),
            real_path: PathBuf::from(path),
            is_dir: false,
            size: head.len() as u64,
            mode,
            head: head.to_vec(),
        }
    }

    fn run(facts: &FileFacts) -> Analysis {
        let exists = |path: &Path| {
            matches!(
                path.to_str(),
                Some("/bin/bash" | "/usr/bin/python3" | "/bin/sh")
            )
        };
        let find = |name: &str| match name {
            "bash" => Some(PathBuf::from("/usr/bin/bash")),
            "python3" => Some(PathBuf::from("/usr/bin/python3")),
            "node" => Some(PathBuf::from("/usr/bin/node")),
            _ => None,
        };
        analyze(
            facts,
            &Env {
                home: Path::new("/home/user"),
                arch: "x86_64",
                exists: &exists,
                find: &find,
            },
        )
    }

    fn has(analysis: &Analysis, id: Id, level: Level) -> bool {
        analysis
            .checks
            .iter()
            .any(|c| c.id == id && c.level == level)
    }

    fn missing(analysis: &Analysis, id: Id) -> bool {
        analysis.checks.iter().all(|c| c.id != id)
    }

    const GOOD: &[u8] = b"#!/bin/bash\nrm -f /home/user/.cache/app/*.tmp\necho done\n";

    #[test]
    fn a_plain_executable_script_is_started_as_it_is() {
        let analysis = run(&facts("/home/user/bin/clean.sh", 0o755, GOOD));
        assert_eq!(
            analysis.kind,
            Kind::Script {
                interpreter: "bash".to_owned(),
                shebang: true
            }
        );
        assert_eq!(analysis.worst(), Level::Ok);
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/home/user/bin/clean.sh")
        );
        assert!(analysis.suggestion.command.args.is_empty());
        assert_eq!(analysis.suggestion.name, "clean");
        assert_eq!(analysis.suggestion.description, "Clean");
    }

    #[test]
    fn a_script_that_is_not_executable_runs_through_its_interpreter() {
        let analysis = run(&facts("/home/user/bin/clean.sh", 0o644, GOOD));
        assert!(has(&analysis, Id::Executable, Level::Ok));
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/bin/bash")
        );
        assert_eq!(
            analysis.suggestion.command.args,
            vec!["/home/user/bin/clean.sh"]
        );
        assert_eq!(analysis.worst(), Level::Ok);
    }

    #[test]
    fn env_in_the_shebang_is_looked_up_in_the_path() {
        let head = b"#!/usr/bin/env -S python3 -u\nprint('hi')\n";
        let analysis = run(&facts("/home/user/x.py", 0o644, head));
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/usr/bin/python3")
        );
        let head = b"#!/usr/bin/env fancylang\nx\n";
        assert!(has(
            &run(&facts("/home/user/x", 0o755, head)),
            Id::Interpreter,
            Level::Problem
        ));
    }

    #[test]
    fn a_missing_interpreter_is_a_problem() {
        let head = b"#!/opt/weird/bin/runner\nx\n";
        let analysis = run(&facts("/home/user/x", 0o755, head));
        assert!(has(&analysis, Id::Interpreter, Level::Problem));
        assert_eq!(analysis.worst(), Level::Problem);
    }

    #[test]
    fn windows_line_endings_after_the_shebang_are_caught_with_their_fix() {
        let head = b"#!/bin/bash\r\necho hi\r\n";
        let analysis = run(&facts("/home/user/my script.sh", 0o755, head));
        let check = analysis
            .checks
            .iter()
            .find(|c| c.id == Id::LineEndings)
            .expect("noticed");
        assert_eq!(check.level, Level::Problem);
        assert_eq!(
            check.fix,
            Some(Fix::Command(
                "sed -i 's/\\r$//' '/home/user/my script.sh'".to_owned()
            ))
        );
    }

    #[test]
    fn an_extension_stands_in_for_a_missing_shebang() {
        let analysis = run(&facts("/home/user/backup.sh", 0o644, b"echo hi\n"));
        assert_eq!(
            analysis.kind,
            Kind::Script {
                interpreter: "bash".to_owned(),
                shebang: false
            }
        );
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/usr/bin/bash")
        );
        // Even if the bit is set, no shebang means the interpreter must be named.
        let analysis = run(&facts("/home/user/backup.sh", 0o755, b"echo hi\n"));
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/usr/bin/bash")
        );
        assert_eq!(
            analysis.suggestion.command.args,
            vec!["/home/user/backup.sh"]
        );
    }

    #[test]
    fn unknown_text_and_data_cannot_be_run() {
        let analysis = run(&facts("/home/user/notes.txt", 0o644, b"just words\n"));
        assert_eq!(analysis.kind, Kind::Text);
        assert!(has(&analysis, Id::Kind, Level::Problem));
        let analysis = run(&facts("/home/user/blob", 0o644, &[0, 159, 146, 150, 0, 1]));
        assert_eq!(analysis.kind, Kind::Unknown);
        assert_eq!(analysis.worst(), Level::Problem);
    }

    #[test]
    fn a_folder_cannot_be_run() {
        let mut dir = facts("/home/user/stuff", 0o755, b"");
        dir.is_dir = true;
        let analysis = run(&dir);
        assert_eq!(analysis.kind, Kind::Directory);
        assert!(missing(&analysis, Id::Location));
        assert_eq!(analysis.worst(), Level::Problem);
    }

    fn elf(machine: u16, big_endian: bool) -> Vec<u8> {
        let mut head = vec![
            0x7f,
            b'E',
            b'L',
            b'F',
            2,
            if big_endian { 2 } else { 1 },
            1,
            0,
        ];
        head.extend_from_slice(&[0; 10]);
        let bytes = if big_endian {
            machine.to_be_bytes()
        } else {
            machine.to_le_bytes()
        };
        head.extend_from_slice(&bytes);
        head
    }

    #[test]
    fn a_program_is_recognised_by_its_architecture() {
        let analysis = run(&facts("/home/user/bin/tool", 0o755, &elf(0x3E, false)));
        assert_eq!(
            analysis.kind,
            Kind::Program {
                arch: Some("x86_64".to_owned()),
                appimage: false
            }
        );
        assert_eq!(analysis.worst(), Level::Ok);
        assert_eq!(
            analysis.suggestion.command.program,
            PathBuf::from("/home/user/bin/tool")
        );
        let arm = run(&facts("/home/user/bin/tool", 0o755, &elf(0xB7, false)));
        assert!(has(&arm, Id::Architecture, Level::Problem));
        let be = run(&facts("/home/user/bin/tool", 0o755, &elf(0x3E, true)));
        assert_eq!(
            be.kind,
            Kind::Program {
                arch: Some("x86_64".to_owned()),
                appimage: false
            }
        );
    }

    #[test]
    fn a_program_without_the_executable_bit_comes_with_the_fix() {
        let analysis = run(&facts("/home/user/bin/tool", 0o644, &elf(0x3E, false)));
        let check = analysis
            .checks
            .iter()
            .find(|c| c.id == Id::Executable)
            .expect("noticed");
        assert_eq!(check.level, Level::Problem);
        assert_eq!(
            check.fix,
            Some(Fix::Command("chmod +x '/home/user/bin/tool'".to_owned()))
        );
    }

    #[test]
    fn an_appimage_is_recognised() {
        let mut head = elf(0x3E, false);
        head.resize(8, 0);
        head[0..4].copy_from_slice(b"\x7fELF");
        head.splice(8..8, b"AI\x02".iter().copied());
        head.resize(20, 0);
        let analysis = run(&facts("/home/user/Apps/x.AppImage", 0o755, &head));
        assert!(matches!(
            analysis.kind,
            Kind::Program { appimage: true, .. }
        ));
    }

    #[test]
    fn unstable_places_are_warned_about() {
        for (path, word) in [
            ("/tmp/job.sh", "temporary"),
            ("/home/user/Downloads/job.sh", "downloads"),
            ("/home/user/Téléchargements/job.sh", "downloads"),
            ("/media/user/USB/job.sh", "removable"),
            ("/run/media/user/USB/job.sh", "removable"),
            ("/home/user/.local/share/Trash/files/job.sh", "trash"),
        ] {
            let analysis = run(&facts(path, 0o755, GOOD));
            let check = analysis
                .checks
                .iter()
                .find(|c| c.id == Id::Location)
                .expect("checked");
            assert_eq!(
                (check.level, check.word.as_str()),
                (Level::Warn, word),
                "{path}"
            );
            assert_eq!(check.fix, Some(Fix::KeepCopy));
        }
        let steady = run(&facts("/home/user/bin/job.sh", 0o755, GOOD));
        let check = steady
            .checks
            .iter()
            .find(|c| c.id == Id::Location)
            .expect("checked");
        assert_eq!((check.level, check.word.as_str()), (Level::Ok, "stable"));
    }

    #[test]
    fn a_link_is_judged_by_where_it_leads() {
        let mut link = facts("/home/user/bin/job.sh", 0o755, GOOD);
        link.real_path = PathBuf::from("/tmp/job.sh");
        assert!(has(&run(&link), Id::Location, Level::Warn));
    }

    #[test]
    fn relative_paths_start_the_script_from_its_folder() {
        let head = b"#!/bin/bash\nsource ./lib.sh\ncat \"./data.txt\"\n";
        let analysis = run(&facts("/home/user/bin/job.sh", 0o755, head));
        assert!(has(&analysis, Id::RelativePaths, Level::Warn));
        assert_eq!(
            analysis.suggestion.command.working_dir,
            Some(PathBuf::from("/home/user/bin"))
        );
        let none = run(&facts("/home/user/bin/job.sh", 0o755, GOOD));
        assert_eq!(none.suggestion.command.working_dir, None);
    }

    #[test]
    fn a_comment_is_not_a_command() {
        let head = b"#!/bin/bash\n# sudo make me a sandwich ./here\necho ok\n";
        let analysis = run(&facts("/home/user/bin/job.sh", 0o755, head));
        assert!(missing(&analysis, Id::Privileges));
        assert!(missing(&analysis, Id::RelativePaths));
    }

    #[test]
    fn privileges_input_and_desktop_calls_are_warned_about() {
        let head = b"#!/bin/bash\nsudo apt update\nread -p 'ok? ' answer\nnotify-send done\n";
        let analysis = run(&facts("/home/user/bin/job.sh", 0o755, head));
        let word = |id| {
            analysis
                .checks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.word.clone())
        };
        assert_eq!(word(Id::Privileges).as_deref(), Some("sudo"));
        assert!(has(&analysis, Id::Input, Level::Warn));
        assert_eq!(word(Id::Desktop).as_deref(), Some("notify-send"));
        assert_eq!(analysis.worst(), Level::Warn);
    }

    #[test]
    fn python_input_is_noticed_but_readlink_is_not_read() {
        let python = b"#!/usr/bin/python3\nname = input('who? ')\n";
        assert!(has(
            &run(&facts("/home/user/x.py", 0o755, python)),
            Id::Input,
            Level::Warn
        ));
        let shell =
            b"#!/bin/bash\nreadlink -f \"$0\"\nwhile read line; do echo $line; done < file\n";
        // `read` after `while` is not the first word of a command here, and a
        // loop over a file does not wait for the keyboard.
        assert!(missing(
            &run(&facts("/home/user/x.sh", 0o755, shell)),
            Id::Input
        ));
    }

    #[test]
    fn a_recursive_delete_with_a_variable_or_wildcard_is_flagged() {
        let flagged = |line: &str| {
            let head = format!("#!/bin/bash\n{line}\n");
            has(
                &run(&facts("/home/user/x.sh", 0o755, head.as_bytes())),
                Id::RecursiveDelete,
                Level::Warn,
            )
        };
        for line in [
            "rm -rf $DIR",
            "rm -r \"$HOME\"/cache/*",
            "rm -Rf /data/*",
            "rm --recursive $X",
            "cd /x && rm -rf ./*",
            "find . | xargs rm -rf $X",
        ] {
            assert!(flagged(line), "{line}");
        }
        for line in [
            "rm -f /tmp/one.tmp",
            "rm -rf /var/cache/fixed",
            "rm $FILE",
            "echo rm -rf $X",
            "# rm -rf $X",
        ] {
            assert!(!flagged(line), "{line}");
        }
    }

    #[test]
    fn names_are_unit_safe() {
        for (file, name, title_) in [
            (
                "clean-Stremio_cache.sh",
                "clean-stremio-cache",
                "Clean Stremio cache",
            ),
            ("My Backup (v2).py", "my-backup-v2", "My Backup (v2)"),
            ("éé.sh", "my-timer", "Éé"),
            ("...", "my-timer", "My timer"),
            ("a.tar.gz", "a-tar", "A tar"),
        ] {
            assert_eq!(slug(file), name, "{file}");
            assert_eq!(title(file), title_, "{file}");
        }
        assert!(slug(&"x".repeat(200)).len() <= 60);
    }

    #[test]
    fn suggested_names_pass_the_timer_rules() {
        use crate::schedule::Schedule;
        use crate::spec::TimerSpec;
        for file in [
            "clean.sh",
            "Mon Script (copie).py",
            "éé.sh",
            "9lives.sh",
            "a.b.c.sh",
        ] {
            let analysis = run(&facts(&format!("/home/user/{file}"), 0o755, GOOD));
            let spec = TimerSpec {
                name: analysis.suggestion.name.clone(),
                description: analysis.suggestion.description.clone(),
                command: analysis.suggestion.command.clone(),
                schedule: Schedule::Every { minutes: 30 },
                catch_up: false,
                random_delay_secs: 0,
                conditions: Vec::new(),
                low_priority: false,
                timeout_secs: None,
            };
            assert_eq!(spec.validate(), Ok(()), "{file}");
        }
    }

    #[test]
    fn the_worst_level_wins() {
        let mut analysis = run(&facts("/home/user/bin/job.sh", 0o755, GOOD));
        assert_eq!(analysis.worst(), Level::Ok);
        analysis
            .checks
            .push(check(Id::Input, Level::Warn, "", None));
        assert_eq!(analysis.worst(), Level::Warn);
        analysis
            .checks
            .push(check(Id::Interpreter, Level::Problem, "x", None));
        assert_eq!(analysis.worst(), Level::Problem);
    }

    #[test]
    fn shell_quoting_survives_a_quote() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("plain"), "'plain'");
    }
}
