// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The window: the home page, its states (loading, error, empty, list), the
//! live link to systemd, and the screenshots of the demo.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use futures_util::StreamExt;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::settings::Settings;
use crate::systemd::{Saved, Systemd};
use crate::update::{self, Install};
use crate::view::{Action, Origin, TimerView};
use crate::words::fill;
use crate::{demo, detail, editor, files, logs, probe, review, widgets};
use tempo_core::analysis::{Analysis, Fix};
use tempo_core::draft::Draft;
use tempo_core::spec::{Command, TimerSpec};

const DEFAULT_SIZE: (i32, i32) = (900, 680);
/// Screenshots are rendered at this scale.
const SCALE: f32 = 1.0;
/// Changes often come in bursts: look once after this pause.
const REFRESH_DEBOUNCE: Duration = Duration::from_millis(300);
/// The relative times (“in 12 min”) are redrawn at least this often.
const REFRESH_EVERY_SECS: u32 = 60;
/// The first update check waits for the window to settle.
const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(5);
/// How often a check is considered; it happens at most once a day.
const UPDATE_TICK: Duration = Duration::from_secs(60 * 60);
/// How often Tempo looks whether a newer version replaced it on disk.
const RESTART_POLL: Duration = Duration::from_secs(60);
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the update banner says.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UpdateNotice {
    None,
    /// A newer version exists.
    Available(String),
    /// A newer version is installed: a restart runs it.
    Restart,
}

pub struct Ui {
    pub window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    nav: adw::NavigationView,
    home: RefCell<Option<adw::NavigationPage>>,
    /// Present once the link to systemd is up: the actions need it.
    systemd: RefCell<Option<Systemd>>,
    stack: gtk::Stack,
    error: adw::StatusPage,
    timers: RefCell<Rc<Vec<TimerView>>>,
    refresh_queued: Cell<bool>,
    update_banner: adw::Banner,
    update_notice: RefCell<UpdateNotice>,
    http: RefCell<Option<soup::Session>>,
}

fn section(title: &str, note: Option<&str>, list: &gtk::ListBox) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .build();
    section.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-4"])
            .build(),
    );
    if let Some(note) = note {
        section.append(
            &gtk::Label::builder()
                .label(note)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
    }
    section.append(list);
    section
}

