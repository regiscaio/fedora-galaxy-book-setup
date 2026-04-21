%global app_id com.caioregis.GalaxyBookSetup
%global pkg_version %{?pkg_version_override}%{!?pkg_version_override:1.0.0}

Name:           galaxybook-setup
Version:        %{pkg_version}
Release:        1%{?dist}
Summary:        Installation and diagnostics assistant for Galaxy Book on Fedora

License:        GPL-2.0-only
URL:            https://github.com/regiscaio/fedora-galaxy-book-setup
Source0:        %{name}-%{version}.tar.gz

ExclusiveArch:  x86_64

BuildRequires:  cargo
BuildRequires:  clang
BuildRequires:  desktop-file-utils
BuildRequires:  gettext
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
APP_VERSION_OVERRIDE=%{version} cargo --offline build --release --locked --bin galaxybook-setup

%install
install -Dm755 target/release/galaxybook-setup %{buildroot}%{_bindir}/galaxybook-setup
install -Dm644 assets/galaxybook-setup.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
for lang in en es it; do \
  install -d %{buildroot}%{_datadir}/locale/${lang}/LC_MESSAGES; \
  msgfmt po/${lang}.po -o %{buildroot}%{_datadir}/locale/${lang}/LC_MESSAGES/%{name}.mo; \
done
sed \
  -e 's|@EXEC@|galaxybook-setup|g' \
  -e 's|@ICON@|%{app_id}|g' \
  -e 's|@STARTUP_WM_CLASS@|%{app_id}|g' \
  data/%{app_id}.desktop > %{app_id}.desktop
install -Dm644 %{app_id}.desktop %{buildroot}%{_datadir}/applications/%{app_id}.desktop
install -Dm644 data/%{app_id}.metainfo.xml %{buildroot}%{_datadir}/metainfo/%{app_id}.metainfo.xml

%check
desktop-file-validate %{app_id}.desktop
APP_VERSION_OVERRIDE=%{version} cargo --offline test --locked --lib --bin galaxybook-setup

%files
%license LICENSE
%{_bindir}/galaxybook-setup
%{_datadir}/applications/%{app_id}.desktop
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/locale/en/LC_MESSAGES/%{name}.mo
%{_datadir}/locale/es/LC_MESSAGES/%{name}.mo
%{_datadir}/locale/it/LC_MESSAGES/%{name}.mo
%{_datadir}/metainfo/%{app_id}.metainfo.xml

%changelog
