# Tempo: plan

Status: phases 1 to 8 done (phase 8: menu, About, shortcuts, icons, desktop entry, `make install`, docs). Core: 67 tests; app: 63 tests + 2 real ones. Generated files accepted by
`systemd-analyze --user verify` on systemd 259. Interface on demo data: home page,
timer page, empty state, English and French, light and dark, 14 tests (wording,
relative times, and the contrast of every colour pair). Clippy is clean.
Working name “Tempo”. Not a git repository yet.

Run the preview with `cargo run -p tempo` and redraw every screenshot with
`scripts/screenshots.sh` (`docs/screenshots/<language>/`, made-up data only).

## Goal

A native GNOME app (Rust, GTK4, libadwaita, `zbus`) to see and manage the
owner's **user** systemd timers. No Node, no Electron, no cron, no web view.

## Decisions

- Scope V1: `systemctl --user` only. System timers (root) are out.
- Timers the app creates (marked by an `X-` key in `[Unit]`) are fully editable
  by form. Every other timer is read-only: the app shows what it does and the
  exact command to stop, disable, mask or remove it, with a Copy button. It
  never changes them.
- A script or program is chosen by drag and drop or a file chooser, then
  analysed without running it (type, interpreter, executable bit, location,
  relative paths, `sudo`, graphical calls). The file is referenced in place,
  with a warning when its place is unstable and an offer to keep a copy.
- Logs: `journalctl --user -u … --output=json --follow` in a subprocess.
- Licence GPL-3.0-or-later; English and French.
- Design: direction B (see `DESIGN.md`), built first on demo data.
- Idea source: Stremio cache cleaner every 30 min, only when Stremio is closed
  (`ExecCondition=/bin/sh -c '! pgrep -x stremio'`).

## Phases

1. **Core**: timer model, schedule → `OnCalendar`, `.timer` / `.service`
   generation and reading of the app's own files. Unit tests, no system deps.
2. **Interface on demo data** (done): reviewed on screenshots in both themes,
   both languages and three widths. The buttons that need later phases (new
   timer, run now, pause, edit, delete) are drawn and disabled.
3. **systemd read-only** through `zbus` (done): the app lists the timers of the
   owner's systemd (`org.freedesktop.systemd1` on the session bus), with state,
   next run, schedule in words, the last run and who owns each: Yours (written
   by Tempo, recognised by its `X-Tempo-*` keys), Personal (written by hand
   under the home folder) or System. It follows changes by signals (debounced)
   and redraws every minute; it handles loading, error with “Try Again”, and
   no timers. Run `TEMPO_PRINT=1 target/debug/tempo` to print the real timers as
   text (`TEMPO_PRINT=watch` also prints a line per change for 15 s), and
   `TEMPO_DEMO=1` to open the preview on made-up timers.
   Known limit until phase 5: the run strip holds only the last run (from the
   service, or just its time when systemd no longer has the service loaded, shown
   as “Ran 2 h ago”); the history comes from the journal.
4. **Actions on the app's own timers** (done), and the commands to copy for the
   rest. On a timer written by Tempo: Run Now (starts the service), Pause and
   Resume (stop and start the timer: it stays enabled, so it starts again with
   the next session), Turn Off and Turn On (disable and enable it for good), and
   Delete (stops it, turns it off, removes its two files and reloads systemd,
   with an Undo message for 8 seconds that writes them back as they were). Each
   action first checks that both files are Tempo's, in the owner's folder
   (`files::managed`), and refuses otherwise; a button shows a spinner and its
   word while it works. Nothing else is ever written or removed. The buttons are
   disabled in the preview. `Edit…` waits for phase 7. A real test drives every
   action on a temporary timer in the real user folder and removes it:
   `cargo test -p tempo live_actions -- --ignored`.
