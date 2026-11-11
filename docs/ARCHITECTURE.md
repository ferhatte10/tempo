# Architecture

Two crates. `tempo-core` is the logic, pure and tested; `tempo` is the app,
which reads the machine and draws it.

```
 file dropped ─► probe.rs ─► core::analysis ─► review.rs ─► editor.rs ─► core::draft ─► core::spec
                                                                                        │ render
 systemd (D-Bus) ─► systemd.rs ─┐                                         unit files ◄───┘
 journalctl      ─► journal.rs ─┼─► snapshot.rs ─► view.rs ─► window.rs / detail.rs / logs.rs
 unit files      ─► files.rs   ─┘
```

## Decisions

**Read from systemd, change only what Tempo wrote.** The user manager answers on
the session bus as `org.freedesktop.systemd1`. Tempo lists `*.timer` files and
loaded units, reads their properties (never `LoadUnit`, which would load a unit
into memory) and the files themselves. Actions exist for a timer only after
`files::managed` has recognised both files by their `X-Tempo-*` keys (systemd
ignores keys that start with `X-`), in the owner's folder.

**Files, not D-Bus, to change a schedule.** systemd cannot set a timer's
properties over D-Bus, so the form writes the `.timer` and `.service`, checks
them with `systemd-analyze --user verify` from a scratch folder, reloads, enables
and starts. A failed step puts everything back. The choices are stored in the
`X-Tempo-*` keys, so a timer is read back exactly and a hand edit is noticed
(`unit::edited_outside`).

**A failed run is told by systemd, not by Tempo.** A timer with
notifications on gets `OnFailure=tempo-notify@%n.service`; that template unit
runs this program with `--notify-failed <service>`, which reads why the run
failed, sends the notification over D-Bus and, on a click, opens Tempo in a
unit of its own (`systemd-run`), since the notifier's unit stops with all it
started. Nothing of Tempo runs in the background. The template points to the
program that last saved a timer, and is written again when it moves.

**Updates are announced, never installed.** Installs made by `install.sh` or
from a package ask `tempo.ferhatsn.fr/latest.json` (a redirect to the latest
release's `latest.json`) at most once a day, and only while “Check
Automatically” is on; a newer version shows a banner with the way to get it.
After an update, the first start offers the release notes of `CHANGELOG.md`.
This is the only network request Tempo makes.

**The journal through `journalctl`.** One call reads the job messages of all the
timers (the `MESSAGE_ID` of a job result with `USER_UNIT`, matched with the start
and the exit status by invocation id); another follows the lines of one timer. A
small reader of flat JSON objects avoids a dependency.

**A file is looked at, not run.** `core::analysis` works on the first 64 KiB and
on facts the app hands it, so it is tested without a machine. The text search for
`sudo`, `read` or `rm -r` is not a parse: it may flag a harmless line and misses
what a script builds at run time.

**Colours are tested.** `theme.rs` holds the palette of `DESIGN.md` and checks the
contrast of every text/background pair, tints included, in both themes.

**One time zone of thought for time.** A timer's next run is read on whichever
clock systemd gives (realtime or monotonic) and compared with that clock now.

## Tests

Unit tests run anywhere. Two tests marked `ignored` drive the real user systemd
(they write timers named `tempo-live-*` and remove them): `cargo test -p tempo
live_ -- --ignored --test-threads=1`.
