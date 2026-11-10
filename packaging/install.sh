#!/bin/sh
# Tempo installer: https://tempo.ferhatsn.fr/install.sh
#
#   curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh
#   curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --dry-run
#   curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --uninstall
#
# Installs the latest release in your home folder, with no administrator
# password: the program and its files in ~/.local/share/tempo, the command
# in ~/.local/bin, the desktop entry and the icons in ~/.local/share. It
# uses your system's GTK and libadwaita (GTK 4.14 and libadwaita 1.5 or
# later: Ubuntu 24.04, Fedora 40, Debian 13 and later). Run it again to update.
# Uninstalling leaves your timers in place: delete them in Tempo first if you
# no longer want them.
#
# Options:
#   --dry-run     print the steps, change nothing
#   --uninstall   remove what this script installed
#   --help
#
# Source: https://github.com/ferhatte10/tempo/blob/main/packaging/install.sh
set -eu

REPO=ferhatte10/tempo
# Where the release files are (SHA256SUMS and the archive); tests point it elsewhere.
DOWNLOAD=${TEMPO_DOWNLOAD_BASE:-https://github.com/$REPO/releases/latest/download}
APP_ID=fr.ferhatsn.Tempo
# The archive is built on Ubuntu 24.04: it needs a C library as recent.
GLIBC_MIN=2.39

DATA=${XDG_DATA_HOME:-$HOME/.local/share}
BIN=$HOME/.local/bin
DEST=$DATA/tempo

dry_run=false
uninstall=false

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
note() { printf '  %s\n' "$*"; }
fail() { printf '\n\033[31mError:\033[0m %s\n' "$*" >&2; exit 1; }

# Runs a command, or only shows it with --dry-run.
run() {
  note "\$ $*"
  [ "$dry_run" = true ] || "$@"
}

usage() {
  cat <<'HELP'
Installs Tempo in your home folder (no administrator password needed).

  curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh
  curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --dry-run
  curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --uninstall

  --dry-run     print the steps, change nothing
  --uninstall   remove what this script installed (your timers stay)
HELP
}

while [ $# -gt 0 ]; do
  case $1 in
    --dry-run) dry_run=true ;;
    --uninstall) uninstall=true ;;
    --help|-h) usage; exit 0 ;;
    *) fail "unknown option: $1 (see --help)" ;;
  esac
  shift
done

[ "$(uname -s)" = Linux ] || fail "Tempo is a Linux app: it manages systemd timers."
[ "$(id -u)" != 0 ] || fail "run this as yourself, not as root: Tempo manages your own timers."

remove() {
  say "Removing Tempo from your home folder"
  run rm -rf "$DEST"
  run rm -f "$BIN/tempo" "$DATA/applications/$APP_ID.desktop" \
    "$DATA/icons/hicolor/scalable/apps/$APP_ID.svg" \
    "$DATA/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg"
  note "Your timers are still in ~/.config/systemd/user and keep running."
}

if [ "$uninstall" = true ]; then
  remove
  say "Done."
  exit 0
fi

case $(uname -m) in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) fail "no release for this processor ($(uname -m)): build Tempo from source, see https://github.com/$REPO" ;;
esac

command -v systemctl >/dev/null 2>&1 || fail "systemd is needed: Tempo manages its timers."
for tool in curl tar sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is needed by this script."
done

glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null | cut -d' ' -f2)
if [ -n "$glibc" ] && [ "$(printf '%s\n%s\n' "$GLIBC_MIN" "$glibc" | sort -V | head -n1)" != "$GLIBC_MIN" ]; then
  fail "this system's C library ($glibc) is older than $GLIBC_MIN: Tempo needs Ubuntu 24.04, Fedora 40, Debian 13 or later."
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

say "Downloading the latest release"
note "$DOWNLOAD"
curl -fsSL "$DOWNLOAD/SHA256SUMS" -o "$work/SHA256SUMS" || fail "cannot download the release's checksums."
name=$(sed -n "s/^[0-9a-f]\{64\}  \(tempo-[0-9][^ ]*-linux-$arch\.tar\.gz\)\$/\1/p" "$work/SHA256SUMS" | head -n1)
[ -n "$name" ] || fail "the latest release has no archive for $arch."
curl -fsSL "$DOWNLOAD/$name" -o "$work/$name" || fail "cannot download $name."
(cd "$work" && grep " $name\$" SHA256SUMS | sha256sum -c - >/dev/null) || fail "$name does not match its checksum: nothing was installed."
note "$name, checksum verified"
tar -xzf "$work/$name" -C "$work"
unpacked=$work/${name%.tar.gz}

# GTK and libadwaita come from the system: check they are there and recent enough.
if command -v ldd >/dev/null 2>&1; then
  missing=$(ldd "$unpacked/bin/tempo" 2>&1 | grep 'not found' || true)
  if [ -n "$missing" ]; then
    printf '%s\n' "$missing" >&2
    fail "GTK 4.14 and libadwaita 1.5 or later are needed. On Ubuntu or Debian: sudo apt install libadwaita-1-0; on Fedora: sudo dnf install libadwaita."
  fi
fi

say "Installing in your home folder"
run rm -rf "$DEST"
run mkdir -p "$DEST" "$BIN" "$DATA/applications" "$DATA/icons/hicolor/scalable/apps" "$DATA/icons/hicolor/symbolic/apps"
run cp -R "$unpacked/." "$DEST/"
run ln -sf "$DEST/bin/tempo" "$BIN/tempo"
note "\$ desktop entry → $DATA/applications/$APP_ID.desktop"
if [ "$dry_run" = false ]; then
  sed "s|^Exec=tempo|Exec=$DEST/bin/tempo|" "$unpacked/share/applications/$APP_ID.desktop" \
    > "$DATA/applications/$APP_ID.desktop"
fi
run cp "$unpacked/share/icons/hicolor/scalable/apps/$APP_ID.svg" "$DATA/icons/hicolor/scalable/apps/"
run cp "$unpacked/share/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg" "$DATA/icons/hicolor/symbolic/apps/"
if [ "$dry_run" = false ]; then
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$DATA/applications" 2>/dev/null || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true
  fi
fi

say "Done."
note "Open Tempo from your applications, or run: tempo"
case ":$PATH:" in
  *":$BIN:"*) ;;
  *) note "($BIN is not in your PATH yet: it is after you log in again.)" ;;
esac
note "To update, run this command again. To remove: curl -fsSL https://tempo.ferhatsn.fr/install.sh | sh -s -- --uninstall"
