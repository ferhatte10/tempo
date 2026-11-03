// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The page that says what Tempo found in a file: what it is, what may go
//! wrong, how to fix it, and the command a timer would run.

use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use tempo_core::analysis::{Analysis, Check, Fix, Id, Kind, Level};

use crate::snapshot::display_command;
use crate::widgets::command_block;
use crate::words::fill;

/// A check in words: its title, and what it means for the timer.
fn describe(check: &Check, analysis: &Analysis, host_arch: &str) -> (String, String) {
    let word = check.word.as_str();
    let with = |text: String, values: &[(&str, &str)]| fill(&text, values);
    match check.id {
        Id::Kind => match &analysis.kind {
            Kind::Directory => (
                gettext("This is a folder"),
                gettext(
                    "A timer runs a program or a script, not a folder. Choose the file inside it.",
                ),
            ),
            Kind::Program { appimage: true, .. } => (
                gettext("AppImage program"),
                gettext("A self-contained program."),
            ),
            Kind::Program {
                arch: Some(arch), ..
            } => (
                with(gettext("Program ({arch})"), &[("arch", arch)]),
                gettext("A compiled program."),
            ),
            Kind::Program { .. } => (gettext("Program"), gettext("A compiled program.")),
            Kind::Script {
                interpreter,
                shebang,
            } => (
                script_title(interpreter),
                if *shebang {
                    with(
                        gettext("Its first line asks for {name}."),
                        &[("name", interpreter)],
                    )
                } else {
                    with(
                        gettext("It has no #! line: Tempo guesses {name} from the file name."),
                        &[("name", interpreter)],
                    )
                },
            ),
            Kind::Text => (
                gettext("A text file Tempo cannot run"),
                gettext(
                    "It has no #! line and its name does not say which program reads it. Add a first line such as #!/bin/bash, or choose another file.",
                ),
            ),
            Kind::Unknown => (
                gettext("Not a program or a script"),
                gettext("This looks like data. Choose the program or script that uses it."),
            ),
        },
        Id::LineEndings => (
            gettext("Windows line endings"),
            with(
                gettext(
                    "The first line ends with an invisible carriage return, so the system looks for “{name}” followed by it, and fails. Convert the file.",
                ),
                &[("name", word)],
            ),
        ),
        Id::Interpreter => (
            with(gettext("{name} is not installed"), &[("name", word)]),
            with(
                gettext(
                    "The file needs {name}, which this computer does not have. Install it, or change the first line.",
                ),
                &[("name", word)],
            ),
        ),
        Id::Executable if check.level == Level::Ok => (
            gettext("Not marked executable"),
            with(
                gettext("That is fine: Tempo starts it with {name}."),
                &[("name", word)],
            ),
        ),
        Id::Executable => (
            gettext("Not marked executable"),
            gettext(
                "A timer cannot start a program that is not marked executable. Run the command below to fix it.",
            ),
        ),
        Id::Architecture => (
            gettext("Built for another processor"),
            with(
                gettext("This program is for {arch}, and this computer is {this}."),
                &[("arch", word), ("this", host_arch)],
            ),
        ),
        Id::Location => location(word),
        Id::RelativePaths => {
            let folder = analysis
                .suggestion
                .command
                .working_dir
                .as_deref()
                .map_or_else(String::new, |dir| dir.display().to_string());
            (
                gettext("Uses paths relative to its folder"),
                with(
                    gettext(
                        "It refers to files such as ./data.txt. Tempo will start it from its own folder, {folder}.",
                    ),
                    &[("folder", &folder)],
                ),
            )
        }
        Id::Privileges => (
            with(
                gettext("Asks for administrator rights ({tool})"),
                &[("tool", word)],
            ),
            gettext(
                "A timer has no terminal to type a password in, so this step will fail unless the command is set up to run without one.",
            ),
        ),
        Id::Input => (
            gettext("Waits for the keyboard"),
            gettext(
                "Nobody is there to type: it would wait for ever. Remove the question, or give it its answers.",
            ),
        ),
        Id::Desktop => (
            with(gettext("Uses a desktop tool ({tool})"), &[("tool", word)]),
            gettext("This works while you are logged in; it cannot show anything otherwise."),
        ),
        Id::RecursiveDelete => (
            gettext("Deletes files recursively"),
            gettext(
                "A mistake in the path or the variable could delete more than you mean. Check it before you schedule it.",
            ),
        ),
    }
}

fn location(word: &str) -> (String, String) {
    match word {
        "temporary" => (
            gettext("In a temporary folder"),
            gettext("Files there are deleted sooner or later, and the timer would then fail."),
        ),
        "downloads" => (
            gettext("In your Downloads folder"),
            gettext("This folder gets tidied, and the timer would fail the day the file goes."),
        ),
        "removable" => (
            gettext("On a removable drive"),
            gettext("The timer would fail whenever the drive is not plugged in."),
        ),
        "trash" => (
            gettext("In the trash"),
            gettext("The file will be deleted when the trash is emptied."),
        ),
        _ => (
            gettext("Kept in a stable place"),
            gettext(
                "It lives in your own files, not in a temporary folder or on a removable drive.",
            ),
        ),
    }
}

