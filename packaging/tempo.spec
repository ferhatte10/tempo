# RPM package (Fedora, RHEL-like, openSUSE), built from the source folder:
#
#   rpmbuild -bb --build-in-place --define "_rpmdir $PWD/target/packages" packaging/tempo.spec
#
# or `make rpm`. The CI builds it inside Fedora.

# The release build keeps no debug information to split out.
%global debug_package %{nil}

Name:           tempo
Version:        0.2.0
Release:        1%{?dist}
Summary:        See and manage your systemd user timers
License:        GPL-3.0-or-later
URL:            https://tempo.ferhatsn.fr

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  make
BuildRequires:  gcc
BuildRequires:  gettext
BuildRequires:  pkgconfig(gtk4) >= 4.14
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5
BuildRequires:  pkgconfig(libsoup-3.0)
BuildRequires:  desktop-file-utils

Requires:       gtk4 >= 4.14
Requires:       libadwaita >= 1.5
Requires:       libsoup3
Requires:       systemd >= 250

%description
Tempo shows what runs by itself on your computer: when each timer runs next,
how its last runs went, and its logs. Drop a script on it to make a new timer:
it is checked before it runs, and a notification tells you when a run fails.
Timers of the system are shown read-only, with the commands to stop them.

%build
make all PREFIX=%{_prefix}

%install
make install PREFIX=%{_prefix} DESTDIR=%{buildroot}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/fr.ferhatsn.Tempo.desktop

%files
%license LICENSE
%doc README.md
%{_bindir}/tempo
%{_datadir}/applications/fr.ferhatsn.Tempo.desktop
%{_datadir}/icons/hicolor/scalable/apps/fr.ferhatsn.Tempo.svg
%{_datadir}/icons/hicolor/symbolic/apps/fr.ferhatsn.Tempo-symbolic.svg
%{_datadir}/locale/*/LC_MESSAGES/tempo.mo

%changelog
* Wed Nov 11 2026 Ferhat Saidoun <ferhatsnpro@gmail.com> - 0.2.0-1
- Tell when a new version exists, and what is new after an update

* Wed Nov 11 2026 Ferhat Saidoun <ferhatsnpro@gmail.com> - 0.1.0-1
- First release
