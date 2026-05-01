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

Once the repository is configured, the app itself can already install the
notebook's main support set through the `Install core support` quick action,
pulling in the camera app, the `OV02C10` driver, and `MAX98390` speaker
support. It can also offer the installation of `Galaxy Book Sound`, which
handles the equalizer, profiles, and compatible Atmos mode.

`Galaxy Book Setup` is an installation and diagnostics helper for Samsung
Galaxy Book laptops on Fedora. The goal of the app is to organize setup flows
that would otherwise end up scattered across terminal commands, logs, RPM
packages, and manual validation steps.

The initial focus is the **internal camera** of the Galaxy Book4 Ultra, but
the project already covers the **internal speakers with MAX98390**, as well as
GPU, fingerprint, platform profile, and broader system integrations.

## Current interface

### Home screen

![Galaxy Book Setup — home screen](img/app-setup-galaxy-1.png)

### Diagnostics

![Galaxy Book Setup — diagnostics](img/app-setup-galaxy-2.png)

### Internal audio

![Galaxy Book Setup — internal audio](img/app-setup-galaxy-3.png)

### `About` modal

![Galaxy Book Setup — About](img/app-setup-galaxy-4.png)

## Scope

This app does not replace:

- the kernel driver;
- the final camera app;
- low-level tools such as `akmods`, `modinfo`, or `journalctl`.

Its role is to work as an **installation and validation assistant**, showing
the current state of the machine and organizing the next steps.

For audio, that means a clean split of responsibilities: `Galaxy Book Setup`
validates the internal-speaker path, organizes installation, and opens
`Galaxy Book Sound`, while equalization, profiles, and `compatible Atmos` stay
in the sound app.

## Relationship with the other repositories

This project works together with:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-max98390>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>
- <https://github.com/regiscaio/fedora-galaxy-book-sound>

Responsibilities:

- `fedora-galaxy-book-ov02c10`: packaged `ov02c10` module for Fedora;
- `fedora-galaxy-book-max98390`: packaged internal speaker support through MAX98390;
- `fedora-galaxy-book-camera`: daily-use camera app;
- `fedora-galaxy-book-sound`: equalizer, profiles, and compatible Atmos app with its own PipeWire backend;
- `fedora-galaxy-book-setup`: installation, diagnostics, and workflow assistant.

## Current capabilities

The current app already organizes the interface into well-defined areas:

- `System`: notebook, Fedora, kernel, and Secure Boot summary;
- `Diagnostics`: global checklist showing the state of the camera, browser
  bridge, audio, `Galaxy Book Sound`, fingerprint reader, GPU, the `akmods`
  MOK key, and desktop integrations, including the GNOME dock used on this
  notebook;
- `Quick actions`: install, repair, and driver priority adjustments; browser
  webcam enablement; internal-speaker enablement; Secure Boot key
  preparation for `MOK`; `Galaxy Book Sound` installation and launch;
  fingerprint stack repair; fingerprint-login enablement; opening
  fingerprint enrollment; NVIDIA flow; balanced profile; dock profile
  reapply; reboot; and camera-app launch.

Inside `Diagnostics`, each row opens a **suggested actions** subsection. That
allows the user to open the most relevant fixes and validations for the
selected item without losing the main `Quick actions` page.

The app also exposes a summary of warnings and errors through desktop
notifications. In docks and extensions that support launcher counters, the app
icon can show the total number of diagnostics currently marked as `Warning` or
`Error`.

The checklist currently covers:

- main camera packages;
- driver generation on boot via `akmods`;
- origin of the active `ov02c10` module;
- camera detection through the direct `libcamera` path used by `Galaxy Book
  Camera`;
- V4L2 bridge for browsers and communication apps;
- known boot errors;
- MAX98390 internal-speaker path, including the case where the package is
  installed but the current kernel still does not expose
  `snd-hda-scodec-max98390` via `modinfo`;
- presence of `Galaxy Book Sound`;
- presence of the integrated fingerprint reader;
- fingerprint-login readiness through `fprintd` and `authselect`;
- NVIDIA driver state and the note that `nvidia-smi` is optional;
- readiness of the `akmods` public key in `MOK` when Secure Boot is enabled;
- platform usage profile, with `balanced` highlighted as the recommended
  default;
- `Dash to Dock` state, including validation of the dock profile used on this
  notebook;
- GNOME extensions such as clipboard history, GSConnect, and desktop icons.

Quick actions do not just copy commands: they execute the main flows directly
through the interface, requesting administrative privilege when needed.

