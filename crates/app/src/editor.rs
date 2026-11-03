// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The form of a timer: to create one from what the analysis of a file found,
//! or to change one that Tempo wrote.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::{gio, glib};
use tempo_core::analysis;
use tempo_core::draft::{Draft, DraftError, Field, ScheduleKind};
use tempo_core::schedule::{MAX_EVERY_MINUTES, Schedule, Timing, Weekday};
use tempo_core::spec::{Command, TimerSpec};

use crate::words::{self, fill};

/// How long after the last change the next runs are asked for again.
const PREVIEW_DELAY: Duration = Duration::from_millis(350);
/// Next runs shown.
const PREVIEW_RUNS: u32 = 5;

/// Create or change the timer: the timer, the file to keep a copy of, whether
/// it is a change, and a call that says the work is over.
pub type SaveHandler = Rc<dyn Fn(TimerSpec, Option<PathBuf>, bool, Rc<dyn Fn()>)>;

/// What the form asks of the window.
pub struct Callbacks {
    pub save: SaveHandler,
    /// Run a command once; the window asks the owner first.
    pub try_now: Rc<dyn Fn(Command)>,
}

pub struct Options {
    /// A timer of Tempo's being changed, rather than a new one.
    pub editing: bool,
    /// The file the timer is made from, when Tempo may keep a copy of it.
    pub keep_copy_of: Option<PathBuf>,
    /// The files were changed outside Tempo: saving replaces those changes.
    pub changed_outside: bool,
    /// The link to systemd is up: without it the form can only be looked at.
    pub live: bool,
}

/// A reason from the core, in the language of the owner when it is a common one.
fn reason(error: &DraftError) -> String {
    let said = match error.reason.as_str() {
        "enter the command to run" => gettext("Enter the command to run."),
        "it must be an absolute path" => {
            gettext("Start with the full path of the program, such as /usr/bin/python3.")
        }
        "it is empty" => gettext("This cannot be empty."),
        "it is too long" => gettext("This is too long."),
        "a ' quote is never closed" | "a \" quote is never closed" => {
            gettext("A quote is never closed.")
        }
        "the line ends with a backslash" => gettext("The line ends with a backslash."),
        "it must not be empty or start or end with a space" => {
            gettext("Give it a title, without a space at the start or the end.")
        }
        "it must have 1 to 64 characters" => gettext("The name must have 1 to 64 characters."),
        "the system cuts process names at 15 characters" => {
            gettext("The system cuts process names at 15 characters.")
        }
        other => other.to_owned(),
    };
    let field = match error.field {
        Field::Name => gettext("Name"),
        Field::Title => gettext("Title"),
        Field::Command => gettext("Command"),
        Field::WorkingDir => gettext("Folder"),
        Field::Schedule => gettext("Schedule"),
        Field::Spread => gettext("Spread"),
        Field::Timeout => gettext("Time limit"),
        Field::NotRunning => gettext("Program"),
        Field::RequirePath => gettext("File"),
    };
    format!("{field}: {said}")
}

fn kind_labels() -> Vec<String> {
    vec![
        gettext("Every few minutes"),
        pgettext("schedule kind", "Every hour"),
        pgettext("schedule kind", "Every day"),
        gettext("On some days of the week"),
        gettext("Every month"),
        gettext("Custom (calendar expression)"),
    ]
}

fn spin(title: &str, subtitle: &str, min: u32, max: u32, value: u32) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(f64::from(min), f64::from(max), 1.0);
    row.set_title(title);
    if !subtitle.is_empty() {
        row.set_subtitle(subtitle);
    }
    row.set_value(f64::from(value));
    row
}

fn entry(title: &str, text: &str) -> adw::EntryRow {
    adw::EntryRow::builder().title(title).text(text).build()
}

fn switch(title: &str, subtitle: &str, active: bool) -> adw::SwitchRow {
    adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(active)
        .build()
}

fn as_u32(row: &adw::SpinRow) -> u32 {
    row.value().round().max(0.0) as u32
}

/// The lines of `systemd-analyze calendar` that give the next times.
fn next_times(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Next elapse:") || line.starts_with("Iter. #"))
        .filter_map(|line| line.split_once(':').map(|(_, rest)| rest))
        .map(|rest| {
            rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':')
                .trim()
        })
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .collect()
}