impl Ui {
    pub fn new(app: &adw::Application, size: (i32, i32), preview: bool) -> Rc<Self> {
        let nav = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&nav));
        let drop_zone = gtk::Box::builder()
            .css_classes(["tempo-drop"])
            .visible(false)
            .can_target(false)
            .build();
        drop_zone.append(
            &gtk::Label::builder()
                .label(gettext("Drop a script or a program"))
                .css_classes(["title-2"])
                .hexpand(true)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .build(),
        );
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&toasts));
        overlay.add_overlay(&drop_zone);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Tempo")
            .default_width(size.0)
            .default_height(size.1)
            .width_request(360)
            .height_request(400)
            .content(&overlay)
            .build();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        let error = adw::StatusPage::builder()
            .icon_name("network-error-symbolic")
            .title(gettext("Cannot Reach systemd"))
            .build();
        let ui = Rc::new(Self {
            window,
            toasts,
            nav,
            home: RefCell::new(None),
            systemd: RefCell::new(None),
            stack,
            error,
            timers: RefCell::new(Rc::new(Vec::new())),
            refresh_queued: Cell::new(false),
            update_banner: adw::Banner::new(""),
            update_notice: RefCell::new(UpdateNotice::None),
            http: RefCell::new(None),
        });
        let home = ui.home_page(preview);
        ui.nav.add(&home);
        *ui.home.borrow_mut() = Some(home);
        ui.accept_drops(&drop_zone);
        ui.add_actions(app);
        ui
    }

    fn home_page(self: &Rc<Self>, preview: bool) -> adw::NavigationPage {
        let header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new("Tempo", ""))
            .build();
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(gettext("New Timer from a File…"))
            .build();
        add.update_property(&[gtk::accessible::Property::Label(&gettext(
            "New Timer from a File…",
        ))]);
        let ui = self.clone();
        add.connect_clicked(move |_| ui.choose_file());
        header.pack_start(&add);
        let menu = gio::Menu::new();
        let files = gio::Menu::new();
        files.append(Some(&gettext("_Choose a File…")), Some("win.open-file"));
        files.append(Some(&gettext("_Refresh")), Some("win.refresh"));
        menu.append_section(None, &files);
        if Install::current().checks_website() && !preview {
            let updates = gio::Menu::new();
            updates.append(
                Some(&gettext("Check for _Updates")),
                Some("win.check-updates"),
            );
            updates.append(
                Some(&gettext("Check _Automatically")),
                Some("win.auto-updates"),
            );
            menu.append_section(None, &updates);
        }
        let app = gio::Menu::new();
        app.append(Some(&gettext("_About Tempo")), Some("app.about"));
        app.append(Some(&gettext("_Quit")), Some("app.quit"));
        menu.append_section(None, &app);
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text(gettext("Main Menu"))
            .menu_model(&menu)
            .primary(true)
            .build();
        menu_button.update_property(&[gtk::accessible::Property::Label(&gettext("Main Menu"))]);
        header.pack_end(&menu_button);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&self.update_banner);
        if preview {
            let banner = adw::Banner::new(&gettext("A preview with made-up timers"));
            banner.set_revealed(true);
            toolbar.add_top_bar(&banner);
        }

        let loading = gtk::Spinner::builder()
            .spinning(true)
            .width_request(32)
            .height_request(32)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let empty = adw::StatusPage::builder()
            .icon_name("preferences-system-time-symbolic")
            .title(gettext("No Timers Yet"))
            .description(gettext(
                "Drop a script or a program on this window, or choose one, to run it on a schedule.",
            ))
            .build();
        let choose = gtk::Button::builder()
            .label(gettext("_Choose a File…"))
            .use_underline(true)
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let ui = self.clone();
        choose.connect_clicked(move |_| ui.choose_file());
        empty.set_child(Some(&choose));
        let retry = gtk::Button::builder()
            .label(gettext("_Try Again"))
            .use_underline(true)
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let ui = self.clone();
        retry.connect_clicked(move |_| {
            ui.stack.set_visible_child_name("loading");
            ui.start_live();
        });
        self.error.set_child(Some(&retry));

        self.stack.add_named(&loading, Some("loading"));
        self.stack.add_named(&empty, Some("empty"));
        self.stack.add_named(&self.error, Some("error"));
        self.stack.set_visible_child_name("loading");
        toolbar.set_content(Some(&self.stack));

        adw::NavigationPage::builder()
            .title("Tempo")
            .tag("home")
            .child(&toolbar)
            .build()
    }

    /// Shows these timers, now. The page opened on a timer keeps what it had.
    pub fn set_timers(self: &Rc<Self>, timers: Vec<TimerView>) {
        let timers = Rc::new(timers);
        *self.timers.borrow_mut() = timers.clone();
        if let Some(old) = self.stack.child_by_name("list") {
            self.stack.remove(&old);
        }
        if timers.is_empty() {
            self.stack.set_visible_child_name("empty");
            return;
        }
        self.stack
            .add_named(&self.home_content(&timers), Some("list"));
        self.stack.set_visible_child_name("list");
    }

    fn show_error(&self, message: &str) {
        self.error.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    fn home_content(self: &Rc<Self>, timers: &Rc<Vec<TimerView>>) -> gtk::ScrolledWindow {
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(28)
            .margin_top(24)
            .margin_bottom(32)
            .margin_start(16)
            .margin_end(16)
            .build();
        content.append(&widgets::next_banner(timers));

        let yours = self.list(timers, |origin| origin != Origin::System);
        if yours.first_child().is_some() {
            content.append(&section(&gettext("Your Timers"), None, &yours));
        }
        let system = self.list(timers, |origin| origin == Origin::System);
        if system.first_child().is_some() {
            content.append(&section(
                &gettext("System Timers"),
                Some(&gettext(
                    "Provided by your system or an app. Tempo shows them and tells you how to stop them, but never changes them.",
                )),
                &system,
            ));
        }

        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(
                &adw::Clamp::builder()
                    .maximum_size(760)
                    .tightening_threshold(560)
                    .child(&content)
                    .build(),
            )
            .build()
    }

    fn list(
        self: &Rc<Self>,
        timers: &Rc<Vec<TimerView>>,
        wanted: impl Fn(Origin) -> bool,
    ) -> gtk::ListBox {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["tempo-cards"])
            .build();
        for timer in timers.iter().filter(|t| wanted(t.origin)) {
            list.append(&widgets::card(timer));
        }
        let ui = self.clone();
        list.connect_row_activated(move |_, row| ui.show_detail(&row.widget_name()));
        list
    }

    pub fn show_detail(self: &Rc<Self>, unit: &str) {
        let timers = self.timers.borrow().clone();
        if let Some(timer) = timers.iter().find(|t| t.unit == unit) {
            self.nav.push(&detail::page(timer, &self.context()));
        }
    }

    /// The page of a timer again, from what systemd says now, in place of the
    /// one on screen.
    fn reopen(self: &Rc<Self>, unit: &str) {
        let timers = self.timers.borrow().clone();
        let (Some(timer), Some(home)) = (
            timers.iter().find(|t| t.unit == unit),
            self.home.borrow().clone(),
        ) else {
            self.nav.pop_to_tag("home");
            return;
        };
        let page = detail::page(timer, &self.context());
        self.nav.replace(&[home, page]);
    }

    /// Update notices (see `update.rs`): “What's New” after an update, then
    /// the website's `latest.json` for the installs that rely on it, and a
    /// restart once a newer version replaced this one.
    fn setup_updates(self: &Rc<Self>) {
        let ui = self.clone();
        self.update_banner
            .connect_button_clicked(move |_| ui.update_clicked());
        self.whats_new();

        let install = Install::current();
        if install.checks_website() {
            let known = Settings::load().update_available;
            if tempo_core::version::is_newer(&known, VERSION) {
                self.show_update(UpdateNotice::Available(known));
            }
            let ui = self.clone();
            glib::timeout_add_local_once(FIRST_UPDATE_CHECK, move || {
                ui.check_for_update();
                glib::timeout_add_local(UPDATE_TICK, move || {
                    ui.check_for_update();
                    glib::ControlFlow::Continue
                });
            });
        }
        let ui = self.clone();
        glib::timeout_add_local(RESTART_POLL, move || {
            if update::installed_while_running(install) {
                ui.show_update(UpdateNotice::Restart);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// First start of a new version: remember it and offer its release notes.
    fn whats_new(&self) {
        let mut settings = Settings::load();
        let previous = std::mem::replace(&mut settings.last_version, VERSION.to_owned());
        if previous == VERSION {
            return;
        }
        if !tempo_core::version::is_newer(&settings.update_available, VERSION) {
            settings.update_available.clear();
        }
        if let Err(err) = settings.save() {
            glib::g_warning!("tempo", "Cannot save settings: {err}");
        }
        // Nothing to announce on the very first start or after a downgrade.
        if previous.is_empty() || !tempo_core::version::is_newer(VERSION, &previous) {
            return;
        }
        self.toasts.add_toast(
            adw::Toast::builder()
                .title(fill(
                    &gettext("Tempo was updated to {version}"),
                    &[("version", VERSION)],
                ))
                .button_label(gettext("_What's New"))
                .action_name("app.about")
                .timeout(10)
                .build(),
        );
    }

    fn check_for_update(self: &Rc<Self>) {
        let settings = Settings::load();
        if !settings.check_updates
            || !tempo_core::version::check_due(settings.last_update_check, update::unix_now())
        {
            return;
        }
        self.fetch_update(false);
    }

    /// Asks the website for the latest version. A check asked from the menu
    /// says what it found, even when there is nothing new or no answer.
    fn fetch_update(self: &Rc<Self>, manual: bool) {
        let session = self
            .http
            .borrow_mut()
            .get_or_insert_with(update::new_session)
            .clone();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let latest = match update::fetch_latest(&session).await {
                Ok(latest) => latest,
                // Offline or the site is down: normal, tried again later.
                Err(err) => {
                    glib::g_info!("tempo", "Cannot check for updates: {err}");
                    if manual {
                        ui.toast(&gettext("Cannot check for updates right now"));
                    }
                    return;
                }
            };
            let newer = tempo_core::version::is_newer(&latest, VERSION);
            let mut settings = Settings::load();
            settings.last_update_check = update::unix_now();
            settings.update_available = if newer { latest.clone() } else { String::new() };
            if let Err(err) = settings.save() {
                glib::g_warning!("tempo", "Cannot save settings: {err}");
            }
            if newer {
                ui.show_update(UpdateNotice::Available(latest));
            } else if manual {
                ui.toast(&gettext("Tempo is up to date"));
            }
        });
    }

    /// Shows `notice` in the banner. A pending restart is never replaced.
    fn show_update(&self, notice: UpdateNotice) {
        {
            let current = self.update_notice.borrow();
            if *current == notice || *current == UpdateNotice::Restart {
                return;
            }
        }
        let (title, button) = match &notice {
            UpdateNotice::None => (String::new(), String::new()),
            UpdateNotice::Available(version) => (
                fill(
                    &gettext("Tempo {version} is available"),
                    &[("version", version)],
                ),
                gettext("_How to Update"),
            ),
            UpdateNotice::Restart => (
                gettext("Tempo was updated: restart it to use the new version"),
                gettext("_Restart"),
            ),
        };
        self.update_banner.set_title(&title);
        self.update_banner.set_button_label(Some(&button));
        self.update_banner.set_use_markup(false);
        self.update_banner
            .set_revealed(notice != UpdateNotice::None);
        self.update_notice.replace(notice);
    }

    fn update_clicked(self: &Rc<Self>) {
        let notice = self.update_notice.borrow().clone();
        match (notice, Install::current()) {
            (UpdateNotice::Restart, _) => crate::request_restart(&self.window),
            (UpdateNotice::Available(version), Install::Home) => self.explain_update(&version),
            (UpdateNotice::Available(_), _) => {
                gtk::UriLauncher::new(update::DOWNLOAD_URL).launch(
                    Some(&self.window),
                    None::<&gio::Cancellable>,
                    |result| {
                        if let Err(err) = result {
                            glib::g_warning!("tempo", "Cannot open the download page: {err}");
                        }
                    },
                );
            }
            (UpdateNotice::None, _) => {}
        }
    }

    /// An install made by `install.sh` updates by running it again.
    fn explain_update(&self, version: &str) {
        let dialog = adw::AlertDialog::builder()
            .heading(fill(
                &gettext("Update to Tempo {version}"),
                &[("version", version)],
            ))
            .body(gettext(
                "Run this command in a terminal: it installs the new version in your home folder, \
                 with no administrator password. Your timers are kept. Then restart Tempo.",
            ))
            .extra_child(&widgets::command_block(update::INSTALL_COMMAND))
            .build();
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(&self.window));
    }

    fn toast(&self, message: &str) {
        self.toasts.add_toast(
            adw::Toast::builder()
                .title(message)
                .use_markup(false)
                .timeout(4)
                .build(),
        );
    }

    fn context(self: &Rc<Self>) -> detail::Context {
        let ui = self.clone();
        detail::Context {
            handler: self.handler(),
            open_logs: Rc::new({
                let ui = ui.clone();
                move |unit| ui.open_logs(unit)
            }),
            edit: Rc::new(move |unit| ui.edit_timer(unit)),
        }
    }

    /// The logs of a timer: the journal when the link to systemd is up, made-up
    /// lines in the preview.
    fn open_logs(&self, unit: &str) {
        let timers = self.timers.borrow().clone();
        let Some(timer) = timers.iter().find(|t| t.unit == unit) else {
            return;
        };
        let source = if self.systemd.borrow().is_some() {
            logs::Source::Journal
        } else {
            logs::Source::Demo(demo::logs(timer))
        };
        self.nav.push(&logs::page(timer, source));
    }

    /// The buttons of a timer page work once the link to systemd is up.
    fn handler(self: &Rc<Self>) -> Option<detail::Handler> {
        self.systemd.borrow().as_ref()?;
        let ui = self.clone();
        Some(Rc::new(move |unit, action, button| {
            ui.act(unit, action, button);
        }))
    }

    fn act(self: &Rc<Self>, unit: &str, action: Action, button: &gtk::Button) {
        let Some(systemd) = self.systemd.borrow().clone() else {
            return;
        };
        let name = self
            .timers
            .borrow()
            .iter()
            .find(|t| t.unit == unit)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| unit.to_owned());
        widgets::set_busy(
            button,
            &match action {
                Action::RunNow => gettext("Starting…"),
                Action::Pause => gettext("Pausing…"),
                Action::Resume => gettext("Resuming…"),
                Action::TurnOff => gettext("Turning off…"),
                Action::TurnOn => gettext("Turning on…"),
                Action::Delete => gettext("Deleting…"),
                Action::Edit => return,
            },
        );
        let (ui, unit) = (self.clone(), unit.to_owned());
        glib::spawn_future_local(async move {
            let done = |text: String| fill(&text, &[("name", &name)]);
            let outcome = match action {
                Action::RunNow => systemd
                    .run_now(&unit)
                    .await
                    .map(|()| done(gettext("“{name}” started"))),
                Action::Pause => systemd
                    .pause(&unit)
                    .await
                    .map(|()| done(gettext("“{name}” paused"))),
                Action::Resume => systemd
                    .resume(&unit)
                    .await
                    .map(|()| done(gettext("“{name}” resumed"))),
                Action::TurnOff => systemd
                    .turn_off(&unit)
                    .await
                    .map(|()| done(gettext("“{name}” turned off"))),
                Action::TurnOn => systemd
                    .turn_on(&unit)
                    .await
                    .map(|()| done(gettext("“{name}” turned on"))),
                Action::Edit => return,
                Action::Delete => match systemd.delete(&unit).await {
                    Ok(saved) => {
                        ui.offer_undo(saved, &systemd, done(gettext("“{name}” deleted")));
                        Ok(String::new())
                    }
                    Err(err) => Err(err),
                },
            };
            match &outcome {
                Ok(message) if !message.is_empty() => ui.toast(message),
                Ok(_) => {}
                Err(err) => {
                    glib::g_warning!("tempo", "Action failed: {err}");
                    ui.toast(&fill(
                        &gettext("Could not do that: {error}"),
                        &[("error", err)],
                    ));
                }
            }
            // The timer needs a moment to follow, then the page shows the truth.
            glib::timeout_future(Duration::from_millis(300)).await;
            ui.refresh(&systemd).await;
            if action == Action::Delete && outcome.is_ok() {
                ui.nav.pop_to_tag("home");
            } else {
                ui.reopen(&unit);
            }
        });
    }

    /// A message with “Undo” for the next seconds after a deletion.
    fn offer_undo(self: &Rc<Self>, saved: Saved, systemd: &Systemd, message: String) {
        let toast = adw::Toast::builder()
            .title(message)
            .use_markup(false)
            .button_label(gettext("Undo"))
            .timeout(8)
            .build();
        let (ui, systemd) = (self.clone(), systemd.clone());
        toast.connect_button_clicked(move |_| {
            let (ui, systemd, saved) = (ui.clone(), systemd.clone(), saved.clone());
            glib::spawn_future_local(async move {
                match systemd.restore(&saved).await {
                    Ok(()) => ui.toast(&fill(
                        &gettext("“{name}” is back"),
                        &[("name", &saved.title)],
                    )),
                    Err(err) => {
                        glib::g_warning!("tempo", "Cannot bring {} back: {err}", saved.name);
                        ui.toast(&fill(
                            &gettext("Could not do that: {error}"),
                            &[("error", &err)],
                        ));
                    }
                }
                glib::timeout_future(Duration::from_millis(300)).await;
                ui.refresh(&systemd).await;
            });
        });
        self.toasts.add_toast(toast);
    }

    /// What the menu and the keys do. The ones that belong to the application
    /// (About, Quit) are added once, whatever the number of windows.
    fn add_actions(self: &Rc<Self>, app: &adw::Application) {
        let open = gio::SimpleAction::new("open-file", None);
        let ui = self.clone();
        open.connect_activate(move |_, _| ui.choose_file());
        self.window.add_action(&open);

        let refresh = gio::SimpleAction::new("refresh", None);
        let ui = self.clone();
        refresh.connect_activate(move |_, _| {
            let systemd = ui.systemd.borrow().clone();
            if let Some(systemd) = systemd {
                ui.queue_refresh(&systemd);
            }
        });
        self.window.add_action(&refresh);

        let back = gio::SimpleAction::new("back", None);
        let ui = self.clone();
        back.connect_activate(move |_, _| {
            ui.nav.pop();
        });
        self.window.add_action(&back);

        let check = gio::SimpleAction::new("check-updates", None);
        let ui = self.clone();
        check.connect_activate(move |_, _| ui.fetch_update(true));
        self.window.add_action(&check);

        let auto = gio::SimpleAction::new_stateful(
            "auto-updates",
            None,
            &Settings::load().check_updates.to_variant(),
        );
        auto.connect_change_state(|action, value| {
            let Some(on) = value.and_then(|value| value.get::<bool>()) else {
                return;
            };
            action.set_state(&on.to_variant());
            let mut settings = Settings::load();
            settings.check_updates = on;
            if let Err(err) = settings.save() {
                glib::g_warning!("tempo", "Cannot save settings: {err}");
            }
        });
        self.window.add_action(&auto);

        if app.lookup_action("about").is_none() {
            let about = gio::SimpleAction::new("about", None);
            let window = self.window.downgrade();
            about.connect_activate(move |_, _| {
                if let Some(window) = window.upgrade() {
                    show_about(&window);
                }
            });
            app.add_action(&about);
            let quit = gio::SimpleAction::new("quit", None);
            let to_quit = app.clone();
            quit.connect_activate(move |_, _| to_quit.quit());
            app.add_action(&quit);
        }
    }

    /// A file dragged over the window shows the drop zone; dropped, it is
    /// looked at.
    fn accept_drops(self: &Rc<Self>, zone: &gtk::Box) {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let shown = zone.clone();
        target.connect_enter(move |_, _, _| {
            shown.set_visible(true);
            gdk::DragAction::COPY
        });
        let hidden = zone.clone();
        target.connect_leave(move |_| hidden.set_visible(false));
        let (ui, hidden) = (self.clone(), zone.clone());
        target.connect_drop(move |_, value, _, _| {
            hidden.set_visible(false);
            let Ok(files) = value.get::<gdk::FileList>() else {
                return false;
            };
            let files = files.files();
            if files.len() > 1 {
                ui.toast(&gettext("Only the first file is looked at."));
            }
            match files.first().and_then(|file| file.path()) {
                Some(path) => {
                    ui.review(&path);
                    true
                }
                None => {
                    ui.toast(&gettext("That is not a file on this computer."));
                    false
                }
            }
        });
        self.window.add_controller(target);
    }

    /// Asks which file to look at.
    pub fn choose_file(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Choose a Script or a Program"))
            .initial_folder(&gio::File::for_path(glib::home_dir()))
            .modal(true)
            .build();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            match dialog.open_future(Some(&ui.window)).await {
                Ok(file) => match file.path() {
                    Some(path) => ui.review(&path),
                    None => ui.toast(&gettext("That is not a file on this computer.")),
                },
                Err(err) if err.matches(gtk::DialogError::Dismissed) => {}
                Err(err) => ui.toast(&err.to_string()),
            }
        });
    }

    /// Looks at a file, without running it, and shows what was found.
    pub fn review(self: &Rc<Self>, path: &std::path::Path) {
        match probe::analyze_path(path) {
            Ok(analysis) => self.show_review(path, &analysis),
            Err(err) => {
                glib::g_warning!("tempo", "Cannot look at {}: {err}", path.display());
                self.toast(&err);
            }
        }
    }

    pub fn show_review(
        self: &Rc<Self>,
        path: &std::path::Path,
        analysis: &tempo_core::analysis::Analysis,
    ) {
        let ui = self.clone();
        let again = ui.clone();
        let (create, creating, source) = (ui.clone(), analysis.clone(), path.to_path_buf());
        let page = review::page(
            path,
            analysis,
            std::env::consts::ARCH,
            Rc::new(move || again.choose_file()),
            Rc::new(move || create.new_timer(&source, &creating)),
        );
        match self.home.borrow().clone() {
            Some(home) => self.nav.replace(&[home, page]),
            None => self.nav.push(&page),
        }
    }

    // ---- The form --------------------------------------------------------

    fn editor_callbacks(self: &Rc<Self>) -> editor::Callbacks {
        let ui = self.clone();
        let save: editor::SaveHandler = Rc::new(
            move |spec: TimerSpec,
                  copy: Option<std::path::PathBuf>,
                  editing: bool,
                  done: Rc<dyn Fn()>| {
                let ui = ui.clone();
                glib::spawn_future_local(async move {
                    let Some(systemd) = ui.systemd.borrow().clone() else {
                        done();
                        return;
                    };
                    let unit = format!("{}.timer", spec.name);
                    match systemd.save(&spec, copy.as_deref(), editing).await {
                        Ok(()) => {
                            let text = if editing {
                                gettext("“{name}” saved")
                            } else {
                                gettext("“{name}” created")
                            };
                            ui.toast(&fill(&text, &[("name", &spec.description)]));
                            glib::timeout_future(Duration::from_millis(300)).await;
                            ui.refresh(&systemd).await;
                            ui.reopen(&unit);
                        }
                        Err(err) => {
                            glib::g_warning!("tempo", "Cannot save {unit}: {err}");
                            ui.alert(&gettext("The Timer Was Not Saved"), &err);
                            done();
                        }
                    }
                });
            },
        );
        let ui = self.clone();
        editor::Callbacks {
            save,
            try_now: Rc::new(move |command| ui.confirm_try(command)),
        }
    }

    /// The form for a new timer made from an analysed file.
    fn new_timer(self: &Rc<Self>, path: &std::path::Path, analysis: &Analysis) {
        let keep = analysis
            .checks
            .iter()
            .any(|check| check.fix == Some(Fix::KeepCopy))
            .then(|| path.to_path_buf());
        let page = editor::page(
            Draft::from_suggestion(&analysis.suggestion),
            editor::Options {
                editing: false,
                keep_copy_of: keep,
                changed_outside: false,
                live: self.systemd.borrow().is_some(),
            },
            self.editor_callbacks(),
        );
        self.nav.push(&page);
    }

    /// The form to change a timer of Tempo's.
    fn edit_timer(self: &Rc<Self>, unit: &str) {
        let name = unit.strip_suffix(".timer").unwrap_or(unit);
        let loaded = files::user_dir()
            .ok_or_else(|| "no home folder".to_owned())
            .and_then(|dir| files::managed(&dir, name));
        match loaded {
            Ok((spec, pair)) => {
                let page = editor::page(
                    Draft::from_spec(&spec),
                    editor::Options {
                        editing: true,
                        keep_copy_of: None,
                        changed_outside: tempo_core::unit::edited_outside(
                            &spec,
                            &pair.timer,
                            &pair.service,
                        ),
                        live: self.systemd.borrow().is_some(),
                    },
                    self.editor_callbacks(),
                );
                self.nav.push(&page);
            }
            Err(err) => self.alert(&gettext("This Timer Cannot Be Changed"), &err),
        }
    }

    fn alert(&self, title: &str, body: &str) {
        let dialog = adw::AlertDialog::new(Some(title), Some(body));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(&self.window));
    }

    /// Asks before running the owner's file once, then shows how it went.
    fn confirm_try(self: &Rc<Self>, command: Command) {
        let line = crate::snapshot::display_command(&command.program, &command.args);
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Run It Once Now?")),
            Some(&fill(
                &gettext(
                    "This runs “{command}” for real, once, with the same environment as the timer. It is stopped after a minute.",
                ),
                &[("command", &line)],
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("run", &gettext("_Run"));
        dialog.set_response_appearance("run", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let ui = self.clone();
        dialog.connect_response(None, move |_, response| {
            if response != "run" {
                return;
            }
            let (ui, command) = (ui.clone(), command.clone());
            glib::spawn_future_local(async move {
                let running = adw::Toast::builder()
                    .title(gettext("Running it…"))
                    .timeout(0)
                    .build();
                ui.toasts.add_toast(running.clone());
                let result = crate::systemd::try_now(&command).await;
                running.dismiss();
                match result {
                    Ok(result) => ui.show_try_result(&result),
                    Err(err) => ui.alert(&gettext("It Could Not Be Run"), &err),
                }
            });
        });
        dialog.present(Some(&self.window));
    }

    fn show_try_result(&self, result: &crate::systemd::TryResult) {
        let (title, body) = if result.exit_code == 0 {
            (gettext("It Worked"), gettext("It ended without an error."))
        } else {
            (
                gettext("It Failed"),
                fill(
                    &gettext("It ended with the exit code {code}."),
                    &[("code", &result.exit_code.to_string())],
                ),
            )
        };
        let dialog = adw::AlertDialog::new(Some(&title), Some(&body));
        if !result.output.trim().is_empty() {
            let output = gtk::Label::builder()
                .label(result.output.trim_end())
                .selectable(true)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .xalign(0.0)
                .css_classes(["monospace", "card"])
                .build();
            let scrolled = gtk::ScrolledWindow::builder()
                .min_content_height(80)
                .max_content_height(240)
                .propagate_natural_height(true)
                .child(&output)
                .build();
            dialog.set_extra_child(Some(&scrolled));
        }
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(&self.window));
    }

    /// Reads the owner's systemd, shows it, and keeps it up to date.
    pub fn start_live(self: &Rc<Self>) {
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let systemd = match Systemd::connect().await {
                Ok(systemd) => systemd,
                Err(err) => {
                    glib::g_warning!("tempo", "Cannot connect to systemd: {err}");
                    ui.show_error(&err.to_string());
                    return;
                }
            };
            *ui.systemd.borrow_mut() = Some(systemd.clone());
            if !ui.refresh(&systemd).await {
                return;
            }
            match systemd.changes().await {
                Ok(mut changes) => {
                    let (ui, systemd) = (ui.clone(), systemd.clone());
                    glib::spawn_future_local(async move {
                        while changes.next().await.is_some() {
                            ui.queue_refresh(&systemd);
                        }
                    });
                }
                Err(err) => glib::g_warning!("tempo", "Cannot watch systemd: {err}"),
            }
            let weak = Rc::downgrade(&ui);
            glib::timeout_add_seconds_local(REFRESH_EVERY_SECS, move || {
                let Some(ui) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                ui.queue_refresh(&systemd);
                glib::ControlFlow::Continue
            });
        });
    }

    /// False when the first look failed (the error page is shown).
    async fn refresh(self: &Rc<Self>, systemd: &Systemd) -> bool {
        match systemd.timers().await {
            Ok(timers) => {
                self.set_timers(timers);
                true
            }
            Err(err) => {
                glib::g_warning!("tempo", "Cannot read the timers: {err}");
                if self.stack.visible_child_name().as_deref() != Some("list") {
                    self.show_error(&err.to_string());
                }
                false
            }
        }
    }

    fn queue_refresh(self: &Rc<Self>, systemd: &Systemd) {
        if self.refresh_queued.replace(true) {
            return;
        }
        let (ui, systemd) = (self.clone(), systemd.clone());
        glib::timeout_add_local_once(REFRESH_DEBOUNCE, move || {
            ui.refresh_queued.set(false);
            glib::spawn_future_local(async move {
                ui.refresh(&systemd).await;
            });
        });
    }
}