fn script_title(interpreter: &str) -> String {
    let name = interpreter;
    match name {
        "sh" | "bash" | "dash" | "zsh" | "fish" | "ksh" => {
            fill(&gettext("Shell script ({name})"), &[("name", name)])
        }
        _ if name.starts_with("python") => gettext("Python script"),
        "node" | "nodejs" => gettext("JavaScript script (Node)"),
        "perl" => gettext("Perl script"),
        "ruby" => gettext("Ruby script"),
        "php" => gettext("PHP script"),
        _ => fill(&gettext("Script ({name})"), &[("name", name)]),
    }
}

fn icon(level: Level) -> gtk::Image {
    let (name, class) = match level {
        Level::Ok => ("object-select-symbolic", "tempo-icon-ok"),
        Level::Warn => ("dialog-warning-symbolic", "tempo-icon-warn"),
        Level::Problem => ("dialog-error-symbolic", "tempo-icon-bad"),
    };
    gtk::Image::builder()
        .icon_name(name)
        .valign(gtk::Align::Center)
        .css_classes([class])
        .build()
}

/// One check as a row; a check with a command to copy opens to show it.
fn check_row(check: &Check, analysis: &Analysis, host_arch: &str) -> gtk::Widget {
    let (title, detail) = describe(check, analysis, host_arch);
    let detail = match &check.fix {
        Some(Fix::KeepCopy) => format!(
            "{detail}\n{}",
            gettext(
                "Tempo can keep a copy of the file in its own folder when you create the timer."
            )
        ),
        _ => detail,
    };
    if let Some(Fix::Command(command)) = &check.fix {
        let row = adw::ExpanderRow::builder()
            .title(title)
            .subtitle(detail)
            .use_markup(false)
            .build();
        row.add_prefix(&icon(check.level));
        let holder = gtk::Box::builder()
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();
        holder.append(
            &gtk::Label::builder()
                .label(gettext("Run this in a terminal"))
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        holder.append(&command_block(command));
        row.add_row(&holder);
        // Open on a problem: the fix is what the owner came for.
        row.set_expanded(check.level == Level::Problem);
        return row.upcast();
    }
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(detail)
        .use_markup(false)
        .build();
    row.add_prefix(&icon(check.level));
    row.upcast()
}

fn summary(analysis: &Analysis) -> String {
    match analysis.worst() {
        Level::Ok => gettext("Ready: a timer can run this file."),
        Level::Warn => gettext("A timer can run it, but look at these points first."),
        Level::Problem => gettext("A timer cannot run this file as it is."),
    }
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

/// The page for `path`. `choose_again` opens the file chooser.
pub fn page(
    path: &Path,
    analysis: &Analysis,
    host_arch: &str,
    choose_again: Rc<dyn Fn()>,
    create_timer: Rc<dyn Fn()>,
) -> adw::NavigationPage {
    let file_name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );

    let title = gtk::Label::builder()
        .label(file_name.as_str())
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(["title-2"])
        .build();
    let top = gtk::Box::builder().spacing(12).build();
    top.append(&icon(analysis.worst()));
    top.append(&title);
    let where_ = gtk::Label::builder()
        .label(path.display().to_string())
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["caption", "dim-label"])
        .build();
    let verdict = gtk::Label::builder()
        .label(summary(analysis))
        .xalign(0.0)
        .wrap(true)
        .build();
    let head = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    head.append(&top);
    head.append(&where_);
    head.append(&verdict);

    let checks = adw::PreferencesGroup::builder()
        .title(gettext("What Tempo found"))
        .build();
    for check in &analysis.checks {
        checks.add(&check_row(check, analysis, host_arch));
    }

    let command = &analysis.suggestion.command;
    let run = adw::PreferencesGroup::builder()
        .title(gettext("What the timer would run"))
        .build();
    run.add(&property(
        &gettext("Command"),
        &display_command(&command.program, &command.args),
    ));
    if let Some(dir) = &command.working_dir {
        run.add(&property(
            &gettext("Starts from the folder"),
            &dir.display().to_string(),
        ));
    }
    run.add(&property(
        &gettext("Suggested name"),
        &analysis.suggestion.name,
    ));
    run.add(&property(
        &gettext("Suggested title"),
        &analysis.suggestion.description,
    ));

    let create = gtk::Button::builder()
        .label(gettext("Create the Timer…"))
        .sensitive(analysis.worst() != Level::Problem)
        .css_classes(["pill", "suggested-action"])
        .build();
    if analysis.worst() == Level::Problem {
        create.set_tooltip_text(Some(&gettext("Fix the problems above first.")));
    }
    create.connect_clicked(move |_| create_timer());
    let another = gtk::Button::builder()
        .label(gettext("Choose Another File…"))
        .css_classes(["pill"])
        .build();
    another.connect_clicked(move |_| choose_again());
    let buttons = gtk::Box::builder()
        .spacing(12)
        .halign(gtk::Align::Center)
        .build();
    buttons.append(&another);
    buttons.append(&create);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(16)
        .margin_end(16)
        .build();
    content.append(&head);
    content.append(&checks);
    content.append(&run);
    content.append(&buttons);

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
        .title(gettext("New Timer"))
        .tag("review")
        .child(&toolbar)
        .build()
}
