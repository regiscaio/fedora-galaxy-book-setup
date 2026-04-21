<p align="center">
  <img src="assets/galaxybook-setup.svg" alt="Galaxy Book Setup icon" width="112">
</p>

<h1 align="center">Galaxy Book Setup</h1>

<p align="center">
  <a href="README.md">🇧🇷 Português</a> 
  <a href="README.en.md">🇺🇸 English</a> 
  <a href="README.es.md">🇪🇸 Español</a> 
  <a href="README.it.md">🇮🇹 Italiano</a>
</p>

## Quick install

To install the setup app from the public DNF repository:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Once the repository is configured, the app can already install the main support
set through the `Install core support` quick action, pulling in the camera app,
the `OV02C10` driver, and `MAX98390` speaker support.

`Galaxy Book Setup` is an installation and diagnostics assistant for Samsung
Galaxy Book laptops on Fedora. Its goal is to organize flows that would
otherwise be spread across terminal commands, logs, RPM packages, and manual
validation steps.

The current focus is the **internal camera** of the Galaxy Book4 Ultra, but the
project already tracks the **internal speakers through MAX98390**, as well as
GPU, platform profile, and general desktop integration. Fingerprint support
remains planned, but is not shipped by the current version.

## Scope

This app does not replace:

- the kernel driver;
- the final daily-use camera app;
- low-level tools such as `akmods`, `modinfo`, or `journalctl`.

Its role is to work as an **installation and validation assistant**, showing
the current state of the machine and organizing the next steps.

## Relationship with the other repositories

This project works together with:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-max98390>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>

Responsibilities:

- `fedora-galaxy-book-ov02c10`: packaged `ov02c10` kernel module for Fedora;
- `fedora-galaxy-book-max98390`: packaged internal speaker support through MAX98390;
- `fedora-galaxy-book-camera`: daily-use camera app;
- `fedora-galaxy-book-setup`: installation, diagnostics, and workflow assistant.

## Current capabilities

The current app already organizes the interface into clear areas:

- `System`: notebook, Fedora, kernel, and Secure Boot summary;
- `Diagnostics`: global checklist for camera, browser bridge, audio, GPU, and desktop integrations, including the GNOME dock profile used on this notebook;
- `Quick actions`: driver install, repair, priority override, browser camera enablement, speaker enablement, NVIDIA flow, balanced profile, dock profile reapply, reboot, and camera app launch;
- `Future modules`: reserved space for fingerprint and other flows.

Inside `Diagnostics`, each row also opens a **suggested actions** subpage. That
lets the user jump to the most relevant fixes and validations for the selected
item without losing the full quick-actions page.

The app also exposes a desktop notification summary for warnings and errors. In
docks and extensions that support launcher counters, the app icon can show the
total number of diagnostics currently marked as warning or error.

The checklist currently covers:

- main camera packages;
- driver generation on boot via `akmods`;
- origin of the active `ov02c10` module;
- direct `libcamera` detection used by `Galaxy Book Camera`;
- V4L2 bridge for browsers and communication apps;
- known boot errors;
- MAX98390 speaker path, including the case where the package is installed but
  the current kernel still does not expose `snd-hda-scodec-max98390` through `modinfo`;
- NVIDIA driver state and the fact that `nvidia-smi` is optional;
- platform profile state, with `balanced` as the recommended default;
- Dash to Dock state, including validation of the dock profile used on this
  notebook;
- GNOME extensions such as clipboard history, GSConnect, and desktop icons.

Quick actions do not just copy commands: they execute the main flows directly
from the UI, requesting administrative privilege when needed.

Current quick actions include reapplying the notebook's Dash to Dock profile,
re-enabling the extension and restoring the expected auto-hiding bottom dock
behavior when the desktop configuration drifts.

## User installation

### Via the public DNF repository

The recommended end-user path is:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Then, inside the app itself:

1. open `Quick actions`;
2. run `Install core support`;
3. use specific actions if camera, audio, NVIDIA, or the dock still need extra
   work.

### Via local RPMs

The project can also be packaged locally:

```bash
make rpm
```

Then the RPM can be installed with:

```bash
sudo dnf install /path/to/galaxybook-setup-*.rpm
```

## Build

Build dependencies on Fedora:

```bash
sudo dnf install cargo rust pkgconf-pkg-config gtk4-devel libadwaita-devel
```

If the host does not have the full toolchain, the `Makefile` uses a rootless
`podman` container.

Main commands:

```bash
make build
make test
make dist
make srpm
make rpm
```

To install the local development launcher:

```bash
make install-local
```

## Packaging

Relevant files:

- RPM spec: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- AppStream metadata: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

The RPM uses `Recommends` for the most important packages in the workflow:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

## Roadmap

Planned modules for future iterations:

- fingerprint;
- broader Galaxy Book compatibility checks on Fedora;
- more guided flows for GNOME desktop integrations and notebook peripherals.

## License

This project is distributed under **GPL-3.0-only**. See [LICENSE](LICENSE).