/// The keys of the interface (see the README).
pub fn set_shortcuts(app: &adw::Application) {
    app.set_accels_for_action("win.open-file", &["<Primary>o", "<Primary>n"]);
    app.set_accels_for_action("win.refresh", &["F5", "<Primary>r"]);
    app.set_accels_for_action("win.back", &["Escape", "<Alt>Left"]);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
}

fn show_about(window: &adw::ApplicationWindow) {
    let about = adw::AboutDialog::builder()
        .application_name("Tempo")
        .application_icon(crate::APP_ID)
        .developer_name("Ferhat Saidoun")
        .version(env!("CARGO_PKG_VERSION"))
        .comments(gettext("See and manage your systemd user timers"))
        .license_type(gtk::License::Gpl30)
        .copyright("© 2026 Ferhat Saidoun")
        .website("https://tempo.ferhatsn.fr")
        .issue_url("https://github.com/ferhatte10/tempo/issues")
        .build();
    if let Some((version, notes)) = update::release_notes() {
        about.set_release_notes_version(&version);
        about.set_release_notes(&notes);
    }
    about.present(Some(window));
}

pub fn present(app: &adw::Application) {
    if let Some(dir) = std::env::var_os("TEMPO_SCREENSHOTS") {
        // Held from here: with no window yet, the application would quit.
        let hold = app.hold();
        glib::spawn_future_local(screenshots(app.clone(), PathBuf::from(dir), hold));
        return;
    }
    if let Some(path) = std::env::var_os("TEMPO_REVIEW") {
        // A developer aid: what the review says of a file, as text.
        match probe::analyze_path(std::path::Path::new(&path)) {
            Ok(analysis) => {
                println!("{:?}", analysis.kind);
                for check in &analysis.checks {
                    println!(
                        "  {:?} {:?} {:?} fix={:?}",
                        check.level, check.id, check.word, check.fix
                    );
                }
                let command = &analysis.suggestion.command;
                println!(
                    "  runs: {} | dir={:?} | name={} | title={}",
                    crate::snapshot::display_command(&command.program, &command.args),
                    command.working_dir,
                    analysis.suggestion.name,
                    analysis.suggestion.description
                );
            }
            Err(err) => println!("refused: {err}"),
        }
        return;
    }
    if std::env::var_os("TEMPO_PRINT").is_some() {
        let hold = app.hold();
        glib::spawn_future_local(print_timers(app.clone(), hold));
        return;
    }
    if std::env::var_os("TEMPO_DEMO").is_some() {
        let ui = Ui::new(app, DEFAULT_SIZE, true);
        ui.set_timers(demo::timers());
        ui.window.present();
        return;
    }
    let ui = Ui::new(app, DEFAULT_SIZE, false);
    ui.start_live();
    ui.setup_updates();
    ui.window.present();
}

