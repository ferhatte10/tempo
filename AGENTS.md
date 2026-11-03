# Guide for AI coding assistants

Tempo is a GTK4/libadwaita app in Rust that shows and manages the owner's systemd
**user** timers. It talks to systemd over D-Bus (`zbus`) and to the journal
through `journalctl`. It never runs as root and never touches system timers.

## Read first

1. `DESIGN.md`: the design system. Every colour, size and motion comes from it;
   extend its tables before using a value that is not in them.
2. `docs/ARCHITECTURE.md`: modules and decisions. `docs/PLAN.md`: what is done.
3. The rules below.

## Rules

- Nothing is changed on a timer that Tempo did not write: `files::managed` must
  accept both files first. Anything else gets a command to copy, not an action.
- A file the owner chooses is never run by the analysis (`probe.rs`,
  `tempo-core::analysis`); only “Try It…”, after a confirmation, runs it.
- No new dependency or system package without the owner's agreement.
- English in the code and in `gettext()`; French in `po/fr.po`. Named
  placeholders (`{name}`). `ngettext` and `pgettext` take string literals, or
  `xgettext` cannot find them.
- Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`. The tests marked `ignored` use the real user
  systemd: run them only when asked.
- Screenshots (`scripts/screenshots.sh`) use made-up data from `demo.rs`: never
  real timers, paths or journal lines.

## Map

- `crates/core/` (`tempo-core`): pure logic, no GTK, D-Bus or file system.
  - `schedule.rs` when a timer fires; `spec.rs` a timer and its validation;
    `unit.rs` the `.timer` and `.service` files and reading them back;
  - `analysis.rs` what a file is and whether a timer can run it;
  - `draft.rs` the form and the timer it makes; `shellwords.rs` a command line.
- `crates/app/` (`tempo`): the app.
  - `systemd.rs` D-Bus: list, actions, save, verify, try; `journal.rs` the
    journal (history of runs, log lines); `files.rs` the owner's unit files and
    kept copies; `probe.rs` reads a file to look at it;
  - `snapshot.rs`, `unitfile.rs`, `view.rs` from what systemd says to what is shown;
  - `window.rs`, `detail.rs`, `review.rs`, `editor.rs`, `logs.rs`, `widgets.rs`
    the interface; `theme.rs` the colours and their contrast tests;
    `words.rs` the sentences; `demo.rs` made-up data.
- `data/` the stylesheet, the icons and the desktop entry; `po/` translations.

## Commands

```sh
cargo run -p tempo
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
