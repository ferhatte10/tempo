// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The page of one timer.

use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;

use crate::view::{Action, Health, Origin, RunResult, State, TimerView, relative};
use crate::widgets::{chip, command_block, dot, strip};
use crate::words::{self, fill};

fn service_of(timer: &TimerView) -> String {
    timer.unit.replace(".timer", ".service")
}

fn heading(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["heading"])
        .build()
}

fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build()
}

/// A command with a title above it.
fn step(title: &str, command: &str) -> gtk::Box {
    let step = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    step.append(&heading(title));
    step.append(&command_block(command));
    step
}

fn property(title: &str, value: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(value)
        .use_markup(false)
        .subtitle_selectable(true)
        .css_classes(["property"])
        .build();
    row.set_subtitle_lines(0);
    row
}

fn status_sentence(timer: &TimerView) -> String {
    match timer.state {
        State::Running => gettext("Running now"),
        State::Paused if timer.enabled => gettext("Paused: it starts again when you log in"),
        State::Paused => gettext("Turned off: it does not run until you turn it on"),
        State::Waiting => match timer.last_run() {
            Some(run) => fill(
                &gettext("Last run: {result}, {when}"),
                &[
                    ("result", &words::result(run.result).to_lowercase()),
                    ("when", &words::when(relative(-run.ago_secs))),
                ],
            ),
            None => gettext("Has not run yet"),
        },
    }
}

fn header(timer: &TimerView) -> gtk::Box {
    let title = gtk::Label::builder()
        .label(timer.title.as_str())
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["title-1"])
        .build();
    let top = gtk::Box::builder().spacing(12).build();
    top.append(&dot(timer.health(), true));
    top.append(&title);
    top.append(&chip(timer.origin));

    let status = gtk::Label::builder()
        .label(status_sentence(timer))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let head = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    head.append(&top);
    head.append(&status);
    head
}

/// Shown when the last run failed: what happened and what to try.
fn failure(timer: &TimerView) -> Option<gtk::Box> {
    if timer.health() != Health::Failed {
        return None;
    }
    let run = timer
        .last_run()
        .filter(|run| run.result == RunResult::Failed)?;
    let service = service_of(timer);

    let title = gtk::Label::builder()
        .label(gettext("The last run failed"))
        .xalign(0.0)
        .css_classes(["heading", "tempo-text-bad"])
        .build();
    let mut facts = vec![fill(
        &gettext("It stopped after {n} s."),
        &[("n", &run.duration_secs.to_string())],
    )];
    if let Some(code) = run.exit_code {
        facts.push(fill(
            &gettext("Exit code {n}."),
            &[("n", &code.to_string())],
        ));
    }
    let callout = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(["tempo-callout", "tempo-failed"])
        .build();
    callout.append(&title);
    callout.append(&note(&facts.join(" ")));
    callout.append(&step(
        &gettext("See what it said"),
        &format!("journalctl --user -u {service} -n 50 --no-pager"),
    ));
    callout.append(&step(
        &gettext("Clear the failed mark once it is fixed"),
        &format!("systemctl --user reset-failed {service}"),
    ));
    Some(callout)
}

fn details(timer: &TimerView) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Details"))
        .build();
    if !timer.description.is_empty() && timer.description != timer.title {
        group.add(&property(&gettext("About"), &timer.description));
    }
    group.add(&property(
        &gettext("Schedule"),
        &words::schedule(&timer.schedule),
    ));
    if let Some(secs) = timer.next_in_secs.filter(|_| timer.state != State::Paused) {
        group.add(&property(
            &gettext("Next run"),
            &words::when(relative(secs)),
        ));
    }

    let (ok, bad, skipped) = timer.counts();
    let history = adw::ActionRow::builder()
        .title(gettext("Last runs"))
        .subtitle(fill(
            &gettext("{ok} succeeded, {bad} failed, {skipped} skipped"),
            &[
                ("ok", &ok.to_string()),
                ("bad", &bad.to_string()),
                ("skipped", &skipped.to_string()),
            ],
        ))
        .css_classes(["property"])
        .build();
    history.add_suffix(&strip(timer, true));
    group.add(&history);

    group.add(&property(&gettext("Runs"), &timer.command));
    group.add(&property(&gettext("Timer file"), &timer.file));
    group
}

/// What a button does: the unit, the action, and the button itself (to show
/// that it is busy).
pub type Handler = Rc<dyn Fn(&str, Action, &gtk::Button)>;

