// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Reads a file the owner chose, only to look at it: its permissions and its
//! first bytes. Nothing is run, written or followed beyond a symbolic link.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tempo_core::analysis::{self, Analysis, Env, FileFacts, HEAD_BYTES};

/// Where a program name is found: the folders of `PATH`, then `~/.local/bin`.
pub fn find_program(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|path| is_executable_file(path))
}

fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// What there is to know of `path` without running it.
pub fn facts(path: &Path) -> Result<FileFacts, String> {
    let path = std::path::absolute(path).map_err(|err| err.to_string())?;
    let meta = std::fs::metadata(&path)
        .map_err(|err| format!("cannot look at {}: {err}", path.display()))?;
    let real_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if meta.is_dir() {
        return Ok(FileFacts {
            path,
            real_path,
            is_dir: true,
            size: 0,
            mode: meta.permissions().mode(),
            head: Vec::new(),
        });
    }
    // A pipe or a device would wait for data that never comes.
    if !meta.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let mut head = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(HEAD_BYTES as u64).read_to_end(&mut head))
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    Ok(FileFacts {
        path,
        real_path,
        is_dir: false,
        size: meta.len(),
        mode: meta.permissions().mode(),
        head,
    })
}

/// Looks at the file with the real system around it.
pub fn analyze_path(path: &Path) -> Result<Analysis, String> {
    let facts = facts(path)?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let exists = |path: &Path| path.exists();
    let env = Env {
        home: &home,
        arch: std::env::consts::ARCH,
        exists: &exists,
        find: &find_program,
    };
    Ok(analysis::analyze(&facts, &env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempo_core::analysis::{Id, Kind, Level};

    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp"
        ))
        .join(format!("probe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("created");
        dir
    }

    fn write(path: &Path, bytes: &[u8], mode: u32) {
        std::fs::File::create(path)
            .and_then(|mut file| file.write_all(bytes))
            .expect("written");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("mode");
    }

    #[test]
    fn a_real_script_is_looked_at_and_not_run() {
        let dir = scratch("script");
        let marker = dir.join("ran");
        let script = dir.join("job.sh");
        let body = format!("#!/bin/sh\ntouch '{}'\n", marker.display());
        write(&script, body.as_bytes(), 0o755);
        let analysis = analyze_path(&script).expect("analysed");
        assert!(matches!(analysis.kind, Kind::Script { shebang: true, .. }));
        assert!(!marker.exists(), "the file must never be run");
        assert_eq!(analysis.suggestion.name, "job");
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn only_the_start_of_a_big_file_is_read() {
        let dir = scratch("big");
        let file = dir.join("big.sh");
        let mut bytes = b"#!/bin/sh\n".to_vec();
        bytes.resize(HEAD_BYTES * 3, b'#');
        write(&file, &bytes, 0o644);
        let facts = facts(&file).expect("read");
        assert_eq!(facts.head.len(), HEAD_BYTES);
        assert_eq!(facts.size, (HEAD_BYTES * 3) as u64);
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_folder_is_reported_not_read() {
        let dir = scratch("folder");
        let analysis = analyze_path(&dir).expect("analysed");
        assert_eq!(analysis.kind, Kind::Directory);
        assert_eq!(analysis.worst(), Level::Problem);
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn a_link_is_followed_for_its_place_but_started_by_its_own_path() {
        let dir = scratch("link");
        let target = dir.join("real.sh");
        write(&target, b"#!/bin/sh\necho hi\n", 0o755);
        let link = dir.join("link.sh");
        std::os::unix::fs::symlink(&target, &link).expect("link");
        let facts = facts(&link).expect("read");
        assert_eq!(facts.path, link);
        assert_eq!(
            facts.real_path,
            std::fs::canonicalize(&target).expect("real")
        );
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn missing_files_and_special_files_are_refused() {
        let dir = scratch("refuse");
        assert!(facts(&dir.join("nothing")).is_err());
        assert!(facts(Path::new("/dev/null")).is_err());
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    #[test]
    fn programs_are_found_in_the_path() {
        assert!(find_program("sh").is_some());
        assert_eq!(find_program("../sh"), None);
        assert_eq!(find_program(""), None);
        assert_eq!(find_program("definitely-not-a-program-tempo"), None);
    }

    #[test]
    fn the_real_system_judges_a_missing_interpreter() {
        let dir = scratch("interp");
        let file = dir.join("job");
        write(
            &file,
            b"#!/usr/bin/env definitely-not-a-program-tempo\nx\n",
            0o755,
        );
        let analysis = analyze_path(&file).expect("analysed");
        assert!(
            analysis
                .checks
                .iter()
                .any(|c| c.id == Id::Interpreter && c.level == Level::Problem)
        );
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }
}