fn render(widget: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = widget.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    snapshot.scale(SCALE, SCALE);
    paintable.snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let node = snapshot.to_node().ok_or("nothing to capture")?;
    let renderer = widget
        .native()
        .and_then(|native| native.renderer())
        .ok_or("the window is not shown")?;
    renderer
        .render_texture(&node, None)
        .save_to_png(path)
        .map_err(|err| err.to_string())
}

/// Every screen of the demo, in both themes and three widths, into `dir`.
async fn screenshots(app: adw::Application, dir: PathBuf, _hold: gtk::gio::ApplicationHoldGuard) {
    let settle = |ms| glib::timeout_future(Duration::from_millis(ms));
    if let Err(err) = std::fs::create_dir_all(&dir) {
        glib::g_critical!("tempo", "Cannot create {}: {err}", dir.display());
        app.quit();
        return;
    }
    // A window that is slow to appear has no size yet: try again a few times.
    let shot = async |ui: &Rc<Ui>, name: String| {
        let path = dir.join(format!("{name}.png"));
        for attempt in 0..5 {
            match render(&ui.window, &path) {
                Ok(()) => return,
                Err(err) if attempt == 4 => {
                    glib::g_critical!("tempo", "Cannot save {name}: {err}");
                }
                Err(_) => settle(600).await,
            }
        }
    };

    for (scheme, theme) in [
        (adw::ColorScheme::ForceLight, "light"),
        (adw::ColorScheme::ForceDark, "dark"),
    ] {
        adw::StyleManager::default().set_color_scheme(scheme);
        for (width, height) in [(360, 700), (640, 720), (900, 680)] {
            let ui = Ui::new(&app, (width, height), false);
            ui.set_timers(demo::timers());
            ui.window.present();
            settle(1200).await;
            shot(&ui, format!("home-{width}-{theme}")).await;
            if width != 640 {
                for (unit, name) in [
                    ("sync-notes.timer", "failed"),
                    ("clean-app-cache.timer", "yours"),
                    ("launchpadlib-cache-clean.timer", "system"),
                ] {
                    ui.show_detail(unit);
                    settle(900).await;
                    shot(&ui, format!("detail-{name}-{width}-{theme}")).await;
                    ui.nav.pop_to_tag("home");
                    settle(700).await;
                }
                for (index, (path, analysis)) in demo::reviews().iter().enumerate() {
                    ui.show_review(path, analysis);
                    settle(900).await;
                    let name = ["warn", "problem"][index];
                    shot(&ui, format!("review-{name}-{width}-{theme}")).await;
                    ui.nav.pop_to_tag("home");
                    settle(700).await;
                }
                if let Some((path, analysis)) = demo::reviews().first() {
                    ui.new_timer(path, analysis);
                    settle(1400).await;
                    shot(&ui, format!("editor-new-{width}-{theme}")).await;
                    ui.nav.pop_to_tag("home");
                    settle(700).await;
                }
                ui.show_detail("sync-notes.timer");
                settle(500).await;
                ui.open_logs("sync-notes.timer");
                settle(900).await;
                shot(&ui, format!("logs-failed-{width}-{theme}")).await;
                ui.nav.pop_to_tag("home");
                settle(400).await;
                ui.nav.pop_to_tag("home");
                settle(400).await;
            }
            ui.window.close();
            settle(300).await;
        }
        let empty = Ui::new(&app, DEFAULT_SIZE, false);
        empty.set_timers(Vec::new());
        empty.window.present();
        settle(900).await;
        shot(&empty, format!("empty-900-{theme}")).await;
        empty.window.close();
        settle(300).await;
    }
    app.quit();
}