Today, the available actions include:

- installing the notebook's main support directly from setup, bringing in the
  camera app, the `OV02C10` driver, and `MAX98390` speaker support;
- installing the camera's main stack;
- rebuilding the driver with `akmods`;
- enabling `ov02c10` loading on boot and loading the module immediately;
- forcing priority for the fixed driver under `updates/`, with Secure Boot
  signing when needed, without incompatible compression, and with an explicit
  message when the current kernel already tried to start the camera too early;
- restoring the packaged Intel IPU6 stack when the direct `Galaxy Book
  Camera` path stops seeing the sensor;
- enabling browser camera support through `icamerasrc`, `v4l2-relayd`, and
  `v4l2loopback` while preserving direct `libcamera` access;
- enabling internal-speaker support through `MAX98390`, with module rebuild,
  manual installation fallback on the current kernel, and I2C service at boot;
- preparing the Secure Boot key for `akmods`, generating the local key,
  creating the import request in `MOK`, and leaving the reboot ready for
  `Enroll MOK` at boot;
- installing `Galaxy Book Sound` to apply equalization and compatible Atmos in
  the current session through PipeWire;
- reinstalling the fingerprint stack with `fprintd` and `libfprint`;
- enabling `with-fingerprint` in `authselect`;
- opening fingerprint enrollment directly in the user settings;
- installing or repairing NVIDIA support;
- applying the `balanced` platform profile;
- reapplying the GNOME dock profile used on this notebook, re-enabling `Dash
  to Dock` and restoring the expected auto-hiding bottom-dock behavior;
- rebooting the system;
- opening `Galaxy Book Camera`;
- opening `Galaxy Book Sound`.

## Camera After Kernel Updates

After a kernel update, boot may try to load `ov02c10` before `akmods` finishes
building the fixed module for that kernel. In that state, the log records:

```text
external clock 26000000 is not supported
probe with driver ov02c10 failed with error -22
```

Even if `modinfo -n ov02c10` later points to `updates/` after `akmods` finishes,
the IPU6 media graph for that boot may already have been created without the
sensor, so `cam -l` does not list the internal camera.

Diagnostics now treat this as a direct-camera path failure and suggest `Adjust
driver priority` followed by a reboot. The action rebuilds and prioritizes the
fixed module for the current kernel; the reboot recreates the media graph with
the correct driver available from the start of boot.

## Secure Boot and MOK

If a quick action fails with something like:

```text
modprobe: ERROR: could not insert 'ov02c10': Key was rejected by service
modprobe: ERROR: could not insert 'snd_hda_scodec_max98390': Key was rejected by service
```

the problem is not the module build itself. This means the kernel is still
running with `Secure Boot` enabled, but the key used to sign the module has
not yet been accepted by `MOK`.

The expected path is:

```bash
mokutil --test-key /etc/pki/akmods/certs/public_key.der
sudo mokutil --import /etc/pki/akmods/certs/public_key.der
```

If `mokutil --test-key` says the key `is already enrolled`, treat that as MOK
already configured. On some Fedora versions, this check may still return a
non-zero shell status in that case.

`Galaxy Book Setup` itself now exposes the `Prepare Secure Boot key` quick
action, which:

- generates the local `akmods` key with `kmodgenca` when needed;
- asks for a temporary `MOK` password in the interface;
- creates the import request through `mokutil`;
- updates diagnostics to show whether the key is ready, pending reboot, or
  still needs attention.

After that:

1. reboot the notebook;
2. enter `Enroll MOK` on the blue boot screen;
3. confirm the password set during `mokutil --import`;
4. return to Fedora and run the quick action again.

The `ov02c10` priority flow and the `MAX98390` enablement flow now perform
this check before trying to load the module, so the error no longer appears as
an opaque failure or a false success.

## User installation

### Via the public DNF repository

The recommended path for end users is:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

After that, inside the app itself:

1. open `Quick actions`;
2. run `Install core support`;
3. use the specific actions if camera, audio, NVIDIA, or the dock still need
   adjustment.

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

The RPM uses `Recommends` to point to the most important packages in the
workflow:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

That allows the app to be installed even before the full camera setup, which
is desirable for an installation helper.

## Roadmap

Planned next evolutions:

- broader Galaxy Book compatibility checks on Fedora;
- more assisted flows for GNOME desktop integrations and notebook peripherals;
- deeper fingerprint checks focused on post-suspend validation and busy-sensor
  scenarios.

## License

This project is distributed under **GPL-3.0-only**. See the [LICENSE](LICENSE)
file.