/// What a calendar says, asked of systemd itself: the next times, or its
/// complaint when the expression is not understood.
async fn ask_calendar(expression: &str) -> Result<Vec<String>, String> {
    let process = crate::journal::spawn_program(
        "systemd-analyze",
        &[
            "calendar".to_owned(),
            format!("--iterations={PREVIEW_RUNS}"),
            expression.to_owned(),
        ],
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    )?;
    let (output, _) = process
        .communicate_utf8_future(None::<String>)
        .await
        .map_err(|err| err.to_string())?;
    let output = output.map(String::from).unwrap_or_default();
    if process.exit_status() == 0 {
        Ok(next_times(&output))
    } else {
        Err(output.lines().next().unwrap_or_default().to_owned())
    }
}

/// Everything that changes with the kind of schedule.
struct ScheduleRows {
    every: adw::SpinRow,
    at_minute: adw::SpinRow,
    hour: adw::SpinRow,
    minute: adw::SpinRow,
    days: gtk::ListBoxRow,
    day_buttons: Vec<(Weekday, gtk::ToggleButton)>,
    month_day: adw::SpinRow,
    expression: adw::EntryRow,
    catch_up: adw::SwitchRow,
}

impl ScheduleRows {
    fn show(&self, kind: ScheduleKind) {
        use ScheduleKind::*;
        self.every.set_visible(kind == Every);
        self.at_minute.set_visible(kind == Hourly);
        self.hour
            .set_visible(matches!(kind, Daily | Weekly | Monthly));
        self.minute
            .set_visible(matches!(kind, Daily | Weekly | Monthly));
        self.days.set_visible(kind == Weekly);
        self.month_day.set_visible(kind == Monthly);
        self.expression.set_visible(kind == Custom);
        self.catch_up.set_visible(kind != Every);
    }
}

