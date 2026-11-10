# Build and install Tempo from source.
#   make                 release build
#   sudo make install    install under $(PREFIX) (default /usr/local)
#   sudo make uninstall  remove what `make install` installed
#   make dev-install     desktop entry and icons in ~/.local, pointing to the
#                        debug build (gives `cargo run` its dock icon, no sudo)
#   make dev-uninstall   remove them
#   make check           format, lints, tests, desktop file
#   make screenshots     the screenshots of the demo (docs/screenshots)
#   make archive|deb|rpm the release files, in target/packages (packaging/)

PREFIX    ?= /usr/local
DESTDIR   ?=
APP_ID    := fr.ferhatsn.Tempo
LOCALEDIR := $(PREFIX)/share/locale
LINGUAS   := $(shell grep -v '^\#' po/LINGUAS)
# rustup installs cargo in ~/.cargo/bin, which is not always in PATH.
CARGO     ?= $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)

.PHONY: all run check install uninstall dev-install dev-uninstall screenshots archive deb rpm clean

all:
	LOCALEDIR=$(LOCALEDIR) $(CARGO) build --release --locked -p tempo

run:
	$(CARGO) run -p tempo

check:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(CARGO) test --workspace
	desktop-file-validate data/$(APP_ID).desktop

install:
	install -Dm755 target/release/tempo $(DESTDIR)$(PREFIX)/bin/tempo
	install -Dm644 data/$(APP_ID).desktop $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg
	install -Dm644 data/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg
	for lang in $(LINGUAS); do \
		mkdir -p $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES; \
		msgfmt -o $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES/tempo.mo po/$$lang.po; \
		chmod 644 $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES/tempo.mo; \
	done

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/tempo
	rm -f $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg
	for lang in $(LINGUAS); do rm -f $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES/tempo.mo; done

USER_DATA := $(HOME)/.local/share

dev-install:
	$(CARGO) build -p tempo
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg $(USER_DATA)/icons/hicolor/scalable/apps/$(APP_ID).svg
	install -Dm644 data/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg $(USER_DATA)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg
	mkdir -p $(USER_DATA)/applications
	sed 's|^Exec=tempo|Exec=$(CURDIR)/target/debug/tempo|' data/$(APP_ID).desktop > $(USER_DATA)/applications/$(APP_ID).desktop

dev-uninstall:
	rm -f $(USER_DATA)/applications/$(APP_ID).desktop
	rm -f $(USER_DATA)/icons/hicolor/scalable/apps/$(APP_ID).svg
	rm -f $(USER_DATA)/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg

screenshots:
	scripts/screenshots.sh

# Release files (packaging/): the archive install.sh unpacks, and the packages.
archive:
	packaging/archive.sh

deb:
	packaging/deb.sh

rpm:
	rpmbuild -bb --build-in-place --define "_rpmdir $(CURDIR)/target/packages" packaging/tempo.spec

clean:
	$(CARGO) clean
