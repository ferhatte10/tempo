// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The two unit files of a timer Tempo wrote, in the owner's folder. Nothing
//! else is ever read for writing, written or removed: a timer is touched only
//! after its files have been recognised as Tempo's.

use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tempo_core::spec::TimerSpec;
use tempo_core::unit;

/// A timer's two files, as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub timer: String,
    pub service: String,
}

/// `~/.config/systemd/user`, or under `XDG_CONFIG_HOME`.
pub fn user_dir() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|dir| !dir.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(config.join("systemd/user"))
}

fn paths(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("{name}.timer")),
        dir.join(format!("{name}.service")),
    )
}

/// The files of `name` when, and only when, Tempo wrote them. The name is
/// checked first, so it cannot point outside the folder.
pub fn managed(dir: &Path, name: &str) -> Result<(TimerSpec, Pair), String> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return Err("that is not a timer name".to_owned());
    }
    let (timer_path, service_path) = paths(dir, name);
    let read = |path: &Path| {
        std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))
    };
    let pair = Pair {
        timer: read(&timer_path)?,
        service: read(&service_path)?,
    };
    let spec = unit::parse(name, &pair.timer, &pair.service)
        .map_err(|err| format!("Tempo did not write this timer ({err})"))?;
    Ok((spec, pair))
}

/// Writes a file whole or not at all, readable by everyone, writable by the
/// owner only, like the files systemd ships.
fn write_file(path: &Path, text: &str) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&temporary)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

pub fn write_pair(dir: &Path, name: &str, pair: &Pair) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let (timer_path, service_path) = paths(dir, name);
    // The service first: a timer never points at a missing one.
    write_file(&service_path, &pair.service)?;
    write_file(&timer_path, &pair.timer)
}

/// Removes both files; one already gone is not an error.
pub fn remove_pair(dir: &Path, name: &str) -> io::Result<()> {
    let (timer_path, service_path) = paths(dir, name);
    for path in [timer_path, service_path] {
        match std::fs::remove_file(&path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(())
}

/// `~/.local/share/tempo/scripts`: where Tempo keeps a copy of a file for a
/// timer, when the file lives somewhere that may disappear.
pub fn scripts_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|dir| !dir.is_empty())
                .map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(data.join("tempo/scripts"))
}

/// Copies `source` into `scripts/<name>/`, with its permissions, and gives the
/// new path. The folder is the owner's alone.
pub fn keep_copy(source: &Path, scripts: &Path, name: &str) -> io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = scripts.join(name);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let destination = dir.join(file_name);
    std::fs::copy(source, &destination)?;
    Ok(destination)
}

/// A kept file, remembered to be written back after an undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptFile {
    pub name: std::ffi::OsString,
    pub bytes: Vec<u8>,
    pub mode: u32,
}

/// Most that is remembered of a kept copy: bigger is left where it is.
const MAX_KEPT_BYTES: u64 = 1024 * 1024;

/// What is in `scripts/<name>/`, when it is small enough to bring back.
pub fn read_kept(scripts: &Path, name: &str) -> Option<Vec<KeptFile>> {
    let dir = scripts.join(name);
    let mut total = 0;
    let mut kept = Vec::new();
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let meta = entry.metadata().ok()?;
        if !meta.is_file() {
            return None;
        }
        total += meta.len();
        if total > MAX_KEPT_BYTES || kept.len() >= 16 {
            return None;
        }
        kept.push(KeptFile {
            name: entry.file_name(),
            bytes: std::fs::read(entry.path()).ok()?,
            mode: meta.permissions().mode(),
        });
    }
    Some(kept)
}