pub fn page(draft: Draft, options: Options, callbacks: Callbacks) -> adw::NavigationPage {
    let draft = Rc::new(RefCell::new(draft));
    let keep_copy = Rc::new(Cell::new(options.keep_copy_of.is_some()));
    let calendar_ok = Rc::new(Cell::new(true));
    let generation = Rc::new(Cell::new(0_u32));
    let name_touched = Rc::new(Cell::new(options.editing));
    let start = draft.borrow().clone();

    // ---- About ---------------------------------------------------------
    let title = entry(&gettext("Title"), &start.title);
    let name = entry(&gettext("Name"), &start.name);
    name.set_sensitive(!options.editing);
    let about = adw::PreferencesGroup::builder()
        .title(gettext("About"))
        .description(if options.editing {
            gettext("The name identifies the timer to systemd: it cannot change.")
        } else {
            gettext(
                "The name identifies the timer to systemd: lowercase letters, digits and dashes.",
            )
        })
        .build();
    about.add(&title);
    about.add(&name);

    // ---- What to run ---------------------------------------------------
    let command = entry(&gettext("Command"), &start.command);
    let folder = entry(
        &gettext("Starts from the folder (optional)"),
        &start.working_dir,
    );
    let keep = switch(
        &gettext("Keep a copy of the file"),
        &options
            .keep_copy_of
            .as_ref()
            .map(|path| {
                fill(
                    &gettext("{path} may disappear: Tempo keeps its own copy and runs that one."),
                    &[("path", &path.display().to_string())],
                )
            })
            .unwrap_or_default(),
        keep_copy.get(),
    );
    keep.set_visible(options.keep_copy_of.is_some());
    let try_row = adw::ActionRow::builder()
        .title(gettext("Try it once, now"))
        .subtitle(gettext(
            "Runs the command for real, as the timer would, and shows what it says.",
        ))
        .build();
    let try_button = gtk::Button::builder()
        .label(gettext("_Try It…"))
        .use_underline(true)
        .valign(gtk::Align::Center)
        .sensitive(options.live)
        .build();
    try_row.add_suffix(&try_button);
    let run = adw::PreferencesGroup::builder()
        .title(gettext("What to run"))
        .build();
    run.add(&command);
    run.add(&folder);
    run.add(&keep);
    run.add(&try_row);

    // ---- When ----------------------------------------------------------
    let kind = adw::ComboRow::builder()
        .title(gettext("Repeat"))
        .model(&gtk::StringList::new(
            &kind_labels().iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .selected(
            ScheduleKind::ALL
                .iter()
                .position(|k| *k == start.kind)
                .unwrap_or(0) as u32,
        )
        .build();
    let every = spin(
        &gettext("Minutes between two runs"),
        &gettext("Counted from the end of the last run."),
        1,
        MAX_EVERY_MINUTES,
        start.every_minutes,
    );
    let at_minute = spin(&gettext("At minute"), "", 0, 59, u32::from(start.minute));
    let hour = spin(&gettext("Hour"), "", 0, 23, u32::from(start.hour));
    let minute = spin(&gettext("Minute"), "", 0, 59, u32::from(start.minute));
    let month_day = spin(
        &gettext("Day of the month"),
        &gettext("From 1 to 28, so that no month skips it."),
        1,
        28,
        u32::from(start.month_day),
    );
    let expression = entry(&gettext("Calendar expression"), &start.expression);
    let catch_up = switch(
        &gettext("Run once after a missed time"),
        &gettext("If the computer was off or you were logged out at the time."),
        start.catch_up,
    );

    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(4)
        .max_children_per_line(7)
        .column_spacing(6)
        .row_spacing(6)
        .homogeneous(true)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    let mut day_buttons = Vec::new();
    for day in Weekday::ALL {
        let button = gtk::ToggleButton::builder()
            .label(words::weekday(day))
            .active(start.days.contains(&day))
            .build();
        flow.append(&button);
        day_buttons.push((day, button));
    }
    let days_title = gtk::Label::builder()
        .label(gettext("Days"))
        .xalign(0.0)
        .margin_top(10)
        .margin_start(12)
        .css_classes(["caption", "dim-label"])
        .build();
    let days_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    days_box.append(&days_title);
    days_box.append(&flow);
    let days = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&days_box)
        .build();

    let rows = Rc::new(ScheduleRows {
        every,
        at_minute,
        hour,
        minute,
        days,
        day_buttons,
        month_day,
        expression,
        catch_up,
    });
    rows.show(start.kind);

    let when = adw::PreferencesGroup::builder()
        .title(gettext("When"))
        .build();
    when.add(&kind);
    when.add(&rows.every);
    when.add(&rows.at_minute);
    when.add(&rows.hour);
    when.add(&rows.minute);
    when.add(&rows.days);
    when.add(&rows.month_day);
    when.add(&rows.expression);
    when.add(&rows.catch_up);
    let preview = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(false)
        .margin_top(6)
        .css_classes(["dim-label"])
        .build();
    let when_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();
    when_box.append(&when);
    when_box.append(&preview);

    // ---- Only if / options -----------------------------------------------
    let not_running = entry(&gettext("This program is not running"), &start.not_running);
    let ac_power = switch(
        &gettext("The computer is on mains power"),
        "",
        start.ac_power,
    );
    let require_path = entry(&gettext("This file exists"), &start.require_path);
    let only_if = adw::PreferencesGroup::builder()
        .title(gettext("Only if"))
        .description(gettext(
            "When one of these is not true, the run is skipped, not failed.",
        ))
        .build();
    only_if.add(&not_running);
    only_if.add(&ac_power);
    only_if.add(&require_path);

    let spread = spin(
        &gettext("Spread the start (minutes)"),
        &gettext("Starts at a random moment within this time, to avoid a rush."),
        0,
        1_440,
        start.spread_minutes,
    );
    let low_priority = switch(
        &gettext("Low priority"),
        &gettext("Does not slow the computer down while it works."),
        start.low_priority,
    );
    let limit = switch(
        &gettext("Stop it if it takes too long"),
        "",
        start.timeout_minutes.is_some(),
    );
    let limit_minutes = spin(
        &gettext("Time limit (minutes)"),
        "",
        1,
        10_080,
        start.timeout_minutes.unwrap_or(30),
    );
    limit_minutes.set_visible(start.timeout_minutes.is_some());
    let options_group = adw::PreferencesGroup::builder()
        .title(gettext("Options"))
        .build();
    options_group.add(&spread);
    options_group.add(&low_priority);
    options_group.add(&limit);
    options_group.add(&limit_minutes);

    // ---- The page --------------------------------------------------------
    let banner = adw::Banner::new("");
    let save = gtk::Button::builder()
        .label(if options.editing {
            gettext("_Save")
        } else {
            gettext("_Create")
        })
        .use_underline(true)
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();
    let header = adw::HeaderBar::new();
    header.pack_end(&save);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(16)
        .margin_end(16)
        .build();
    for widget in [
        about.upcast_ref::<gtk::Widget>(),
        run.upcast_ref(),
        when_box.upcast_ref(),
        only_if.upcast_ref(),
        options_group.upcast_ref(),
    ] {
        content.append(widget);
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
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&banner);
    toolbar.set_content(Some(&scrolled));

    if options.changed_outside {
        banner.set_title(&gettext(
            "These files were changed outside Tempo. Saving replaces those changes.",
        ));
        banner.set_revealed(true);
    }

    // ---- Keeping the draft and the page in step --------------------------
    // Reads every widget into the draft, shows what is wrong, and asks for the
    // next runs again.
    let refresh: Rc<dyn Fn()> = {
        let (draft, rows, save, banner, preview) = (
            draft.clone(),
            rows.clone(),
            save.clone(),
            banner.clone(),
            preview.clone(),
        );
        let (title, name, command, folder, kind) = (
            title.clone(),
            name.clone(),
            command.clone(),
            folder.clone(),
            kind.clone(),
        );
        let (not_running, ac_power, require_path) =
            (not_running.clone(), ac_power.clone(), require_path.clone());
        let (spread, low_priority, limit, limit_minutes) = (
            spread.clone(),
            low_priority.clone(),
            limit.clone(),
            limit_minutes.clone(),
        );
        let (calendar_ok, generation) = (calendar_ok.clone(), generation.clone());
        let editing = options.editing;
        let live = options.live;
        let outside = options.changed_outside;
        Rc::new(move || {
            let mut d = draft.borrow_mut();
            d.title = title.text().to_string();
            d.name = name.text().to_string();
            d.command = command.text().to_string();
            d.working_dir = folder.text().to_string();
            d.kind = ScheduleKind::ALL
                .get(kind.selected() as usize)
                .copied()
                .unwrap_or(ScheduleKind::Daily);
            d.every_minutes = as_u32(&rows.every);
            d.hour = as_u32(&rows.hour) as u8;
            d.minute = if d.kind == ScheduleKind::Hourly {
                as_u32(&rows.at_minute) as u8
            } else {
                as_u32(&rows.minute) as u8
            };
            d.days = rows
                .day_buttons
                .iter()
                .filter(|(_, button)| button.is_active())
                .map(|(day, _)| *day)
                .collect();
            d.month_day = as_u32(&rows.month_day) as u8;
            d.expression = rows.expression.text().to_string();
            d.catch_up = rows.catch_up.is_active();
            d.spread_minutes = as_u32(&spread);
            d.low_priority = low_priority.is_active();
            d.timeout_minutes = limit.is_active().then(|| as_u32(&limit_minutes));
            d.not_running = not_running.text().to_string();
            d.ac_power = ac_power.is_active();
            d.require_path = require_path.text().to_string();
            let result = d.to_spec();
            let schedule = d.schedule();
            let kind = d.kind;
            drop(d);

            rows.show(kind);
            limit_minutes.set_visible(limit.is_active());
            match &result {
                Err(error) => {
                    banner.set_title(&reason(error));
                    banner.set_revealed(true);
                }
                Ok(_) if outside && editing => {}
                Ok(_) => banner.set_revealed(false),
            }
            save.set_sensitive(result.is_ok() && calendar_ok.get() && live);

            // The next runs, a moment after the last change.
            generation.set(generation.get() + 1);
            let (current, preview, save, banner, calendar_ok) = (
                generation.clone(),
                preview.clone(),
                save.clone(),
                banner.clone(),
                calendar_ok.clone(),
            );
            let wanted = generation.get();
            let ok_now = result.is_ok();
            glib::timeout_add_local_once(PREVIEW_DELAY, move || {
                if current.get() != wanted {
                    return;
                }
                glib::spawn_future_local(async move {
                    show_next_runs(&schedule, &preview).await;
                    let valid = !preview.has_css_class("error");
                    calendar_ok.set(valid);
                    save.set_sensitive(ok_now && valid && live);
                    if !valid {
                        banner.set_title(&preview.text());
                        banner.set_revealed(true);
                    }
                });
            });
        })
    };

    // Every widget runs the same refresh.
    let go = |refresh: &Rc<dyn Fn()>| {
        let refresh = refresh.clone();
        move || refresh()
    };
    for entry in [
        &title,
        &name,
        &command,
        &folder,
        &rows.expression,
        &not_running,
        &require_path,
    ] {
        entry.connect_changed({
            let go = go(&refresh);
            move |_| go()
        });
    }
    for spin in [
        &rows.every,
        &rows.at_minute,
        &rows.hour,
        &rows.minute,
        &rows.month_day,
        &spread,
        &limit_minutes,
    ] {
        spin.connect_value_notify({
            let go = go(&refresh);
            move |_| go()
        });
    }
    for switch in [&rows.catch_up, &ac_power, &low_priority, &limit] {
        switch.connect_active_notify({
            let go = go(&refresh);
            move |_| go()
        });
    }
    kind.connect_selected_notify({
        let go = go(&refresh);
        move |_| go()
    });
    for (_, button) in &rows.day_buttons {
        button.connect_toggled({
            let go = go(&refresh);
            move |_| go()
        });
    }
    keep.connect_active_notify({
        let keep_copy = keep_copy.clone();
        let keep = keep.clone();
        move |_| keep_copy.set(keep.is_active())
    });

    // The name follows the title until it is typed by hand.
    name.connect_changed({
        let touched = name_touched.clone();
        let name = name.clone();
        move |_| {
            if name.has_focus() || name.focus_child().is_some() {
                touched.set(true);
            }
        }
    });
    title.connect_changed({
        let (touched, name) = (name_touched.clone(), name.clone());
        move |title| {
            if !touched.get() {
                name.set_text(&analysis::slug(&title.text()));
            }
        }
    });

    try_button.connect_clicked({
        let draft = draft.clone();
        let try_now = callbacks.try_now.clone();
        let banner = banner.clone();
        move |_| match draft.borrow().command() {
            Ok(command) => try_now(command),
            Err(error) => {
                banner.set_title(&reason(&error));
                banner.set_revealed(true);
            }
        }
    });

    save.connect_clicked({
        let (draft, save_cb, keep_source) = (
            draft.clone(),
            callbacks.save.clone(),
            options.keep_copy_of.clone(),
        );
        let (keep_copy, editing) = (keep_copy.clone(), options.editing);
        let label = save.label().unwrap_or_default();
        move |button| {
            let Ok(spec) = draft.borrow().to_spec() else {
                return;
            };
            let copy = keep_source.clone().filter(|_| keep_copy.get());
            // A spinner and a word while it works, then the button again.
            crate::widgets::set_busy(
                button,
                &if editing {
                    gettext("Saving…")
                } else {
                    gettext("Creating…")
                },
            );
            let (button, label) = (button.clone(), label.clone());
            let done: Rc<dyn Fn()> = Rc::new(move || {
                button.set_child(gtk::Widget::NONE);
                button.set_use_underline(true);
                button.set_label(&label);
                button.set_sensitive(true);
            });
            save_cb(spec, copy, editing, done);
        }
    });

    refresh();
    adw::NavigationPage::builder()
        .title(if options.editing {
            gettext("Edit Timer")
        } else {
            gettext("New Timer")
        })
        .tag("editor")
        .child(&toolbar)
        .build()
}

/// Puts into `label` when the next runs are, or what is wrong with the
/// schedule (the label then has the `error` class).
async fn show_next_runs(schedule: &Schedule, label: &gtk::Label) {
    label.remove_css_class("error");
    let timing = match schedule.timing() {
        Ok(timing) => timing,
        Err(error) => {
            label.set_text(&error.to_string());
            label.add_css_class("error");
            return;
        }
    };
    match timing {
        Timing::Monotonic {
            first_secs,
            repeat_secs,
        } => label.set_text(&fill(
            &gettext(
                "First run {first} after you log in, then {repeat} after the end of each run.",
            ),
            &[
                ("first", &words::duration(first_secs)),
                ("repeat", &words::duration(repeat_secs)),
            ],
        )),
        Timing::Calendar(expression) => match ask_calendar(&expression).await {
            Ok(times) if times.is_empty() => label.set_text(""),
            Ok(times) => {
                label.set_text(&format!("{}\n{}", gettext("Next runs:"), times.join("\n")))
            }
            Err(message) => {
                label.set_text(&if message.is_empty() {
                    gettext("systemd does not understand this calendar expression.")
                } else {
                    message
                });
                label.add_css_class("error");
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_times_are_picked_from_systemd_s_answer() {
        let output = "  Original form: daily\nNormalized form: *-*-* 00:00:00\n    Next elapse: Wed 2026-11-04 00:00:00 CET\n       (in UTC): Tue 2026-11-03 23:00:00 UTC\n       From now: 9h left\n       Iter. #2: Thu 2026-11-05 00:00:00 CET\n       (in UTC): Wed 2026-11-04 23:00:00 UTC\n       From now: 1 day 9h left\n";
        assert_eq!(
            next_times(output),
            vec!["Wed 2026-11-04 00:00:00 CET", "Thu 2026-11-05 00:00:00 CET"]
        );
        assert!(next_times("nothing useful").is_empty());
    }

    #[test]
    fn the_common_reasons_have_their_own_words() {
        let error = |field, reason: &str| DraftError {
            field,
            reason: reason.to_owned(),
        };
        assert_eq!(
            reason(&error(Field::Command, "enter the command to run")),
            "Command: Enter the command to run."
        );
        assert_eq!(
            reason(&error(Field::Schedule, "hour 30 is not between 0 and 23")),
            "Schedule: hour 30 is not between 0 and 23"
        );
    }
}
