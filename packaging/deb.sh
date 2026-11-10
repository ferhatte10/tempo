#!/bin/sh
# Builds the .deb package (Ubuntu, Debian, Mint, Pop!_OS…) for this machine's
# architecture. Run from the repository root:
#
#   packaging/deb.sh   → target/packages/tempo_<version>_<arch>.deb
#
# The library dependencies are read from the program (dpkg-shlibdeps), so
# they match the system it was built on: the CI builds on Ubuntu 24.04, the
# oldest supported.
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
arch=$(dpkg --print-architecture)
stage=target/packages/deb-root
out=target/packages/tempo_${version}_${arch}.deb

make all PREFIX=/usr
rm -rf "$stage"
make install PREFIX=/usr DESTDIR="$stage"
strip --strip-unneeded "$stage/usr/bin/tempo"
install -Dm644 LICENSE "$stage/usr/share/doc/tempo/copyright"

shlibs_dir=$(mktemp -d)
mkdir -p "$shlibs_dir/debian"
printf 'Source: tempo\n\nPackage: tempo\nArchitecture: any\n' > "$shlibs_dir/debian/control"
binary=$(realpath "$stage/usr/bin/tempo")
libraries=$(cd "$shlibs_dir" && dpkg-shlibdeps -O -e "$binary" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
rm -rf "$shlibs_dir"
[ -n "$libraries" ] || { echo "Could not compute library dependencies" >&2; exit 1; }

mkdir -p "$stage/DEBIAN"
size=$(du -sk --exclude=DEBIAN "$stage" | cut -f1)
cat > "$stage/DEBIAN/control" <<CONTROL
Package: tempo
Version: $version
Architecture: $arch
Maintainer: Ferhat Saidoun <ferhatsnpro@gmail.com>
Installed-Size: $size
Depends: $libraries, systemd (>= 250)
Section: utils
Priority: optional
Homepage: https://tempo.ferhatsn.fr
Description: See and manage your systemd user timers
 Tempo shows what runs by itself on your computer: when each timer runs
 next, how its last runs went, and its logs. Drop a script on it to make a
 new timer: it is checked before it runs, and a notification tells you when
 a run fails. Timers of the system are shown read-only, with the commands to
 stop them.
CONTROL

mkdir -p target/packages
dpkg-deb --root-owner-group --build "$stage" "$out" >/dev/null
echo "$out"
