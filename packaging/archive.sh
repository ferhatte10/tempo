#!/bin/sh
# Builds the release archive for this machine's processor, the one install.sh
# unpacks in the home folder. Run from the repository root:
#
#   packaging/archive.sh   → target/packages/tempo-<version>-linux-<arch>.tar.gz
#
# It holds the program, its desktop entry, icons and translations, laid out
# as under /usr (bin/, share/); Tempo finds its translations beside itself.
# It uses the system's GTK and libadwaita: built on the oldest supported
# system (Ubuntu 24.04), it starts on newer ones.
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
arch=$(uname -m)
name=tempo-$version-linux-$arch
stage=target/packages/archive
out=target/packages/$name.tar.gz

make all PREFIX=/usr
rm -rf "$stage"
make install PREFIX= DESTDIR="$stage/$name"
strip --strip-unneeded "$stage/$name/bin/tempo"
install -m644 LICENSE README.md "$stage/$name/"

tar -czf "$out" -C "$stage" "$name"
echo "$out"
