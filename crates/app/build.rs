// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Compiles the translations listed in `po/LINGUAS` into `.mo` files, in
//! `$OUT_DIR/locale` for development, or in `LOCALEDIR` when packaging. Without
//! `msgfmt` the build still works and the app stays in English.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const DOMAIN: &str = "tempo";
const PO_DIR: &str = "../../po";

fn main() {
    println!("cargo:rerun-if-changed={PO_DIR}");
    println!("cargo:rerun-if-env-changed=LOCALEDIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo always sets OUT_DIR"));
    let dev_dir = out_dir.join("locale");
    let localedir = env::var("LOCALEDIR").unwrap_or_else(|_| dev_dir.display().to_string());
    println!("cargo:rustc-env=TEMPO_LOCALEDIR={localedir}");

    let linguas = fs::read_to_string(format!("{PO_DIR}/LINGUAS")).unwrap_or_default();
    for lang in linguas
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        compile(lang, &dev_dir);
    }
}

fn compile(lang: &str, dev_dir: &Path) {
    let target = dev_dir.join(lang).join("LC_MESSAGES");
    if let Err(err) = fs::create_dir_all(&target) {
        println!("cargo:warning=cannot create {}: {err}", target.display());
        return;
    }
    let output = target.join(format!("{DOMAIN}.mo"));
    match Command::new("msgfmt")
        .args(["--check", "-o"])
        .arg(&output)
        .arg(format!("{PO_DIR}/{lang}.po"))
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => println!("cargo:warning=msgfmt failed for {lang}.po ({status})"),
        Err(err) => println!("cargo:warning=msgfmt not found ({err}): the app stays in English"),
    }
}
