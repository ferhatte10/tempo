// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Tempo: see and manage your systemd user timers.
//!
//! - [`view`]    what the interface shows about a timer, without widgets;
//! - [`words`]   its sentences, in the language of the system;
//! - [`theme`]   the colours of DESIGN.md and the check that they are readable;
//! - [`systemd`] the owner's systemd over D-Bus, read only;
//! - [`notify`]  the notification of a failed run (`--notify-failed`);
//! - [`snapshot`], [`unitfile`] from what it says to what is shown;
//! - [`demo`]    made-up timers for the preview and the screenshots;
//! - [`widgets`], [`detail`], [`window`] the interface.

mod demo;
mod detail;
mod editor;
mod files;
mod journal;
mod logs;
mod notify;
mod probe;
mod review;
mod settings;
mod snapshot;
mod systemd;
mod theme;
mod unitfile;
mod update;
mod view;
mod widgets;
mod window;
mod words;

use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use gettextrs::LocaleCategory;
use gtk::glib;

pub const APP_ID: &str = "fr.ferhatsn.Tempo";
const GETTEXT_PACKAGE: &str = "tempo";
const LOCALEDIR: &str = env!("TEMPO_LOCALEDIR");
/// Set by [`request_restart`]: start again once the application has quit.
static RESTART: AtomicBool = AtomicBool::new(false);

fn main() -> glib::ExitCode {
    init_i18n();
    // Started by systemd when a run fails (notify.rs): no window.
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, service] = args.as_slice()
        && flag == "--notify-failed"
    {
        return notify::run(service);
    }
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|app| {
        load_css();
        setup_icon();
        window::set_shortcuts(app);
    });
    app.connect_activate(window::present);
    let code = app.run();
    if RESTART.load(Ordering::SeqCst) {
        relaunch();
    }
    code
}

/// Quits, then starts the program again (after an update, the new one).
pub fn request_restart(window: &adw::ApplicationWindow) {
    RESTART.store(true, Ordering::SeqCst);
    if let Some(app) = window.application() {
        app.quit();
    }
}

/// Replaces this process with a fresh one. The old instance has released its
/// D-Bus name by now, so the new one starts as the primary instance.
fn relaunch() {
    match std::env::current_exe() {
        Ok(exe) => {
            // After an update the running program is “deleted”: start the new
            // one installed at the same path.
            let path = exe.to_string_lossy();
            let exe = path.strip_suffix(" (deleted)").unwrap_or(&path);
            let err = std::process::Command::new(exe).exec();
            glib::g_critical!("tempo", "Cannot restart Tempo: {err}");
        }
        Err(err) => glib::g_critical!("tempo", "Cannot find the Tempo program: {err}"),
    }
}

/// English is the source language; the system's language picks a translation.
/// A failure only leaves the interface in English.
#[allow(unsafe_code)]
fn init_i18n() {
    // SAFETY: called first thing in `main`, before GTK starts any thread.
    if unsafe { gettextrs::setlocale(LocaleCategory::LcAll, "") }.is_none() {
        glib::g_warning!("tempo", "Unsupported locale, falling back to English");
    }
    if let Err(err) = gettextrs::bindtextdomain(GETTEXT_PACKAGE, locale_dir()) {
        glib::g_warning!("tempo", "Cannot bind translations: {err}");
    }
    if let Err(err) = gettextrs::bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8") {
        glib::g_warning!("tempo", "Cannot set translation encoding: {err}");
    }
    if let Err(err) = gettextrs::textdomain(GETTEXT_PACKAGE) {
        glib::g_warning!("tempo", "Cannot select translation domain: {err}");
    }
}

/// Where the translations are: `TEMPO_LOCALEDIR`, else `share/locale` beside
/// the program when it holds Tempo's (the release archive, unpacked anywhere
/// by install.sh), else the folder chosen at build time.
fn locale_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("TEMPO_LOCALEDIR") {
        return dir.into();
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("share/locale")));
    beside
        .filter(|dir| has_translations(dir))
        .unwrap_or_else(|| LOCALEDIR.into())
}

fn has_translations(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .path()
                .join("LC_MESSAGES")
                .join(format!("{GETTEXT_PACKAGE}.mo"))
                .is_file()
        })
    })
}

/// The icon of the windows. Beside the source, `cargo run` finds it in `data/icons`.
fn setup_icon() {
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display)
            .add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/icons"));
    }
    gtk::Window::set_default_icon_name(APP_ID);
}

/// Loads the stylesheet, with the colours of the current light or dark style,
/// and again whenever the style changes.
fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        glib::g_warning!("tempo", "No display: custom styles not loaded");
        return;
    };
    let provider = gtk::CssProvider::new();
    let apply = {
        let provider = provider.clone();
        move |dark: bool| {
            let css = format!(
                "{}{}",
                theme::css_colors(dark),
                include_str!("../data/style.css")
            );
            provider.load_from_string(&css);
        }
    };
    let style = adw::StyleManager::default();
    apply(style.is_dark());
    style.connect_dark_notify(move |style| apply(style.is_dark()));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
