# Tempo: design system

Working name (renaming is cheap until the first release). A native GNOME app to
see and manage your own systemd timers. Direction chosen by the owner
(2026-11-03): **B**, faithful libadwaita with a stronger identity of its own.
Read this before writing or changing any screen; extend the tables here before
using a value that is not in them.

## 1. Atmosphere & identity

A calm control room for things that run by themselves. It should feel like part
of GNOME 50, then feel a little more alive: you open it and it already tells you
what happens next, in plain words. Quiet when all is well, clear and helpful
when something failed.

Two signatures, used nowhere else:

- **The next-run line**: the first thing on the home page, large, in words
  (“Next: Clean Stremio cache, in 12 min”).
- **The run strip**: ten small bars per timer, one per past run, green, red or
  grey. It replaces a table of dates.

Everything else is plain libadwaita: no custom window chrome, no fake glass, no
decoration that carries no meaning. Never a design that looks like a generic
admin panel.

## 2. Color

libadwaita's own tokens first (`@window_bg_color`, `@card_bg_color`,
`@window_fg_color`, `@accent_bg_color`…): they follow the system style and
contrast settings. The app adds only the roles below, defined once in
`data/style.css` for light and dark.

| Role | Token | Light | Dark | Usage |
|---|---|---|---|---|
| Accent | `--tempo-accent` | #5A3EDC | #B8ACFF | The app's colour: primary button, focus, the next-run line's time |
| Accent text on it | `--tempo-on-accent` | #FFFFFF | #1D1D20 | Text on an accent fill |
| Success | `--tempo-ok` | #1E8E4E | #57D38C | Last run succeeded |
| Warning | `--tempo-warn` | #855D00 | #F8E45C | A check to look at, a skipped run |
| Error | `--tempo-bad` | #C01C28 | #FF938C | Last run failed |
| Running | `--tempo-run` | = accent | = accent | A run in progress (pulse) |
| Idle / paused | `--tempo-idle` | #5E5E5E | #A8A8A8 | Disabled, never run, not ours |
| State tint | alpha 0.12 of the state colour | | | Card background for a failed or warned timer |

Rules:

- The accent marks what can be clicked and the single next-run time. Never
  decoration.
- A state is never shown by colour alone: it always has an icon and a word.
- `cargo test` fails when a text/background pair is under 4.5:1 (3:1 for dots and
  bars) in either theme, tints included.
- The accent was darkened (light) and lightened (dark) after the first check
  failed on the banner and chip tints. The check is `theme::tests` in the app
  crate (`cargo test`), not a separate script.

## 3. Typography

The system font and nothing bundled: GNOME's UI font (Adwaita Sans on GNOME 50),
monospace only for commands, file paths and logs.

| Level | libadwaita class | Usage |
|---|---|---|
| Next-run line | `title-1` (largest) | The home page headline |
| Page title | `title-2` | Detail page title |
| Card title | `heading` | Timer name |
| Body | default | Sentences |
| Meta | `caption`, `dim-label` | Relative times, paths, counters |
| Code | `monospace` | `systemctl` commands, paths, log lines |

Rules: no font below the system's caption size; text is selectable where it can
be copied; long paths ellipsize in the middle, never wrap mid-word.

## 4. Spacing & layout

Base unit 4 px (GTK scales it with the display). Tokens: 4, 8, 12, 16, 24, 32.

- Window: default 900×680, minimum 360×480. Content clamped to 760 px.
- Adaptive: below 640 sp (`AdwBreakpoint`) the list and the detail become one
  navigation stack instead of two columns.
- Card padding 16, gap between cards 12, section gap 24.
- No magic numbers: every margin is one of the tokens above.

## 5. Components

Each is documented here before it is built, with its states: default, hover,
focus, active, disabled, loading, empty, error.

- **Next-run banner**: icon, the sentence, a secondary line (“then Backup, in
  2 h”). Empty: “Nothing scheduled. Create your first timer.” Updates live.
- **Timer card**: state dot (pulses while running), name, schedule in words
  (“Every 30 minutes”), last result and relative time, the run strip, an origin
  chip (“Yours” or “System”). A failed timer gets the error tint and its fix.
- **Run strip**: ten bars, newest on the right; a tooltip gives date, duration
  and exit code of each; keyboard-reachable.
- **Fix suggestion**: a short sentence and the exact command in a monospace
  block with a Copy button (becomes a check mark for 2 s). Shown for every
  system timer and every failure; nothing is run for the user.
- **Drop overlay**: dropping a file anywhere dims the window and shows a
  highlighted zone (“Drop a script or a program”).
- **Analysis card**: one row per check, ✓ or ⚠, a one-line explanation and its
  fix. Never executes the file; “Test now” is a separate, confirmed button.
- **Schedule picker**: presets (every X minutes, hourly, daily at, weekly on),
  an expression field for experts, and the next 5 runs shown live.
- **Log view**: monospace lines, severity colours, live, sticky to the end only
  while the reader is at the end, filter, Copy, Save.
- **Empty state**: icon, one friendly sentence, one button.
- **Toast with Undo**: for delete and for any change that can be reverted.

## 6. Motion

Motion says what changed; it never decorates.

| What | Animation | Duration |
|---|---|---|
| Row added or removed | height and fade | 200 ms, ease-out |
| State change (dot, tint) | cross-fade | 250 ms |
| Running dot | slow pulse, only while running | 1.6 s loop |
| Dialog open | libadwaita default | system |
| Drop overlay | fade | 150 ms |
| Copy confirmation | icon swap | 120 ms |

Rules: all of it stops when the system turns animations off; nothing blocks the
interface (every D-Bus call is asynchronous); no animation longer than 400 ms
except the running pulse.

## 7. Language, accessibility and rules

- English is the source; French from the first screen (`gettext`, named
  placeholders). Plain words: “Every 30 minutes”, never `OnCalendar`.
- Every control has an accessible name; the whole app works with the keyboard;
  focus is always visible.
- Both themes and both languages are checked on screenshots at 360, 640 and
  900 px before a screen counts as done.
- A **demo mode** with made-up timers draws every screenshot: no real unit
  names, paths or journal lines ever reach an image.
- A busy button shows a spinner and its label.
- Scope guard: the app never changes a system or packaged timer. It shows the
  command to run instead.
