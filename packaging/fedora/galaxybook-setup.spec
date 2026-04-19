%global app_id com.caioregis.GalaxyBookSetup

Name:           galaxybook-setup
Version:        1.0.0
Release:        1%{?dist}
Summary:        Installation and diagnostics assistant for Galaxy Book on Fedora

License:        NOASSERTION
URL:            https://github.com/regiscaio/fedora-galaxy-book-setup
Source0:        %{name}-%{version}.tar.gz

ExclusiveArch:  x86_64

BuildRequires:  cargo
BuildRequires:  clang
BuildRequires:  desktop-file-utils
BuildRequires:  gcc-c++
BuildRequires:  make
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  rust

Recommends:     akmod-galaxybook-ov02c10 >= 0.1.0
Recommends:     galaxybook-camera >= 1.0.0

%description
Galaxy Book Setup is a native GTK4 and libadwaita helper for Fedora on Galaxy
Book notebooks. It organizes installation and diagnostic flows for hardware
support, starting with the internal camera stack.

%prep
%autosetup -n %{name}-%{version}

%build
cargo --offline build --release --locked --bin galaxybook-setup

%install
install -Dm755 target/release/galaxybook-setup %{buildroot}%{_bindir}/galaxybook-setup
install -Dm644 assets/galaxybook-setup.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
sed \
  -e 's|@EXEC@|galaxybook-setup|g' \
  -e 's|@ICON@|%{app_id}|g' \
  -e 's|@STARTUP_WM_CLASS@|%{app_id}|g' \
  data/%{app_id}.desktop > %{app_id}.desktop
install -Dm644 %{app_id}.desktop %{buildroot}%{_datadir}/applications/%{app_id}.desktop
install -Dm644 data/%{app_id}.metainfo.xml %{buildroot}%{_datadir}/metainfo/%{app_id}.metainfo.xml

%check
desktop-file-validate %{app_id}.desktop
cargo --offline test --locked --lib --bin galaxybook-setup

%files
%{_bindir}/galaxybook-setup
%{_datadir}/applications/%{app_id}.desktop
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/metainfo/%{app_id}.metainfo.xml

%changelog
* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-1
- Start the stable RPM line at 1.0.0

* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-6
- Reduce icon padding so the setup symbol occupies more of the launcher canvas

* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-5
- Refine the setup icon to match the Device Care geometry more closely

* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-4
- Rewrite the About details page to describe the setup app itself instead of camera flow commands

* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-3
- Replace the setup About dialog with the same native grouped modal style as the camera app
- Refresh the app icon with a Device Care-inspired visual inside the shared Galaxy Book card base

* Sat Apr 18 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-2
- Fix startup crash on AdwApplicationWindow by mounting the header inside window content

* Sat Apr 18 2026 Caio Régis <regiscaio@users.noreply.github.com> - 0.1.0-1
- Initial RPM packaging