/// What a timer page can call upon: the actions (once the link to systemd is
/// up) and the logs.
pub struct Context {
    pub handler: Option<Handler>,
    pub open_logs: Rc<dyn Fn(&str)>,
    /// Opens the form to change a timer of Tempo's.
    pub edit: Rc<dyn Fn(&str)>,
}

/// The way into the logs of the timer.
fn logs_group(timer: &TimerView, context: &Context) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().build();
    let row = adw::ActionRow::builder()
        .title(gettext("View Logs"))
        .subtitle(gettext("The lines of the timer and of its service, live"))
        .activatable(true)
        .build();
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let (open, unit) = (context.open_logs.clone(), timer.unit.clone());
    row.connect_activated(move |_| open(&unit));
    group.add(&row);
    group
}

/// The buttons of a timer Tempo wrote. Without a handler (the preview) they
/// are drawn and disabled.
fn actions(timer: &TimerView, context: &Context) -> adw::PreferencesGroup {
    let handler = context.handler.as_ref();
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Actions"))
        .build();
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(5)
        .column_spacing(8)
        .row_spacing(8)
        .homogeneous(false)
        .build();
    let paused = timer.state == State::Paused;
    let items = [
        (gettext("Run Now"), Some(Action::RunNow), false),
        if paused {
            (gettext("Resume"), Some(Action::Resume), false)
        } else {
            (gettext("Pause"), Some(Action::Pause), false)
        },
        if timer.enabled {
            (gettext("Turn Off"), Some(Action::TurnOff), false)
        } else {
            (gettext("Turn On"), Some(Action::TurnOn), false)
        },
        (gettext("Edit…"), Some(Action::Edit), false),
        (gettext("Delete"), Some(Action::Delete), true),
    ];
    for (label, action, destructive) in items {
        let button = gtk::Button::builder().label(label).build();
        match (action, handler) {
            (Some(Action::Edit), Some(_)) => {
                let (edit, unit) = (context.edit.clone(), timer.unit.clone());
                button.connect_clicked(move |_| edit(&unit));
            }
            (Some(action), Some(handler)) => {
                button.set_sensitive(!(action == Action::RunNow && timer.state == State::Running));
                let (handler, unit) = (handler.clone(), timer.unit.clone());
                button.connect_clicked(move |button| handler(&unit, action, button));
            }
            (Some(_), None) => {
                button.set_sensitive(false);
                button.set_tooltip_text(Some(&gettext("Not available in the preview")));
            }
            (None, _) => button.set_sensitive(false),
        }
        if destructive {
            button.add_css_class("destructive-action");
        }
        flow.append(&button);
    }
    group.add(&flow);
    group
}

/// For a timer that is not Tempo's: what it does, and the commands to use.
fn system_help(timer: &TimerView) -> gtk::Box {
    let help = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .build();
    help.append(
        &gtk::Label::builder()
            .label(gettext("To stop or remove it"))
            .xalign(0.0)
            .css_classes(["title-4"])
            .build(),
    );
    let why = if timer.origin == Origin::Personal {
        gettext(
            "You wrote this timer by hand, so Tempo does not manage it. To act on it, run a command in a terminal.",
        )
    } else {
        gettext(
            "Tempo never changes a timer it did not create. To act on this one, run a command in a terminal.",
        )
    };
    help.append(&note(&why));
    help.append(&step(
        &gettext("Stop it until you log in again"),
        &format!("systemctl --user stop {}", timer.unit),
    ));
    help.append(&step(
        &gettext("Turn it off for good"),
        &format!("systemctl --user disable --now {}", timer.unit),
    ));
    help.append(&step(
        &gettext("Prevent it from starting at all"),
        &format!("systemctl --user mask --now {}", timer.unit),
    ));
    help.append(&note(&gettext(
        "To remove it for every user, uninstall the package that installed it.",
    )));
    help
}

pub fn page(timer: &TimerView, context: &Context) -> adw::NavigationPage {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(16)
        .margin_end(16)
        .build();
    content.append(&header(timer));
    if let Some(callout) = failure(timer) {
        content.append(&callout);
    }
    content.append(&details(timer));
    content.append(&logs_group(timer, context));
    match timer.origin {
        Origin::Yours => content.append(&actions(timer, context)),
        Origin::Personal | Origin::System => content.append(&system_help(timer)),
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(
            &adw::Clamp::builder()
                .maximum_size(760)
                .tightening_threshold(560)
                .child(&content)
                .build(),
        )
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&scrolled));
    adw::NavigationPage::builder()
        .title(timer.title.as_str())
        .tag(timer.unit.as_str())
        .child(&toolbar)
        .build()
}