5. **Logs** and the run history (done). The history comes from the user journal
   with one `journalctl --user --output=json` call for all the timers: each run
   is the message systemd writes when a job ends (`MESSAGE_ID` of the job result,
   with `USER_UNIT`), its start and its exit status are matched by invocation id
   (`journal.rs`; ids read on a real journal; the JSON lines are read by a small
   parser, no new dependency). A run skipped by a condition counts as skipped.
   The strip shows the last ten runs, and the page of each timer opens its logs:
   the lines of the timer and of its service, followed live (`--follow`, stopped
   when the page closes), problems coloured, a “Problems Only” filter, Copy, and
   Save (file mode 0600). Known limits: lines are classified by priority and a
   few words (`error`, `warn`) that programs print at the normal priority, so a
   harmless line with such a word is coloured; a program that exits 0 after
   printing an error counts as a success, as for systemd.
6. **File analysis** (done): a file dropped on the window (a dashed zone shows
   while it is dragged), or chosen with the “+” button or the empty page's
   button, is looked at and never run (`probe.rs` reads its permissions and its
   first 64 KiB, refuses pipes and devices; the pure rules are in
   `tempo-core::analysis`, 22 tests). The page says what it is (shell, Python,
   Node… script, program with its architecture, AppImage), and checks: an
   interpreter that is missing, Windows line endings after `#!` (with the `sed`
   command to copy), a program that is not executable (with `chmod +x`), a program
   for another processor, a file in `/tmp`, Downloads, a removable drive or the
   trash (a copy is offered), paths relative to its folder (Tempo then starts it
   from that folder), `sudo`, waiting for the keyboard, desktop tools, `rm -r`
   with a variable or a wildcard. It shows the command a timer would run (the
   file itself when it is executable with a `#!` line, otherwise its interpreter
   with the file as argument), a name and a title for the form. “Create the
   Timer…” is disabled until phase 7. `TEMPO_REVIEW=<file>` prints the review as
   text. Known limit: the look at a script is a text search, not a parse, so it
   misses what it builds at run time and may flag a harmless line.
7. **Editor** (done). “Create the Timer…” on the review page, and “Edit…” on the
   page of a timer of Tempo's, open the same form: title and name (the name
   follows the title until it is typed, and cannot change when editing), the
   command (a shell-like line, split by `tempo-core::shellwords`, never
   expanded) and the folder, a copy of the file when it lives somewhere unstable,
   the schedule (every few minutes, hourly, daily, some days of the week,
   monthly, or a calendar expression) with the next five runs asked of
   `systemd-analyze calendar`, conditions (a program that is not running, mains
   power, a file that exists), and options (catch up, spread, low priority, time
   limit). The form is `tempo-core::draft::Draft`, which says which field is
   wrong; the page shows the first problem in a banner and keeps the button off.
   Saving first has systemd check the two files (`systemd-analyze --user verify`
   from a scratch folder: a program that does not exist is refused), then writes
   them atomically, reloads, enables and starts the timer, and puts everything
   back if a step fails; changing a timer restarts it. A name that exists
   (a system unit too) is refused. “Try It…” runs the command once through
   `systemd-run --user --pipe --wait` (same environment as the timer, stopped
   after a minute) after a confirmation, and shows the exit code and the output.
   A kept copy lives in `~/.local/share/tempo/scripts/<name>/`; deleting the timer
   removes it and Undo brings it back (up to 1 MiB). Real tests:
   `cargo test -p tempo live_ -- --ignored --test-threads=1`.
8. **Polish**: translations, notifications on failure, documentation.

## To check on a prototype before relying on it

- The last result of a run that succeeded: the service's `Result` disappears
  once systemd unloads it, so the journal may be needed.
- Which cgroup limits (`CPUWeight`, `MemoryMax`) a user unit may set here;
  `Nice` and the idle I/O class are expected to work.
- How packaged and Snap timers are properly removed, for the commands shown.
- A failure notification through `OnFailure=` depends on a notifier being
  installed.
- User timers only run during a session unless linger is on; the app shows the
  command, it does not change it.
