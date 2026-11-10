#!/bin/sh
# Takes the screenshots of the demo: every screen, in both themes and three
# widths, in English and French (docs/screenshots/<language>/). They are drawn
# from made-up timers; windows open briefly on the desktop while it runs, and a
# window that is hidden or minimised meanwhile cannot be captured, so a language
# is redone (up to three times) until all the files are there.
# Usage: scripts/screenshots.sh [output folder]
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/docs/screenshots}
expected=36
cargo build -q -p tempo --manifest-path "$root/Cargo.toml"
for lang in en fr; do
  for attempt in 1 2 3; do
    rm -rf "${out:?}/$lang"
    LANGUAGE=$lang TEMPO_SCREENSHOTS="$out/$lang" "$root/target/debug/tempo" || true
    [ "$(find "$out/$lang" -name "*.png" 2>/dev/null | wc -l)" -ge "$expected" ] && break
    echo "$lang: attempt $attempt took fewer than $expected screenshots" >&2
  done
done
