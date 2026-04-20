%global app_id com.caioregis.GalaxyBookSetup

Name:           galaxybook-setup
Version:        1.0.0
Release:        5%{?dist}
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

Recommends:     akmod-galaxybook-ov02c10 >= 1.0.0
Recommends:     akmod-galaxybook-max98390 >= 1.0.0
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
* Mon Apr 20 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-5
- Add a one-click main install flow to bootstrap camera and speaker support from the setup
- Update the README to document setup-first installation from the public DNF repository

* Mon Apr 20 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-4
- Detect MAX98390 kmod exposure failures more precisely in the diagnostics
- Add a manual fallback to install the speaker modules in the current kernel

* Mon Apr 20 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-3
- Add the initial MAX98390 speaker diagnostics and quick action
- Fix containerized manifest paths in the Makefile test/build flow

* Mon Apr 20 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-2
- Hide raw IPU6 V4L2 nodes when enabling the browser camera bridge

* Sun Apr 19 2026 Caio Régis <regiscaio@users.noreply.github.com> - 1.0.0-1
- Start the stable RPM line at 1.0.0
