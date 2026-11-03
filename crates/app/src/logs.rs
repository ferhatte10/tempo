// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The logs of a timer: its lines and those of its service, live from the
//! journal (`journalctl --follow`, stopped when the page closes), with a filter
//! for problems, Copy and Save.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::journal::{self, Entry, Severity};
use crate::view::TimerView;
use crate::words::fill;

/// Most lines kept in the page.
const MAX_LINES: usize = 3000;
/// How often new lines are put on screen.
const FLUSH_EVERY: Duration = Duration::from_millis(300);
/// Ticks without a line before saying there is nothing.
const EMPTY_AFTER_TICKS: u32 = 3;
/// Distance from the end, in pixels, under which new lines keep it in view.
const STICK_TO_END: f64 = 24.0;

/// Where the lines come from.
pub enum Source {
    /// The real journal, followed.
    Journal,
    /// Made-up lines, for the preview and the screenshots.
    Demo(Vec<Entry>),
}

fn format_time(time_usec: u64) -> String {
    i64::try_from(time_usec / 1_000_000)
        .ok()
        .and_then(|secs| glib::DateTime::from_unix_local(secs).ok())
        .and_then(|time| time.format("%x %X").ok())
        .map(String::from)
        .unwrap_or_default()
}

fn severity_label(severity: Severity) -> Option<String> {
    match severity {
        Severity::Info => None,
        Severity::Warning => Some(gettext("Warning")),
        Severity::Error => Some(gettext("Error")),
    }
}

/// One line as plain text, for Copy and Save.
fn plain_line(entry: &Entry) -> String {
    let mut line = format_time(entry.time_usec);
    line.push_str("  ");
    if let Some(label) = severity_label(entry.severity()) {
        line.push_str(&label);
        line.push_str("  ");
    }
    line.push_str(&entry.message);
    line
}

fn entry_row(entry: &Entry) -> gtk::Box {
    let severity = entry.severity();
    let header = gtk::Box::builder().spacing(8).build();
    header.append(
        &gtk::Label::builder()
            .label(format_time(entry.time_usec))
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["caption", "dim-label"])
            .build(),
    );
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(["tempo-log-entry"])
        .build();
    if let Some(label) = severity_label(severity) {
        let (text_class, row_class) = match severity {
            Severity::Warning => ("tempo-text-warn", "tempo-log-warn"),
            _ => ("tempo-text-bad", "tempo-log-bad"),
        };
        header.append(
            &gtk::Label::builder()
                .label(label)
                .css_classes(["caption-heading", text_class])
                .build(),
        );
        row.add_css_class(row_class);
    }
    row.append(&header);
    row.append(
        &gtk::Label::builder()
            .label(entry.message.as_str())
            .use_markup(false)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .selectable(true)
            .focusable(false)
            .css_classes(["monospace", "tempo-log-message"])
            .build(),
    );
    row
}

/// The list of lines; only those on screen exist as widgets.
struct LogList {
    store: gio::ListStore,
    shown: gtk::FilterListModel,
    scrolled: gtk::ScrolledWindow,
}

impl LogList {
    fn new(problems_only: &gtk::ToggleButton) -> Self {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let button = problems_only.clone();
        let filter = gtk::CustomFilter::new(move |item| {
            !button.is_active()
                || item
                    .downcast_ref::<glib::BoxedAnyObject>()
                    .is_some_and(|item| item.borrow::<Entry>().severity() != Severity::Info)
        });
        problems_only.connect_toggled({
            let filter = filter.clone();
            move |_| filter.changed(gtk::FilterChange::Different)
        });
        let shown = gtk::FilterListModel::new(Some(store.clone()), Some(filter));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let row = item
                .item()
                .and_downcast::<glib::BoxedAnyObject>()
                .map(|entry| entry_row(&entry.borrow::<Entry>()));
            item.set_child(row.as_ref());
        });
        factory.connect_unbind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(gtk::Widget::NONE);
            }
        });
        let list = gtk::ListView::builder()
            .model(&gtk::NoSelection::new(Some(shown.clone())))
            .factory(&factory)
            .show_separators(true)
            .build();
        list.update_property(&[gtk::accessible::Property::Label(&gettext("Logs"))]);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        Self {
            store,
            shown,
            scrolled,
        }
    }

    fn append(&self, entries: &[Entry]) {
        if entries.is_empty() {
            return;
        }
        let adjustment = self.scrolled.vadjustment();
        let at_end =
            adjustment.value() + adjustment.page_size() >= adjustment.upper() - STICK_TO_END;
        let added: Vec<glib::BoxedAnyObject> = entries
            .iter()
            .cloned()
            .map(glib::BoxedAnyObject::new)
            .collect();
        self.store.extend_from_slice(&added);
        let excess = self.store.n_items().saturating_sub(MAX_LINES as u32);
        if excess > 0 {
            self.store.splice(0, excess, &[] as &[glib::BoxedAnyObject]);
        }
        if at_end {
            // Rows are measured as they appear, so the end moves while the
            // list settles: go there again a few times.
            for delay in [60, 200, 450] {
                let adjustment = adjustment.clone();
                glib::timeout_add_local_once(Duration::from_millis(delay), move || {
                    adjustment.set_value(adjustment.upper());
                });
            }
        }
    }

    /// What the list shows, as text. The filter model announces plain objects,
    /// so a typed iterator would panic.
    fn text(&self) -> String {
        let mut text = String::new();
        for item in self.shown.iter::<glib::Object>().flatten() {
            if let Some(item) = item.downcast_ref::<glib::BoxedAnyObject>() {
                text.push_str(&plain_line(&item.borrow::<Entry>()));
                text.push('\n');
            }
        }
        text
    }
}