/// A developer aid: prints what systemd says, as text, and quits. It reads
/// the real timers, so its output is not for sharing.
async fn print_timers(app: adw::Application, _hold: gtk::gio::ApplicationHoldGuard) {
    match Systemd::connect().await {
        Ok(systemd) => match systemd.timers().await {
            Ok(timers) => {
                for timer in timers {
                    println!(
                        "{:<48} {:<9} {:<8} next={:<8} last={:<22} {} | {} | {}",
                        timer.unit,
                        format!("{:?}", timer.origin),
                        format!("{:?}", timer.state),
                        timer
                            .next_in_secs
                            .map_or("-".to_owned(), |s| format!("{s}s")),
                        timer.last_run().map_or("-".to_owned(), |r| format!(
                            "{:?} {}s ago",
                            r.result, r.ago_secs
                        )),
                        crate::words::schedule(&timer.schedule),
                        timer.command,
                        timer.title,
                    );
                }
            }
            Err(err) => eprintln!("Cannot read the timers: {err}"),
        },
        Err(err) => eprintln!("Cannot connect to systemd: {err}"),
    }
    if std::env::var("TEMPO_PRINT").as_deref() == Ok("watch")
        && let Ok(systemd) = Systemd::connect().await
        && let Ok(mut changes) = systemd.changes().await
    {
        // Prints a line for each change seen in the next 15 seconds.
        let deadline = glib::timeout_future(Duration::from_secs(15));
        futures_util::pin_mut!(deadline);
        while let futures_util::future::Either::Left((Some(()), _)) =
            futures_util::future::select(changes.next(), deadline.as_mut()).await
        {
            let count = systemd.timers().await.map_or(0, |timers| timers.len());
            println!("-- change seen, {count} timers now");
        }
    }
    app.quit();
}
