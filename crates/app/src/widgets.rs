// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The pieces every screen is made of (DESIGN.md, “Components”).

use std::cell::RefCell;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::theme;
use crate::view::{Health, Origin, RunResult, State, TimerView, relative, upcoming};
use crate::words::{self, fill};

/// Runs `change` with the new value whenever the system switches between light
/// and dark, until `widget` is destroyed.
pub fn on_dark_change(widget: &impl IsA<gtk::Widget>, change: impl Fn(bool) + 'static) {
    let style = adw::StyleManager::default();
    let id = style.connect_dark_notify(move |style| change(style.is_dark()));
    let handler = RefCell::new(Some(id));
    let style = style.clone();
    widget.connect_destroy(move |_| {
        if let Some(id) = handler.borrow_mut().take() {
            style.disconnect(id);
        }
    });
}

/// Shows that a button's action is under way: a spinner and a word, and no
/// second click.
pub fn set_busy(button: &gtk::Button, label: &str) {
    let content = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::Center)
        .build();
    content.append(&gtk::Spinner::builder().spinning(true).build());
    content.append(&gtk::Label::new(Some(label)));
    button.set_child(Some(&content));
    button.set_sensitive(false);
}

fn health_class(health: Health) -> &'static str {
    match health {
        Health::Ok => "ok",
        Health::Failed => "bad",
        Health::Running => "run",
        Health::Idle => "idle",
    }
}

/// The state dot. It only decorates: the state is also written next to it.
pub fn dot(health: Health, large: bool) -> gtk::Box {
    let dot = gtk::Box::builder()
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .css_classes(["tempo-dot", health_class(health)])
        .build();
    if large {
        dot.add_css_class("large");
    }
    dot
}

pub fn chip(origin: Origin) -> gtk::Label {
    let (text, class) = match origin {
        Origin::Yours => (gettext("Yours"), "yours"),
        Origin::Personal => (gettext("Personal"), "personal"),
        Origin::System => (gettext("System"), "system"),
    };
    gtk::Label::builder()
        .label(text)
        .valign(gtk::Align::Center)
        .css_classes(["tempo-chip", class])
        .build()
}

fn run_tooltip(run: &crate::view::Run) -> String {
    let mut parts = vec![
        words::result(run.result),
        words::when(relative(-run.ago_secs)),
        fill(
            &gettext("took {n} s"),
            &[("n", &run.duration_secs.to_string())],
        ),
    ];
    if let Some(code) = run.exit_code {
        parts.push(fill(&gettext("exit code {n}"), &[("n", &code.to_string())]));
    }
    parts.join(" · ")
}

/// Ten bars, one per past run, newest on the right.
pub fn strip(timer: &TimerView, large: bool) -> gtk::Box {
    let row = gtk::Box::builder()
        .spacing(if large { 5 } else { 4 })
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Start)
        .focusable(true)
        .css_classes(["tempo-strip"])
        .build();
    if large {
        row.add_css_class("large");
    }
    for slot in timer.strip() {
        let bar = gtk::Box::builder().css_classes(["tempo-bar"]).build();
        match slot {
            Some(run) => {
                bar.add_css_class(match run.result {
                    RunResult::Success => "ok",
                    RunResult::Failed => "bad",
                    RunResult::Skipped | RunResult::Unknown => "skip",
                });
                bar.set_tooltip_text(Some(&run_tooltip(run)));
            }
            None => bar.add_css_class("none"),
        }
        row.append(&bar);
    }
    let (ok, bad, skipped) = timer.counts();
    let summary = fill(
        &gettext("Last runs: {ok} succeeded, {bad} failed, {skipped} skipped"),
        &[
            ("ok", &ok.to_string()),
            ("bad", &bad.to_string()),
            ("skipped", &skipped.to_string()),
        ],
    );
    row.update_property(&[gtk::accessible::Property::Label(&summary)]);
    row
}

/// A command to run in a terminal, with a button that copies it.
pub fn command_block(command: &str) -> gtk::Box {
    let label = gtk::Label::builder()
        .label(command)
        .css_classes(["monospace"])
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        // Selectable by the pointer, but not focused (and so not all selected)
        // when the page opens.
        .focusable(false)
        .hexpand(true)
        .xalign(0.0)
        .build();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(gettext("Copy"))
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    copy.update_property(&[gtk::accessible::Property::Label(&gettext("Copy"))]);
    let text = command.to_owned();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(&text);
        button.set_icon_name("object-select-symbolic");
        let weak = button.downgrade();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if let Some(button) = weak.upgrade() {
                button.set_icon_name("edit-copy-symbolic");
            }
        });
    });
    let block = gtk::Box::builder()
        .spacing(6)
        .css_classes(["card", "tempo-command"])
        .build();
    block.append(&label);
    block.append(&copy);
    block
}

