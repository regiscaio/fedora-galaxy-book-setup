%global app_id com.caioregis.GalaxyBookSetup
%global pkg_version %{?pkg_version_override}%{!?pkg_version_override:1.0.0}
%global source_date_epoch_from_changelog 0
%global clamp_mtime_to_source_date_epoch 1

Name:           galaxybook-setup
Version:        %{pkg_version}
Release:        1%{?dist}
Summary:        Installation and diagnostics assistant for Galaxy Book on Fedora

License:        GPL-3.0-only
URL:            https://github.com/regiscaio/fedora-galaxy-book-setup
Source0:        %{name}-%{version}.tar.gz

ExclusiveArch:  x86_64

BuildRequires:  appstream
BuildRequires:  bash
BuildRequires:  cargo
BuildRequires:  clang
BuildRequires:  coreutils
BuildRequires:  desktop-file-utils
BuildRequires:  diffutils
BuildRequires:  findutils
BuildRequires:  gettext
BuildRequires:  gawk
BuildRequires:  gcc-c++
BuildRequires:  grep
BuildRequires:  make
BuildRequires:  openssl
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  rust
BuildRequires:  sed
BuildRequires:  /usr/bin/flock
BuildRequires:  /usr/bin/gzip
BuildRequires:  /usr/bin/xz
BuildRequires:  /usr/bin/zstd

Requires:       akmods
Requires:       authselect
Requires:       bash
Requires:       coreutils
Requires:       dnf5
Requires:       diffutils
Requires:       dracut
Requires:       findutils
Requires:       gawk
Requires:       grep
Requires:       kmod
Requires:       libcamera-tools
Requires:       mokutil
Requires:       openssl
Requires:       polkit
Requires:       rpm
Requires:       /usr/bin/flock
Requires:       /usr/bin/gzip
Requires:       /usr/bin/sg
Requires:       /usr/bin/xz
Requires:       /usr/bin/zstd
Requires:       systemd
Requires:       systemd-udev
Requires:       usbutils
Requires:       v4l-utils
Recommends:     fprintd
Recommends:     gnome-control-center
Recommends:     gnome-shell
Recommends:     wireplumber
Recommends:     akmod-galaxybook-ov02c10 >= 1.00.23
Recommends:     galaxybook-ov02c10-kmod-common >= 1.00.23
Recommends:     kmod-galaxybook-ov02c10 >= 1.00.23
Recommends:     akmod-galaxybook-max98390 >= 1.00.10
Recommends:     galaxybook-max98390-kmod-common >= 1.00.10
Recommends:     kmod-galaxybook-max98390 >= 1.00.10
Recommends:     galaxybook-camera >= 1.00.43
Suggests:       galaxybook-sound >= 1.00.18

%description
Galaxy Book Setup is a native GTK4 and libadwaita helper for Fedora on Galaxy
Book notebooks. It organizes installation and diagnostic flows for hardware
support, starting with the internal camera stack and extending to MAX98390
audio support plus the Galaxy Book Sound panel.

%prep
%autosetup -n %{name}-%{version}

%build
APP_VERSION_OVERRIDE=%{version} cargo --offline build --release --locked --bin galaxybook-setup

%install
install -Dm755 target/release/galaxybook-setup %{buildroot}%{_bindir}/galaxybook-setup
install -Dm755 scripts/reconcile-kmods.sh %{buildroot}%{_libexecdir}/galaxybook-setup/reconcile-kmods
install -Dm755 scripts/configure-browser-camera.sh %{buildroot}%{_libexecdir}/galaxybook-setup/configure-browser-camera
install -Dm755 scripts/prepare-secure-boot-key.sh %{buildroot}%{_libexecdir}/galaxybook-setup/prepare-secure-boot-key
install -d -m0755 %{buildroot}%{_localstatedir}/lib/galaxybook-setup
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
appstreamcli validate --no-net data/%{app_id}.metainfo.xml
find scripts tests -type f -name '*.sh' -exec bash -n '{}' ';'
bash tests/reconcile-kmods.sh
bash tests/configure-browser-camera.sh
bash tests/prepare-secure-boot-key.sh
make i18n-validate VERSION=%{version}
APP_VERSION_OVERRIDE=%{version} cargo --offline test --locked --lib --bin galaxybook-setup

%files
%license LICENSE
%{_bindir}/galaxybook-setup
%{_libexecdir}/galaxybook-setup/reconcile-kmods
%{_libexecdir}/galaxybook-setup/configure-browser-camera
%{_libexecdir}/galaxybook-setup/prepare-secure-boot-key
%dir %{_localstatedir}/lib/galaxybook-setup
%{_datadir}/applications/%{app_id}.desktop
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/locale/en/LC_MESSAGES/%{name}.mo
%{_datadir}/locale/es/LC_MESSAGES/%{name}.mo
%{_datadir}/locale/it/LC_MESSAGES/%{name}.mo
%{_datadir}/metainfo/%{app_id}.metainfo.xml

%changelog