/// Asks where to save the logs. They name paths and programs: owner only.
async fn save(
    parent: &impl IsA<gtk::Widget>,
    name: &str,
    text: &str,
) -> Result<Option<String>, String> {
    let dialog = gtk::FileDialog::builder()
        .title(gettext("Save Logs"))
        .initial_name(format!("{name}.log"))
        .initial_folder(&gio::File::for_path(glib::home_dir()))
        .modal(true)
        .build();
    let window = parent.as_ref().root().and_downcast::<gtk::Window>();
    let file = match dialog.save_future(window.as_ref()).await {
        Ok(file) => file,
        Err(err) if err.matches(gtk::DialogError::Dismissed) => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };
    let Some(path) = file.path() else {
        return Err(gettext("This location cannot be written to"));
    };
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .and_then(|mut out| out.write_all(text.as_bytes()))
        .map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Reads the lines `journalctl` prints into `pending`, until it stops.
fn follow(timer: &TimerView, pending: &Rc<RefCell<VecDeque<Entry>>>) -> Option<gio::Subprocess> {
    let args = journal::log_args(&timer.unit, &timer.service, true);
    let process = match journal::spawn(
        &args,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    ) {
        Ok(process) => process,
        Err(err) => {
            glib::g_warning!("tempo", "Cannot start journalctl: {err}");
            return None;
        }
    };
    let stdout = process.stdout_pipe()?;
    let stream = gio::DataInputStream::new(&stdout);
    let pending = pending.clone();
    glib::spawn_future_local(async move {
        while let Ok(Some(line)) = stream.read_line_utf8_future(glib::Priority::LOW).await {
            if let Some(entry) = journal::parse_line(&line) {
                pending.borrow_mut().push_back(entry);
            }
        }
    });
    Some(process)
}

pub fn page(timer: &TimerView, source: Source) -> adw::NavigationPage {
    let problems_only = gtk::ToggleButton::builder()
        .icon_name("dialog-warning-symbolic")
        .tooltip_text(gettext("Problems Only"))
        .build();
    problems_only.update_property(&[gtk::accessible::Property::Label(&gettext("Problems Only"))]);
    let log = Rc::new(LogList::new(&problems_only));

    let empty = adw::StatusPage::builder()
        .icon_name("document-open-recent-symbolic")
        .build();
    let loading = gtk::Spinner::builder()
        .spinning(true)
        .width_request(32)
        .height_request(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&empty, Some("empty"));
    stack.add_named(&log.scrolled, Some("log"));

    let toasts = adw::ToastOverlay::new();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(gettext("Copy Logs"))
        .build();
    copy.update_property(&[gtk::accessible::Property::Label(&gettext("Copy Logs"))]);
    let copied = log.clone();
    copy.connect_clicked(move |button| {
        button.clipboard().set_text(copied.text().trim_end());
        button.set_icon_name("object-select-symbolic");
        let weak = button.downgrade();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if let Some(button) = weak.upgrade() {
                button.set_icon_name("edit-copy-symbolic");
            }
        });
    });
    let save_button = gtk::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text(gettext("Save Logs"))
        .build();
    save_button.update_property(&[gtk::accessible::Property::Label(&gettext("Save Logs"))]);
    let (saved, name, overlay) = (log.clone(), timer.service.clone(), toasts.clone());
    save_button.connect_clicked(move |button| {
        let (button, saved, name, overlay) =
            (button.clone(), saved.clone(), name.clone(), overlay.clone());
        glib::spawn_future_local(async move {
            let message = match save(&button, &name, &saved.text()).await {
                Ok(Some(path)) => fill(&gettext("Saved to {path}"), &[("path", &path)]),
                Ok(None) => return,
                Err(err) => err,
            };
            overlay.add_toast(
                adw::Toast::builder()
                    .title(message)
                    .use_markup(false)
                    .timeout(5)
                    .build(),
            );
        });
    });

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(&gettext("Logs"), &timer.title))
        .build();
    header.pack_start(&problems_only);
    header.pack_end(&save_button);
    header.pack_end(&copy);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));
    toasts.set_child(Some(&toolbar));
    let page = adw::NavigationPage::builder()
        .title(gettext("Logs"))
        .tag("logs")
        .child(&toasts)
        .build();

    // Where the lines come from.
    let pending: Rc<RefCell<VecDeque<Entry>>> = Rc::default();
    let live = matches!(source, Source::Journal);
    let process = match source {
        Source::Journal => follow(timer, &pending),
        Source::Demo(entries) => {
            pending.borrow_mut().extend(entries);
            None
        }
    };
    let closed = Rc::new(Cell::new(false));
    {
        let closed = closed.clone();
        page.connect_hidden(move |_| {
            closed.set(true);
            if let Some(process) = &process {
                process.force_exit();
            }
        });
    }

    let ticks = Cell::new(0_u32);
    let tick = move || {
        if closed.get() {
            return glib::ControlFlow::Break;
        }
        ticks.set(ticks.get() + 1);
        let fresh: Vec<Entry> = pending.borrow_mut().drain(..).collect();
        log.append(&fresh);
        let name = if log.shown.n_items() > 0 {
            "log"
        } else if problems_only.is_active() {
            empty.set_title(&gettext("No Problems"));
            empty.set_description(Some(&gettext("Nothing went wrong in these logs.")));
            "empty"
        } else if ticks.get() >= EMPTY_AFTER_TICKS || !live {
            empty.set_title(&gettext("No Log Lines Yet"));
            empty.set_description(Some(&gettext(
                "Lines appear here as soon as this timer or its service writes some.",
            )));
            "empty"
        } else {
            "loading"
        };
        stack.set_visible_child_name(name);
        glib::ControlFlow::Continue
    };
    glib::timeout_add_local(FLUSH_EVERY, tick);
    page
}