fn next_text(timer: &TimerView) -> String {
    match timer.state {
        State::Running => gettext("Running now"),
        State::Paused => gettext("Paused"),
        State::Waiting => match timer.next_in_secs {
            Some(secs) => fill(
                &gettext("Next run {when}"),
                &[("when", &words::when(relative(secs)))],
            ),
            None => gettext("No next run planned"),
        },
    }
}

fn last_text(timer: &TimerView) -> String {
    match timer.last_run() {
        Some(run) if run.result == RunResult::Unknown => fill(
            &gettext("Ran {when}"),
            &[("when", &words::when(relative(-run.ago_secs)))],
        ),
        Some(run) => fill(
            &gettext("{result} {when}"),
            &[
                ("result", &words::result(run.result)),
                ("when", &words::when(relative(-run.ago_secs))),
            ],
        ),
        None => gettext("Never run"),
    }
}

/// One timer on the home page.
pub fn card(timer: &TimerView) -> gtk::ListBoxRow {
    let health = timer.health();

    let name = gtk::Label::builder()
        .label(timer.title.as_str())
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["heading"])
        .build();
    let top = gtk::Box::builder().spacing(10).build();
    top.append(&dot(health, false));
    top.append(&name);
    top.append(&chip(timer.origin));

    let schedule = gtk::Label::builder()
        .label(fill(
            &gettext("{schedule} · {next}"),
            &[
                ("schedule", &words::schedule(&timer.schedule)),
                ("next", &next_text(timer)),
            ],
        ))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();

    let last = gtk::Label::builder()
        .label(last_text(timer))
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["caption"])
        .build();
    match health {
        Health::Failed => last.add_css_class("tempo-text-bad"),
        _ => last.add_css_class("dim-label"),
    }
    let bottom = gtk::Box::builder().spacing(12).build();
    bottom.append(&strip(timer, false));
    bottom.append(&last);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();
    content.append(&top);
    content.append(&schedule);
    content.append(&bottom);

    let row = gtk::ListBoxRow::builder()
        .activatable(true)
        .child(&content)
        .build();
    row.set_widget_name(&timer.unit);
    row.set_cursor_from_name(Some("pointer"));
    if health == Health::Failed {
        row.add_css_class("tempo-failed");
    }
    row.update_property(&[gtk::accessible::Property::Label(&fill(
        &gettext("{name}, {schedule}, {last}"),
        &[
            ("name", &timer.title),
            ("schedule", &words::schedule(&timer.schedule)),
            ("last", &last_text(timer)),
        ],
    ))]);
    row
}

/// The first thing on the home page: what happens next, in words.
pub fn next_banner(timers: &[TimerView]) -> gtk::Box {
    let next = upcoming(timers);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["title-2"])
        .build();
    let detail = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();

    match next.first() {
        Some(first) => {
            let first_when = words::when(relative(first.next_in_secs.unwrap_or(0)));
            let name = first.title.clone();
            let markup = move |dark: bool| {
                fill(
                    &gettext("Next: {name}, {when}"),
                    &[
                        ("name", &glib::markup_escape_text(&name)),
                        (
                            "when",
                            &format!(
                                "<span foreground=\"{}\">{}</span>",
                                theme::palette(dark).accent.css(),
                                glib::markup_escape_text(&first_when)
                            ),
                        ),
                    ],
                )
            };
            title.set_markup(&markup(adw::StyleManager::default().is_dark()));
            let weak = title.downgrade();
            on_dark_change(&title, move |dark| {
                if let Some(title) = weak.upgrade() {
                    title.set_markup(&markup(dark));
                }
            });
            if let Some(second) = next.get(1) {
                detail.set_label(&fill(
                    &gettext("Then {name}, {when}"),
                    &[
                        ("name", &second.title),
                        (
                            "when",
                            &words::when(relative(second.next_in_secs.unwrap_or(0))),
                        ),
                    ],
                ));
            } else {
                detail.set_visible(false);
            }
        }
        None => {
            title.set_label(&gettext("Nothing is scheduled"));
            detail.set_label(&gettext("Create your first timer to see it here."));
        }
    }

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();
    text.append(&title);
    text.append(&detail);

    let icon = gtk::Image::builder()
        .icon_name("preferences-system-time-symbolic")
        .pixel_size(40)
        .valign(gtk::Align::Center)
        .build();
    let banner = gtk::Box::builder()
        .spacing(16)
        .css_classes(["tempo-banner"])
        .build();
    banner.append(&icon);
    banner.append(&text);
    banner
}
