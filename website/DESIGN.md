# Tempo website: design

The site of the Tempo app, in the same family as tunnelbird.ferhatsn.fr (same
type, same layout rules, same triangle), with Tempo's own colour and motif.
Read this before changing a page.

## Identity

- The app icon as is (`static/icon.svg`, copied from the app): never redrawn.
- One motif: the app's run strip, ten bars and the next run as an open circle
  (`RunStrip.svelte`), above the hero screenshot and alone on the error page.
  Nowhere else.
- The red and blue triangle, Ferhat Saidoun's personal mark (the arrowhead of
  the icon), small, before section titles (`TriangleMark.svelte`).
- Violet (`--accent`, `--link`, `--icon`) is for what can be clicked; red
  (`--mark-red`) only marks: current page underline, the triangle. The run
  colours (`--run-*`) are the app's states and appear only in the run strip.
- Real screenshots of the app (`make screenshots` in the repository, made-up
  timers), in the visitor's language, the dark ones with the dark theme. Never
  mock-ups drawn in HTML.

## Themes and colours

Tokens in `src/app.css`, `:root` (day) and `:root[data-theme='dark']`
(night); components use them through Tailwind (`bg-bg`, `text-text-2`…),
never raw colours. The theme follows the system, the sun/moon button
overrides it (kept in localStorage), applied by `static/theme.js` before the
first paint. `bun run check` fails if a text/background pair drops under
4.5:1 (3:1 for icons and the run strip) in either theme
(`scripts/contrast.js`).

## Type

- Headings: Bricolage Grotesque (variable), extra bold, tight.
- Text: Atkinson Hyperlegible, 18 px base: made for legibility, fits a
  non-technical audience.
- Monospace only for commands.
- Both self-hosted (fontsource): no request to Google.

## Motion

Motion says something: what to read first, that a section starts, where the
new theme comes from. Never a loop that keeps moving, never a decoration
alone. Tokens in `src/app.css` (`--ease-out`, `--motion-fast` 160 ms,
`--motion-base` 320 ms, `--motion-slow` 700 ms); only `transform` and `opacity` are animated.

- **Hero arrival** (`.enter`, `--d` = order): title, lead, command, buttons
  rise 1rem in a 90 ms cascade; the screenshot settles from slightly lower.
- **Signature moment**: the ten bars of the run strip grow one after the
  other, oldest first, then the next run appears (`RunStrip.svelte`). Once,
  on load.
- **Sections on scroll** (`use:reveal`, `--i` = order in a group): fade and
  rise once when they enter the screen; the two halves of the section
  triangle join. What is already on screen at load is never hidden.
- **Between pages** (View Transitions): fade-through, the old page leaves in
  140 ms, then the new one rises in; the header stays still. Changing
  language (a full load) does the same.
- **Day and night**: the new theme spreads as a circle from the button.
- **Feedback**: buttons press (`.press`, scale 0.97).
- **Reduced motion** (`prefers-reduced-motion`): everything is shown at once,
  no delay. **Without JavaScript**: nothing is ever hidden.

## Rules

- No AI-template habits: no pulsing status dot, no spaced uppercase mono
  overline, no row of key figures, no blurred halo or background grid, no
  bordered cards everywhere.
- Pointer cursor on everything clickable (set globally in `app.css`).
- Every text in `messages/en.json` and `messages/fr.json`, nothing hard-coded
  (command lines excepted).
- Phone first: no horizontal scroll at 375 px, touch targets of 44 px.
- Right from the first frame: what depends on the theme (icons, labels,
  screenshots) follows `<html data-theme>` through CSS, never a state set
  once the page starts; the fonts are preloaded; the hero images load at
  once in both themes. A reload must not flash or shift.
- Languages: each under its own name (`Intl.DisplayNames`, no labels to
  write); phones show up to 3 side by side in the menu, more as a list that
  scrolls; the desktop menu scrolls past its height. Changing language loads
  the page (see `src/lib/language.ts`). First visit of `/`: a browser whose
  first language is French opens `/fr/`, any other stays in English
  (`static/language.js`, before the first paint); a language chosen in the
  menu wins, kept in the browser like the theme (no cookie).
- French text: no-break space before `: ; ? !` and inside `« »`.
- French uses the app's own words (“minuteur”, “journal”); “timers systemd”
  only where the technical name helps people find the site.
- While `AVAILABLE` is false the site says “coming soon”, honestly: no fake
  download, no link to a source that is not public.
- Honest claims only: no “trusted by” with the author's own projects, no
  invented numbers.