pub fn remove_kept(scripts: &Path, name: &str) -> io::Result<()> {
    match std::fs::remove_dir_all(scripts.join(name)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

pub fn write_kept(scripts: &Path, name: &str, files: &[KeptFile]) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = scripts.join(name);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    for file in files {
        let path = dir.join(&file.name);
        std::fs::write(&path, &file.bytes)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(file.mode))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempo_core::schedule::Schedule;
    use tempo_core::spec::{Command, TimerSpec};

    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp"
        ))
        .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn spec() -> TimerSpec {
        TimerSpec {
            name: "files-test".to_owned(),
            description: "Files test".to_owned(),
            command: Command {
                program: PathBuf::from("/usr/bin/true"),
                args: Vec::new(),
                working_dir: None,
            },
            schedule: Schedule::Every { minutes: 60 },
            catch_up: false,
            random_delay_secs: 0,
            conditions: Vec::new(),
            low_priority: false,
            timeout_secs: None,
        }
    }

    fn pair() -> Pair {
        Pair {
            timer: unit::render_timer(&spec()).expect("renders"),
            service: unit::render_service(&spec()).expect("renders"),
        }
    }

    #[test]
    fn written_files_are_read_back_as_tempos() {
        let dir = scratch("roundtrip");
        write_pair(&dir, "files-test", &pair()).expect("written");
        let (read, files) = managed(&dir, "files-test").expect("recognised");
        assert_eq!(read, spec());
        assert_eq!(files, pair());
        let mode = std::fs::metadata(dir.join("files-test.timer"))
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o644);
        assert!(!dir.join("files-test.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_file_tempo_did_not_write_is_refused() {
        let dir = scratch("foreign");
        let foreign = Pair {
            timer: "[Timer]\nOnCalendar=daily\n".to_owned(),
            service: "[Service]\nExecStart=/bin/true\n".to_owned(),
        };
        write_pair(&dir, "mine", &foreign).expect("written");
        let err = managed(&dir, "mine").expect_err("refused");
        assert!(err.contains("did not write"), "{err}");
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_missing_file_is_reported() {
        let dir = scratch("missing");
        std::fs::create_dir_all(&dir).expect("created");
        assert!(managed(&dir, "nothing").is_err());
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn names_cannot_leave_the_folder() {
        let dir = scratch("escape");
        for name in ["", "../x", "a/b", ".hidden"] {
            assert!(managed(&dir, name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn removing_twice_is_fine() {
        let dir = scratch("remove");
        write_pair(&dir, "files-test", &pair()).expect("written");
        remove_pair(&dir, "files-test").expect("removed");
        remove_pair(&dir, "files-test").expect("nothing left is not an error");
        assert!(!dir.join("files-test.timer").exists());
        assert!(!dir.join("files-test.service").exists());
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_kept_copy_keeps_its_mode_and_comes_back() {
        let dir = scratch("kept");
        let source = dir.join("src/job.sh");
        std::fs::create_dir_all(source.parent().expect("parent")).expect("created");
        std::fs::write(&source, b"#!/bin/sh\necho hi\n").expect("written");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o750)).expect("mode");
        let scripts = dir.join("scripts");
        let kept = keep_copy(&source, &scripts, "my-timer").expect("copied");
        assert_eq!(kept, scripts.join("my-timer/job.sh"));
        let mode = std::fs::metadata(&kept)
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o750);
        let dir_mode = std::fs::metadata(scripts.join("my-timer"))
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700);

        let remembered = read_kept(&scripts, "my-timer").expect("small enough");
        remove_kept(&scripts, "my-timer").expect("removed");
        assert!(!kept.exists());
        remove_kept(&scripts, "my-timer").expect("twice is fine");
        write_kept(&scripts, "my-timer", &remembered).expect("written back");
        assert_eq!(std::fs::read(&kept).expect("back"), b"#!/bin/sh\necho hi\n");
        assert_eq!(
            std::fs::metadata(&kept)
                .expect("exists")
                .permissions()
                .mode()
                & 0o777,
            0o750
        );
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_big_kept_copy_is_not_remembered() {
        let dir = scratch("kept-big");
        let scripts = dir.join("scripts");
        std::fs::create_dir_all(scripts.join("big")).expect("created");
        std::fs::write(
            scripts.join("big/data"),
            vec![0_u8; (MAX_KEPT_BYTES + 1) as usize],
        )
        .expect("written");
        assert_eq!(read_kept(&scripts, "big"), None);
        assert_eq!(read_kept(&scripts, "missing"), None);
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }
}
