mod i18n;

use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

pub const APP_ID: &str = "com.caioregis.GalaxyBookSetup";
pub const APP_NAME: &str = "Galaxy Book Setup";
pub use i18n::{init_i18n, tr, trf, tr_mark, trn};
pub const CAMERA_APP_DESKTOP_ID: &str = "com.caioregis.GalaxyBookCamera.desktop";
pub const SOUND_APP_DESKTOP_ID: &str = "com.caioregis.GalaxyBookSound.desktop";
pub const AKMODS_PUBLIC_KEY_PATH: &str = "/etc/pki/akmods/certs/public_key.der";
pub const AKMODS_PRIVATE_KEY_PATH: &str = "/etc/pki/akmods/private/private_key.priv";
const CAMERA_APP_TUNING_FILE: &str =
    "/usr/share/galaxybook-camera/libcamera/simple/ov02c10.yaml";
pub const INSTALL_MAIN_SUPPORT_COMMAND: &str = "dnf install -y galaxybook-camera galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 galaxybook-max98390-kmod-common akmod-galaxybook-max98390 i2c-tools";
pub const INSTALL_CAMERA_COMMAND: &str =
    "dnf install -y galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 galaxybook-camera";
pub const INSTALL_SOUND_APP_COMMAND: &str = "dnf install -y galaxybook-sound";
pub const REPAIR_CAMERA_COMMAND: &str =
    r#"akmods --force --akmod galaxybook-ov02c10 --kernels "$(uname -r)" && depmod -a"#;
pub const ENABLE_CAMERA_MODULE_COMMAND: &str = r#"set -euo pipefail
install -d /etc/modules-load.d /etc/modprobe.d
cat > /etc/modules-load.d/galaxybook-ov02c10.conf <<'EOF'
ov02c10
EOF
cat > /etc/modprobe.d/galaxybook-ov02c10.conf <<'EOF'
softdep intel_ipu6_isys pre: ov02c10
EOF
akmods_key_enrolled() {
  local test_output
  if test_output="$(mokutil --test-key /etc/pki/akmods/certs/public_key.der 2>&1)"; then
    return 0
  fi
  printf '%s' "$test_output" | grep -qi 'already enrolled'
}
module_path="$(modinfo -n ov02c10 2>/dev/null || true)"
secure_boot_state="$(mokutil --sb-state 2>/dev/null || true)"
if printf '%s' "$secure_boot_state" | grep -qi 'enabled' \
  && printf '%s' "$module_path" | grep -Eq '/(updates|extra)/' \
  && [ -r /etc/pki/akmods/certs/public_key.der ] \
  && ! akmods_key_enrolled; then
  echo "Secure Boot está ativo, mas a chave do akmods ainda não foi inscrita no MOK. Execute 'sudo mokutil --import /etc/pki/akmods/certs/public_key.der', defina a senha, reinicie e conclua 'Enroll MOK' antes de tentar carregar o módulo novamente." >&2
  exit 1
fi
modprobe ov02c10
lsmod | grep '^ov02c10 '
"#;
pub const FORCE_CAMERA_DRIVER_COMMAND: &str = r#"set -euo pipefail
kernel="$(uname -r)"
workdir="$(mktemp -d)"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

ensure_akmods_secure_boot_ready() {
  if [ ! -r /etc/pki/akmods/private/private_key.priv ] || [ ! -r /etc/pki/akmods/certs/public_key.der ]; then
    echo "Secure Boot está ativo, mas a chave do akmods não está disponível para assinar o módulo." >&2
    exit 1
  fi

  local test_output
  if test_output="$(mokutil --test-key /etc/pki/akmods/certs/public_key.der 2>&1)"; then
    return 0
  fi

  if ! printf '%s' "$test_output" | grep -qi 'already enrolled'; then
    echo "Secure Boot está ativo, mas a chave do akmods ainda não foi inscrita no MOK. Execute 'sudo mokutil --import /etc/pki/akmods/certs/public_key.der', defina a senha, reinicie e conclua 'Enroll MOK' antes de tentar carregar o módulo novamente." >&2
    exit 1
  fi
}

akmods --force --akmod galaxybook-ov02c10 --kernels "$kernel"

source_rpm="$(readlink -f /usr/src/akmods/galaxybook-ov02c10-kmod.latest)"
if [ ! -f "$source_rpm" ]; then
  echo "O source RPM do akmod não foi encontrado em /usr/src/akmods." >&2
  exit 1
fi

if [ ! -d "/usr/src/kernels/$kernel" ]; then
  echo "Os headers do kernel atual não estão instalados: /usr/src/kernels/$kernel" >&2
  exit 1
fi

rpm2cpio "$source_rpm" | (cd "$workdir" && cpio -idm --quiet)
archive="$(find "$workdir" -maxdepth 1 -name 'galaxybook-ov02c10-kmod-*.tar.gz' | head -n1)"
if [ -z "$archive" ]; then
  echo "Não foi possível localizar o tarball do driver dentro do source RPM." >&2
  exit 1
fi

tar -C "$workdir" -xf "$archive"
srcdir="$(find "$workdir" -maxdepth 1 -mindepth 1 -type d -name 'galaxybook-ov02c10-kmod-*' | head -n1)"
if [ -z "$srcdir" ]; then
  echo "Não foi possível extrair a árvore do driver corrigido." >&2
  exit 1
fi

make -C "/usr/src/kernels/$kernel" M="$srcdir/module" modules
install -d "/lib/modules/$kernel/updates"
rm -f \
  "/lib/modules/$kernel/updates/ov02c10.ko" \
  "/lib/modules/$kernel/updates/ov02c10.ko.xz"

secure_boot_state="$(mokutil --sb-state 2>/dev/null || true)"
if printf '%s' "$secure_boot_state" | grep -qi 'enabled'; then
  ensure_akmods_secure_boot_ready
  sign_file="/usr/src/kernels/$kernel/scripts/sign-file"
  if [ ! -x "$sign_file" ]; then
    sign_file="/lib/modules/$kernel/build/scripts/sign-file"
  fi

  if [ ! -x "$sign_file" ]; then
    echo "O utilitário sign-file do kernel não foi encontrado para $kernel." >&2
    exit 1
  fi

  "$sign_file" sha256 \
    /etc/pki/akmods/private/private_key.priv \
    /etc/pki/akmods/certs/public_key.der \
    "$srcdir/module/ov02c10.ko"
fi

install -m 0644 "$srcdir/module/ov02c10.ko" "/lib/modules/$kernel/updates/ov02c10.ko"
if command -v restorecon >/dev/null 2>&1; then
  restorecon "/lib/modules/$kernel/updates/ov02c10.ko" || true
fi
depmod -a "$kernel"

if lsmod | grep -q '^ov02c10 '; then
  modprobe -r ov02c10 || true
fi
modprobe ov02c10
lsmod | grep '^ov02c10 '
modinfo -n ov02c10
modinfo -F signer "/lib/modules/$kernel/updates/ov02c10.ko" || true
"#;
pub const RESTORE_INTEL_CAMERA_COMMAND: &str = r#"set -euo pipefail
kernel="$(uname -r)"

dnf install -y \
  akmod-intel-ipu6 \
  ipu6-camera-bins \
  ipu6-camera-hal \
  gstreamer1-plugins-icamerasrc \
  libcamera \
  libcamera-ipa \
  libcamera-v4l2 \
  pipewire-plugin-libcamera

rm -f \
  "/lib/modules/$kernel/updates/ov02c10.ko" \
  "/lib/modules/$kernel/updates/ov02c10.ko.xz"

akmods --force --akmod intel-ipu6 --kernels "$kernel" || true
depmod -a "$kernel"

if lsmod | grep -q '^ov02c10 '; then
  modprobe -r ov02c10 || true
fi
modprobe ov02c10
lsmod | grep '^ov02c10 '
modinfo -n ov02c10
"#;
pub const ENABLE_BROWSER_CAMERA_COMMAND: &str = r#"set -euo pipefail
dnf install -y \
  v4l2-relayd \
  v4l2loopback \
  gstreamer1-plugins-icamerasrc \
  v4l-utils

install -d /etc/v4l2-relayd.d /etc/modprobe.d /etc/wireplumber/wireplumber.conf.d
cat > /etc/v4l2-relayd.d/icamerasrc.conf <<'EOF'
VIDEOSRC="icamerasrc"
FORMAT=NV12
WIDTH=1280
HEIGHT=720
FRAMERATE=30/1
CARD_LABEL="Intel MIPI Camera"
EOF

cat > /etc/modprobe.d/galaxybook-v4l2loopback.conf <<'EOF'
options v4l2loopback exclusive_caps=1 card_label="Intel MIPI Camera"
EOF

rm -f /etc/udev/rules.d/90-hide-ipu6-v4l2.rules

cat > /etc/wireplumber/wireplumber.conf.d/50-disable-ipu6-v4l2.conf <<'EOF'
monitor.v4l2.rules = [
  {
    matches = [
      { node.name = "~v4l2_input.pci-0000_00_05*" }
    ]
    actions = {
      update-props = {
        node.disabled = true
      }
    }
  }
]
EOF

udevadm control --reload-rules || true
udevadm trigger --action=change --subsystem-match=video4linux || true

systemctl stop v4l2-relayd@icamerasrc.service || true
modprobe -r v4l2loopback || true
modprobe v4l2loopback || true
udevadm settle || true

systemctl enable --now v4l2-relayd.service
systemctl enable --now v4l2-relayd@icamerasrc.service
systemctl restart v4l2-relayd@icamerasrc.service

device="$(grep -l -m1 -E '^Intel MIPI Camera$' /sys/devices/virtual/video4linux/*/name 2>/dev/null | xargs -r basename || true)"
if [ -n "$device" ] && command -v v4l2-ctl >/dev/null 2>&1; then
  v4l2-ctl -D -d "/dev/$device" || true
fi

echo "O bridge V4L2 foi configurado e qualquer regra antiga que removia o acesso direto do libcamera aos nós crus do IPU6 foi descartada. Faça logout/login ou reinicie a sessão se a lista de câmeras ainda não refletir o novo estado."
"#;
pub const ENABLE_SPEAKER_COMMAND: &str = r#"set -euo pipefail
kernel="$(uname -r)"
workdir="$(mktemp -d)"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

ensure_akmods_secure_boot_ready() {
  if [ ! -r /etc/pki/akmods/private/private_key.priv ] || [ ! -r /etc/pki/akmods/certs/public_key.der ]; then
    echo "Secure Boot está ativo, mas a chave do akmods não está disponível para assinar os módulos MAX98390." >&2
    exit 1
  fi

  local test_output
  if test_output="$(mokutil --test-key /etc/pki/akmods/certs/public_key.der 2>&1)"; then
    return 0
  fi

  if ! printf '%s' "$test_output" | grep -qi 'already enrolled'; then
    echo "Secure Boot está ativo, mas a chave do akmods ainda não foi inscrita no MOK. Execute 'sudo mokutil --import /etc/pki/akmods/certs/public_key.der', defina a senha, reinicie e conclua 'Enroll MOK' antes de tentar carregar os módulos MAX98390 novamente." >&2
    exit 1
  fi
}

dnf install -y \
  galaxybook-max98390-kmod-common \
  akmod-galaxybook-max98390 \
  i2c-tools

akmods --force --akmod galaxybook-max98390 --kernels "$kernel"

source_rpm="$(readlink -f /usr/src/akmods/galaxybook-max98390-kmod.latest)"
if [ ! -f "$source_rpm" ]; then
  echo "O source RPM do suporte MAX98390 não foi encontrado em /usr/src/akmods." >&2
  exit 1
fi

if [ ! -d "/usr/src/kernels/$kernel" ]; then
  echo "Os headers do kernel atual não estão instalados: /usr/src/kernels/$kernel" >&2
  exit 1
fi

rpm2cpio "$source_rpm" | (cd "$workdir" && cpio -idm --quiet)
archive="$(find "$workdir" -maxdepth 1 -name 'galaxybook-max98390-kmod-*.tar.gz' | head -n1)"
if [ -z "$archive" ]; then
  echo "Não foi possível localizar o tarball do suporte MAX98390 dentro do source RPM." >&2
  exit 1
fi

tar -C "$workdir" -xf "$archive"
srcdir="$(find "$workdir" -maxdepth 1 -mindepth 1 -type d -name 'galaxybook-max98390-kmod-*' | head -n1)"
if [ -z "$srcdir" ]; then
  echo "Não foi possível extrair a árvore do suporte MAX98390." >&2
  exit 1
fi

make -C "/usr/src/kernels/$kernel" M="$srcdir/module" modules

updates_dir="/lib/modules/$kernel/updates/sound/hda/codecs/side-codecs"
install -d "$updates_dir"
rm -f \
  "$updates_dir/snd-hda-scodec-max98390.ko" \
  "$updates_dir/snd-hda-scodec-max98390.ko.xz" \
  "$updates_dir/snd-hda-scodec-max98390-i2c.ko" \
  "$updates_dir/snd-hda-scodec-max98390-i2c.ko.xz"

secure_boot_state="$(mokutil --sb-state 2>/dev/null || true)"
if printf '%s' "$secure_boot_state" | grep -qi 'enabled'; then
  ensure_akmods_secure_boot_ready
  sign_file="/usr/src/kernels/$kernel/scripts/sign-file"
  if [ ! -x "$sign_file" ]; then
    sign_file="/lib/modules/$kernel/build/scripts/sign-file"
  fi

  if [ ! -x "$sign_file" ]; then
    echo "O utilitário sign-file do kernel não foi encontrado para $kernel." >&2
    exit 1
  fi

  "$sign_file" sha256 \
    /etc/pki/akmods/private/private_key.priv \
    /etc/pki/akmods/certs/public_key.der \
    "$srcdir/module/snd-hda-scodec-max98390.ko"
  "$sign_file" sha256 \
    /etc/pki/akmods/private/private_key.priv \
    /etc/pki/akmods/certs/public_key.der \
    "$srcdir/module/snd-hda-scodec-max98390-i2c.ko"
fi

install -m 0644 "$srcdir/module/snd-hda-scodec-max98390.ko" \
  "$updates_dir/snd-hda-scodec-max98390.ko"
install -m 0644 "$srcdir/module/snd-hda-scodec-max98390-i2c.ko" \
  "$updates_dir/snd-hda-scodec-max98390-i2c.ko"

if command -v restorecon >/dev/null 2>&1; then
  restorecon "$updates_dir/snd-hda-scodec-max98390.ko" || true
  restorecon "$updates_dir/snd-hda-scodec-max98390-i2c.ko" || true
fi

depmod -a "$kernel"

modprobe snd-hda-scodec-max98390
modprobe snd-hda-scodec-max98390-i2c

systemctl enable max98390-hda-i2c-setup.service
systemctl enable max98390-hda-check-upstream.service || true
systemctl start max98390-hda-i2c-setup.service
systemctl start max98390-hda-check-upstream.service || true

lsmod | grep '^snd_hda_scodec_max98390' || true
modinfo -n snd-hda-scodec-max98390
modinfo -n snd-hda-scodec-max98390-i2c
"#;
pub const PREPARE_SECURE_BOOT_KEY_COMMAND: &str = r#"set -euo pipefail
public_key="/etc/pki/akmods/certs/public_key.der"
private_key="/etc/pki/akmods/private/private_key.priv"

if ! command -v mokutil >/dev/null 2>&1; then
  echo "O utilitário mokutil não está disponível neste sistema." >&2
  exit 1
fi

if ! command -v kmodgenca >/dev/null 2>&1; then
  echo "O utilitário kmodgenca não está disponível neste sistema." >&2
  exit 1
fi

install -d /etc/pki/akmods/certs /etc/pki/akmods/private

if [ ! -r "$public_key" ] || [ ! -r "$private_key" ]; then
  kmodgenca -a -f
fi

if [ ! -r "$public_key" ] || [ ! -r "$private_key" ]; then
  echo "Não foi possível gerar a chave local do akmods." >&2
  exit 1
fi

test_output=""
if test_output="$(mokutil --test-key "$public_key" 2>&1)" \
  || printf '%s' "$test_output" | grep -qi 'already enrolled'; then
  echo "A chave do akmods já está inscrita no MOK." >&2
  exit 0
fi

hash_file="$(mktemp)"
cleanup() {
  rm -f "$hash_file"
}
trap cleanup EXIT
chmod 600 "$hash_file"

mokutil --generate-hash | tail -n1 > "$hash_file"
mokutil --import "$public_key" --hash-file "$hash_file"

if mokutil --list-new 2>/dev/null | grep -q .; then
  echo "O pedido de importação da chave do akmods foi criado. Reinicie o notebook e conclua 'Enroll MOK' na tela azul do boot usando a senha definida nesta etapa."
else
  echo "A chave do akmods foi preparada, mas o mokutil não listou nenhum pedido pendente. Revise a saída acima antes de continuar." >&2
fi
"#;
pub const REPAIR_NVIDIA_COMMAND: &str =
    r#"dnf install -y akmod-nvidia && akmods --force --kernels "$(uname -r)" && depmod -a && dracut --force"#;
pub const SET_BALANCED_PROFILE_COMMAND: &str = r#"if [ -w /sys/firmware/acpi/platform_profile ]; then printf 'balanced' > /sys/firmware/acpi/platform_profile; cat /sys/firmware/acpi/platform_profile; else echo 'O perfil da plataforma não está disponível neste sistema.' >&2; exit 1; fi"#;
pub const REPAIR_FINGERPRINT_COMMAND: &str =
    r#"dnf reinstall -y fprintd libfprint && systemctl restart fprintd && rpm -q fprintd libfprint"#;
pub const ENABLE_FINGERPRINT_AUTH_COMMAND: &str =
    r#"authselect enable-feature with-fingerprint && authselect apply-changes && authselect current"#;
pub const OPEN_FINGERPRINT_SETTINGS_COMMAND: &str = r#"if ! command -v gnome-control-center >/dev/null 2>&1; then echo "O gnome-control-center não está disponível neste sistema." >&2; exit 1; fi
nohup gnome-control-center user-accounts >/dev/null 2>&1 &
echo "As configurações de Usuários foram abertas para gerenciar o cadastro de digitais."
"#;
pub const REBOOT_COMMAND: &str = "systemctl reboot -i";

const CLIPBOARD_EXTENSION_IDS: &[&str] = &[
    "clipboard-indicator@tudmotu.com",
    "clipboard-history@alexsaveau.dev",
    "GPaste@gnome-shell-extensions.gnome.org",
    "pano@elhan.io",
];
const CLIPBOARD_PROFILE_EXTENSION_ID: &str = "clipboard-indicator@tudmotu.com";
const GSCONNECT_EXTENSION_ID: &str = "gsconnect@andyholmes.github.io";
const DESKTOP_ICONS_EXTENSION_ID: &str = "ding@rastersoft.com";
const DASH_TO_DOCK_EXTENSION_ID: &str = "dash-to-dock@micxgx.gmail.com";
const FINGERPRINT_SENSOR_PATTERNS: &[&str] = &[
    "1c7a:05a1",
    "LighTuning Technology Inc. ETU905A80-E",
    "Egis",
    "LighTuning",
];
const DASH_TO_DOCK_SCHEMA: &str = "org.gnome.shell.extensions.dash-to-dock";
const DASH_TO_DOCK_PROFILE_SETTINGS: &[(&str, &str, &str)] = &[
    ("dock-position", "'BOTTOM'", "posição inferior"),
    ("dock-fixed", "false", "dock flutuante"),
    ("autohide", "true", "auto-ocultação"),
    (
        "autohide-in-fullscreen",
        "true",
        "auto-ocultação em tela cheia",
    ),
    (
        "click-action",
        "'cycle-windows'",
        "clique para alternar janelas",
    ),
    (
        "shift-click-action",
        "'minimize'",
        "Shift+clique minimiza",
    ),
    (
        "middle-click-action",
        "'launch'",
        "clique do meio abre nova instância",
    ),
    ("dash-max-icon-size", "48", "ícones em 48 px"),
    ("show-trash", "true", "lixeira visível"),
    ("show-mounts", "true", "unidades visíveis"),
    (
        "show-mounts-only-mounted",
        "true",
        "apenas unidades montadas",
    ),
    (
        "show-mounts-network",
        "false",
        "ocultar unidades de rede",
    ),
    ("isolate-locations", "true", "locais isolados"),
    ("show-windows-preview", "true", "prévia das janelas"),
    ("show-icons-emblems", "true", "emblemas dos ícones"),
    (
        "show-icons-notifications-counter",
        "true",
        "contador de notificações",
    ),
    ("show-show-apps-button", "true", "botão de aplicativos"),
    (
        "show-apps-always-in-the-edge",
        "true",
        "botão de aplicativos na borda",
    ),
    ("scroll-action", "'do-nothing'", "rolagem sem ação na dock"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Good,
    Warning,
    Error,
    Unknown,
}

impl Health {
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Good => "object-select-symbolic",
            Self::Warning => "dialog-warning-symbolic",
            Self::Error => "dialog-error-symbolic",
            Self::Unknown => "dialog-question-symbolic",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "OK",
            Self::Warning => "Atenção",
            Self::Error => "Erro",
            Self::Unknown => "Indefinido",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Good => "status-pill-good",
            Self::Warning => "status-pill-warning",
            Self::Error => "status-pill-error",
            Self::Unknown => "status-pill-unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleOrigin {
    Patched,
    InTree,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckItem {
    pub title: &'static str,
    pub detail: String,
    pub health: Health,
    pub code: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSummary {
    pub notebook: String,
    pub fedora: String,
    pub kernel: String,
    pub secure_boot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupSnapshot {
    pub system: SystemSummary,
    pub packages: CheckItem,
    pub akmods: CheckItem,
    pub module: CheckItem,
    pub libcamera: CheckItem,
    pub browser_camera: CheckItem,
    pub boot: CheckItem,
    pub speakers: CheckItem,
    pub sound_app: CheckItem,
    pub fingerprint_reader: CheckItem,
    pub fingerprint_login: CheckItem,
    pub gpu: CheckItem,
    pub secure_boot_key: CheckItem,
    pub platform_profile: CheckItem,
    pub clipboard_extension: CheckItem,
    pub gsconnect_extension: CheckItem,
    pub desktop_icons_extension: CheckItem,
    pub dock_extension: CheckItem,
    pub recommendation_title: String,
    pub recommendation_body: String,
    pub install_main_support_command: String,
    pub install_command: String,
    pub repair_command: String,
    pub enable_camera_module_command: String,
    pub force_camera_command: String,
    pub restore_intel_camera_command: String,
    pub enable_browser_camera_command: String,
    pub enable_speaker_command: String,
    pub prepare_secure_boot_key_command: String,
    pub install_sound_app_command: String,
    pub repair_nvidia_command: String,
    pub set_balanced_profile_command: String,
    pub repair_fingerprint_command: String,
    pub enable_fingerprint_auth_command: String,
    pub open_fingerprint_settings_command: String,
    pub apply_clipboard_profile_command: String,
    pub apply_gsconnect_profile_command: String,
    pub apply_desktop_icons_profile_command: String,
    pub apply_dock_profile_command: String,
    pub reboot_command: String,
    pub camera_app_installed: bool,
    pub sound_app_installed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PackagePresence {
    installed: Vec<String>,
    missing: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FingerprintEnrollmentState {
    Enrolled,
    NotEnrolled,
    Busy,
    NoDevice,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FingerprintContext {
    sensor_line: Option<String>,
    missing_packages: Vec<String>,
    authselect_enabled: bool,
    list_state: FingerprintEnrollmentState,
}

pub fn collect_snapshot() -> SetupSnapshot {
    let system = SystemSummary {
        notebook: detect_notebook(),
        fedora: detect_fedora_release(),
        kernel: command_text("uname", &["-r"]).unwrap_or_else(|_| "Desconhecido".into()),
        secure_boot: detect_secure_boot(),
    };
    let secure_boot_key_check = detect_secure_boot_key_check();

    let packages = package_presence(&[
        "galaxybook-ov02c10-kmod-common",
        "akmod-galaxybook-ov02c10",
        "galaxybook-camera",
    ]);
    let camera_app_installed = packages.installed.iter().any(|pkg| pkg == "galaxybook-camera");

    let package_check = if packages.missing.is_empty() {
        CheckItem {
            title: "Pacotes principais",
            detail: trf(
                "Instalados: {packages}",
                &[("packages", packages.installed.join(", "))],
            ),
            health: Health::Good,
            code: "packages-installed",
        }
    } else {
        CheckItem {
            title: "Pacotes principais",
            detail: trf(
                "Faltando: {packages}",
                &[("packages", packages.missing.join(", "))],
            ),
            health: Health::Warning,
            code: "packages-missing",
        }
    };

    let akmods_log = command_text("journalctl", &["-b", "-u", "akmods", "--no-pager"])
        .unwrap_or_default();
    let akmods_failed = akmods_log.contains("Building and installing galaxybook-ov02c10-kmod [FAILED]")
        || akmods_log.contains("Building rpms failed")
        || akmods_log.contains("galaxybook-ov02c10/") && akmods_log.contains("failed.log");
    let akmods_check = if packages
        .installed
        .iter()
        .any(|pkg| pkg == "akmod-galaxybook-ov02c10")
    {
        if akmods_failed {
            CheckItem {
                title: "Akmods no boot",
                detail: tr("Falhou ao gerar o módulo para o kernel atual."),
                health: Health::Error,
                code: "akmods-failed",
            }
        } else {
            CheckItem {
                title: "Akmods no boot",
                detail: tr("Nenhuma falha do akmods encontrada no boot atual."),
                health: Health::Good,
                code: "akmods-ok",
            }
        }
    } else {
        CheckItem {
            title: "Akmods no boot",
            detail: tr(
                "O driver ainda não foi instalado, então o akmods não executou esse fluxo.",
            ),
            health: Health::Unknown,
            code: "akmods-unavailable",
        }
    };

    let module_path = command_text("modinfo", &["-n", "ov02c10"]).ok();
    let module_origin = module_origin_from_path(module_path.as_deref());
    let module_owner = module_path.as_deref().and_then(rpm_owner_for_file);
    let packaged_camera_driver_installed = packages
        .installed
        .iter()
        .any(|pkg| pkg == "akmod-galaxybook-ov02c10");
    let module_loaded = read_trimmed("/proc/modules")
        .map(|modules| modules.lines().any(|line| line.starts_with("ov02c10 ")))
        .unwrap_or(false);
    let manual_updates_override = module_path
        .as_deref()
        .map(|path| {
            path.contains("/updates/") && module_owner.is_none() && !packaged_camera_driver_installed
        })
        .unwrap_or(false);
    let kernel_log = command_text("journalctl", &["-b", "-k", "--no-pager"]).unwrap_or_default();
    let clock_error = detect_clock_error(&kernel_log);
    let module_check = match (module_origin, module_path.as_deref()) {
        (ModuleOrigin::Patched, Some(path)) if !module_loaded => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "O módulo corrigido existe em {path}, mas ainda não foi carregado no kernel. Habilite o carregamento automático e recarregue o driver pela seção de ações rápidas.",
                &[("path", path.to_string())],
            ),
            health: Health::Error,
            code: "module-not-loaded",
        },
        (ModuleOrigin::Patched, Some(path)) if manual_updates_override => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "Usando um override manual em {path}. Como esse arquivo não pertence a um RPM, ele pode divergir do stack Intel IPU6 que o restante do sistema espera.",
                &[("path", path.to_string())],
            ),
            health: Health::Warning,
            code: "module-manual-override",
        },
        (ModuleOrigin::Patched, Some(path)) => CheckItem {
            title: "Módulo ativo",
            detail: match module_owner {
                Some(ref owner) => trf(
                    "Usando módulo externo em {path}, fornecido por {owner}.",
                    &[("path", path.to_string()), ("owner", owner.to_string())],
                ),
                None => trf("Usando módulo externo: {path}", &[("path", path.to_string())]),
            },
            health: Health::Good,
            code: "module-patched",
        },
        (ModuleOrigin::Patched, None) => CheckItem {
            title: "Módulo ativo",
            detail: tr("O sistema indicou um módulo externo, mas o caminho não pôde ser lido."),
            health: Health::Warning,
            code: "module-patched-path-missing",
        },
        (ModuleOrigin::InTree, Some(path)) => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "O sistema está usando o módulo in-tree do kernel: {path}. Use a ação rápida para ajustar a prioridade do driver corrigido.",
                &[("path", path.to_string())],
            ),
            health: if clock_error {
                Health::Error
            } else {
                Health::Warning
            },
            code: "module-in-tree",
        },
        (ModuleOrigin::InTree, None) => CheckItem {
            title: "Módulo ativo",
            detail: tr(
                "O sistema parece ter caído para o módulo in-tree. Use a ação rápida para ajustar a prioridade do driver corrigido.",
            ),
            health: if clock_error {
                Health::Error
            } else {
                Health::Warning
            },
            code: "module-in-tree",
        },
        (ModuleOrigin::Missing, _) => CheckItem {
            title: "Módulo ativo",
            detail: tr("Não foi possível localizar o módulo ov02c10 via modinfo."),
            health: Health::Error,
            code: "module-missing",
        },
        (ModuleOrigin::Unknown, Some(path)) => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "Módulo localizado, mas sem origem claramente classificada: {path}",
                &[("path", path.to_string())],
            ),
            health: Health::Warning,
            code: "module-unknown",
        },
        (ModuleOrigin::Unknown, None) => CheckItem {
            title: "Módulo ativo",
            detail: tr("Origem do módulo ov02c10 não pôde ser determinada."),
            health: Health::Unknown,
            code: "module-unknown",
        },
    };

    let libcamera_output = direct_camera_command_text("cam", &["-l"]);
    let libcamera_detected = libcamera_output
        .as_ref()
        .map(|output| libcamera_output_has_camera(output))
        .unwrap_or(false);
    let libcamera_permission_blocked = browser_camera_rule_blocks_libcamera();
    let libcamera_check = match libcamera_output {
        Ok(output) if libcamera_detected => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: extract_first_matching_line(&output, &["Internal front camera", "'ov02c10'"])
                .unwrap_or_else(|| {
                    tr(
                        "A câmera interna apareceu no caminho direto usado pelo Galaxy Book Câmera.",
                    )
                }),
            health: Health::Good,
            code: "libcamera-ready",
        },
        Ok(_) => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr("A ferramenta cam executou, mas o caminho direto usado pelo Galaxy Book Câmera não listou a câmera interna. Isso não significa, por si só, que Snapshot, navegador ou o sistema também falharam."),
            health: Health::Warning,
            code: "libcamera-missing",
        },
        Err(_) if libcamera_permission_blocked => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr("O caminho direto do libcamera perdeu acesso aos nós crus do IPU6 porque uma configuração antiga da câmera para navegador removeu o uaccess desses dispositivos. Reaplique a ação rápida da câmera para navegador para migrar o bridge para o fluxo novo, que mantém o libcamera e a webcam virtual funcionando juntos."),
            health: Health::Error,
            code: "libcamera-permission-blocked",
        },
        Err(_) => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr("A ferramenta 'cam' não está disponível ou falhou ao executar, então o setup não conseguiu validar o caminho direto usado pelo Galaxy Book Câmera."),
            health: Health::Unknown,
            code: "libcamera-unavailable",
        },
    };

    let browser_packages = package_presence(&[
        "v4l2-relayd",
        "v4l2loopback",
        "gstreamer1-plugins-icamerasrc",
        "v4l-utils",
    ]);
    let camera_source_ready = detect_system_camera_source_ready();
    let browser_camera = detect_browser_camera_check(
        &browser_packages,
        libcamera_detected,
        camera_source_ready,
    );
    let speakers_check = detect_speakers_check();
    let sound_app_installed = detect_sound_app_installed();
    let sound_app_check = detect_sound_app_check(sound_app_installed);
    let fingerprint = collect_fingerprint_context();
    let fingerprint_reader_check = detect_fingerprint_reader_check(&fingerprint);
    let fingerprint_login_check = detect_fingerprint_login_check(&fingerprint);

    let boot_check = if clock_error {
        CheckItem {
            title: "Erros no boot",
            detail: tr(
                "O boot registrou que o driver in-tree não suporta o clock externo de 26 MHz nesta máquina.",
            ),
            health: Health::Error,
            code: "boot-clock-error",
        }
    } else {
        CheckItem {
            title: "Erros no boot",
            detail: tr("Nenhum erro de clock/probe do ov02c10 foi encontrado no boot atual."),
            health: Health::Good,
            code: "boot-ok",
        }
    };

    let gpu_check = detect_nvidia_check();
    let platform_profile_check = detect_platform_profile_check();

    let enabled_extensions = enabled_gnome_shell_extensions();
    let installed_extensions = installed_gnome_shell_extensions();

    let clipboard_check = extension_check(
        "Histórico da área de transferência",
        CLIPBOARD_EXTENSION_IDS,
        &enabled_extensions,
        &installed_extensions,
        "Nenhuma extensão conhecida de histórico da área de transferência foi encontrada.",
    );
    let gsconnect_check = extension_check(
        "GSConnect",
        &[GSCONNECT_EXTENSION_ID],
        &enabled_extensions,
        &installed_extensions,
        "O GSConnect não está instalado.",
    );
    let desktop_icons_check = extension_check(
        "Ícones na área de trabalho",
        &[DESKTOP_ICONS_EXTENSION_ID],
        &enabled_extensions,
        &installed_extensions,
        "A extensão Desktop Icons NG não está instalada.",
    );
    let dock_check = detect_dash_to_dock_check(
        &enabled_extensions,
        &installed_extensions,
    );

    let (recommendation_title, recommendation_body) = recommend_next_step(
        &secure_boot_key_check,
        &packages,
        akmods_failed,
        module_origin,
        manual_updates_override,
        clock_error,
        module_loaded,
        libcamera_detected,
        libcamera_permission_blocked,
        camera_source_ready,
        browser_camera.health == Health::Good,
        camera_app_installed,
        speakers_check.health != Health::Unknown,
        speakers_check.health == Health::Good,
        sound_app_installed,
    );

    SetupSnapshot {
        system,
        packages: package_check,
        akmods: akmods_check,
        module: module_check,
        libcamera: libcamera_check,
        browser_camera,
        boot: boot_check,
        speakers: speakers_check,
        sound_app: sound_app_check,
        fingerprint_reader: fingerprint_reader_check,
        fingerprint_login: fingerprint_login_check,
        gpu: gpu_check,
        secure_boot_key: secure_boot_key_check,
        platform_profile: platform_profile_check,
        clipboard_extension: clipboard_check,
        gsconnect_extension: gsconnect_check,
        desktop_icons_extension: desktop_icons_check,
        dock_extension: dock_check,
        recommendation_title,
        recommendation_body,
        install_main_support_command: INSTALL_MAIN_SUPPORT_COMMAND.into(),
        install_command: INSTALL_CAMERA_COMMAND.into(),
        repair_command: REPAIR_CAMERA_COMMAND.into(),
        enable_camera_module_command: ENABLE_CAMERA_MODULE_COMMAND.into(),
        force_camera_command: FORCE_CAMERA_DRIVER_COMMAND.into(),
        restore_intel_camera_command: RESTORE_INTEL_CAMERA_COMMAND.into(),
        enable_browser_camera_command: ENABLE_BROWSER_CAMERA_COMMAND.into(),
        enable_speaker_command: ENABLE_SPEAKER_COMMAND.into(),
        prepare_secure_boot_key_command: PREPARE_SECURE_BOOT_KEY_COMMAND.into(),
        install_sound_app_command: INSTALL_SOUND_APP_COMMAND.into(),
        repair_nvidia_command: REPAIR_NVIDIA_COMMAND.into(),
        set_balanced_profile_command: SET_BALANCED_PROFILE_COMMAND.into(),
        repair_fingerprint_command: REPAIR_FINGERPRINT_COMMAND.into(),
        enable_fingerprint_auth_command: ENABLE_FINGERPRINT_AUTH_COMMAND.into(),
        open_fingerprint_settings_command: OPEN_FINGERPRINT_SETTINGS_COMMAND.into(),
        apply_clipboard_profile_command: build_clipboard_profile_command(),
        apply_gsconnect_profile_command: build_gsconnect_profile_command(),
        apply_desktop_icons_profile_command: build_desktop_icons_profile_command(),
        apply_dock_profile_command: build_dash_to_dock_profile_command(),
        reboot_command: REBOOT_COMMAND.into(),
        camera_app_installed,
        sound_app_installed,
    }
}

pub fn run_smoke_test() -> Result<(), String> {
    let snapshot = collect_snapshot();

    println!("app_id={APP_ID}");
    println!("app_name={APP_NAME}");
    println!("notebook={}", snapshot.system.notebook);
    println!("fedora={}", snapshot.system.fedora);
    println!("kernel={}", snapshot.system.kernel);
    println!("secure_boot={}", snapshot.system.secure_boot);
    println!(
        "checks={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        snapshot.packages.health.icon_name(),
        snapshot.akmods.health.icon_name(),
        snapshot.module.health.icon_name(),
        snapshot.libcamera.health.icon_name(),
        snapshot.browser_camera.health.icon_name(),
        snapshot.boot.health.icon_name(),
        snapshot.speakers.health.icon_name(),
        snapshot.sound_app.health.icon_name(),
        snapshot.fingerprint_reader.health.icon_name(),
        snapshot.fingerprint_login.health.icon_name(),
        snapshot.gpu.health.icon_name(),
        snapshot.secure_boot_key.health.icon_name(),
        snapshot.platform_profile.health.icon_name(),
        snapshot.clipboard_extension.health.icon_name(),
        snapshot.gsconnect_extension.health.icon_name(),
        snapshot.desktop_icons_extension.health.icon_name(),
        snapshot.dock_extension.health.icon_name()
    );
    println!("recommendation_title={}", snapshot.recommendation_title);
    println!("camera_app_installed={}", snapshot.camera_app_installed);
    println!("sound_app_installed={}", snapshot.sound_app_installed);

    if snapshot.system.kernel.trim().is_empty() {
        return Err(tr("Kernel não pode estar vazio no smoke test."));
    }

    Ok(())
}

fn detect_notebook() -> String {
    let vendor = read_trimmed("/sys/devices/virtual/dmi/id/sys_vendor");
    let product = read_trimmed("/sys/devices/virtual/dmi/id/product_name");

    match (vendor, product) {
        (Some(vendor), Some(product)) => format!("{vendor} {product}"),
        (None, Some(product)) => product,
        (Some(vendor), None) => vendor,
        (None, None) => tr("Galaxy Book (modelo não identificado)"),
    }
}

fn detect_fedora_release() -> String {
    read_trimmed("/etc/fedora-release")
        .unwrap_or_else(|| tr("Fedora (versão não identificada)"))
}

fn detect_secure_boot() -> String {
    match command_text("mokutil", &["--sb-state"]) {
        Ok(output) => parse_secure_boot(&output).into(),
        Err(_) => tr("Não foi possível verificar"),
    }
}

fn secure_boot_enabled() -> Option<bool> {
    let output = command_text("mokutil", &["--sb-state"]).ok()?;
    match parse_secure_boot(&output) {
        "Ativado" => Some(true),
        "Desativado" => Some(false),
        _ => None,
    }
}

fn package_presence(packages: &[&str]) -> PackagePresence {
    let mut status = PackagePresence::default();

    for package in packages {
        match Command::new("rpm").args(["-q", package]).output() {
            Ok(output) if output.status.success() => status.installed.push((*package).into()),
            _ => status.missing.push((*package).into()),
        }
    }

    status
}

fn enabled_gnome_shell_extensions() -> Vec<String> {
    command_text("gsettings", &["get", "org.gnome.shell", "enabled-extensions"])
        .map(|output| parse_gsettings_string_array(&output))
        .unwrap_or_default()
}

fn installed_gnome_shell_extensions() -> Vec<String> {
    let mut installed = Vec::new();
    let mut bases = vec!["/usr/share/gnome-shell/extensions".to_string()];
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(format!(
            "{}/.local/share/gnome-shell/extensions",
            home.to_string_lossy()
        ));
    }
    for base in bases {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        installed.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    installed.sort();
    installed.dedup();
    installed
}

fn parse_gsettings_string_array(output: &str) -> Vec<String> {
    output
        .split('\'')
        .skip(1)
        .step_by(2)
        .filter(|item| !item.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extension_check(
    title: &'static str,
    ids: &[&str],
    enabled_extensions: &[String],
    installed_extensions: &[String],
    missing_detail: &str,
) -> CheckItem {
    let enabled: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| enabled_extensions.iter().any(|enabled| enabled == id))
        .collect();
    if !enabled.is_empty() {
        return CheckItem {
            title,
            detail: trf("Ativa: {extensions}", &[("extensions", enabled.join(", "))]),
            health: Health::Good,
            code: "extension-enabled",
        };
    }

    let installed: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| installed_extensions.iter().any(|installed| installed == id))
        .collect();
    if !installed.is_empty() {
        return CheckItem {
            title,
            detail: trf(
                "Instalada, mas desativada: {extensions}",
                &[("extensions", installed.join(", "))],
            ),
            health: Health::Warning,
            code: "extension-installed-disabled",
        };
    }

    CheckItem {
        title,
        detail: tr(missing_detail),
        health: Health::Unknown,
        code: "extension-missing",
    }
}

fn gsettings_value(schema: &str, key: &str) -> Option<String> {
    command_text("gsettings", &["get", schema, key]).ok()
}

fn dash_to_dock_profile_mismatches() -> Vec<&'static str> {
    DASH_TO_DOCK_PROFILE_SETTINGS
        .iter()
        .filter_map(|(key, expected, label)| {
            match gsettings_value(DASH_TO_DOCK_SCHEMA, key) {
                Some(current) if current == *expected => None,
                Some(_) | None => Some(*label),
            }
        })
        .collect()
}

fn dash_to_dock_check_from_state(
    enabled: bool,
    installed: bool,
    mismatches: &[&str],
) -> CheckItem {
    if enabled && mismatches.is_empty() {
        return CheckItem {
            title: "Dock do GNOME",
            detail: tr(
                "Dash to Dock ativo com dock inferior auto-ocultável, clique ciclando janelas, ícones em 48 px, lixeira e unidades montadas visíveis.",
            ),
            health: Health::Good,
            code: "dash-to-dock-ready",
        };
    }

    if enabled {
        return CheckItem {
            title: "Dock do GNOME",
            detail: trf(
                "Dash to Dock ativo, mas fora do perfil recomendado em: {items}",
                &[("items", mismatches.join(", "))],
            ),
            health: Health::Warning,
            code: "dash-to-dock-mismatch",
        };
    }

    if installed {
        return CheckItem {
            title: "Dock do GNOME",
            detail: trf(
                "Instalada, mas desativada: {extension}",
                &[("extension", DASH_TO_DOCK_EXTENSION_ID.to_string())],
            ),
            health: Health::Warning,
            code: "dash-to-dock-disabled",
        };
    }

    CheckItem {
        title: "Dock do GNOME",
        detail: tr("A extensão Dash to Dock não está instalada."),
        health: Health::Unknown,
        code: "dash-to-dock-missing",
    }
}

fn detect_dash_to_dock_check(
    enabled_extensions: &[String],
    installed_extensions: &[String],
) -> CheckItem {
    let enabled = enabled_extensions
        .iter()
        .any(|id| id == DASH_TO_DOCK_EXTENSION_ID);
    let installed = installed_extensions
        .iter()
        .any(|id| id == DASH_TO_DOCK_EXTENSION_ID);
    let mismatches = if enabled {
        dash_to_dock_profile_mismatches()
    } else {
        Vec::new()
    };

    dash_to_dock_check_from_state(enabled, installed, &mismatches)
}

fn build_gnome_extension_profile_command(
    extension_id: &str,
    prelude_commands: &[&str],
) -> String {
    let mut command = String::from("set -euo pipefail\n\n");

    for prelude in prelude_commands {
        command.push_str(prelude);
        command.push('\n');
    }

    command.push_str(&format!(
        r#"
if ! command -v gnome-extensions >/dev/null 2>&1; then
  echo "O utilitário gnome-extensions não está disponível neste sistema." >&2
  exit 1
fi

python3 - '{extension_id}' <<'PY'
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request

uuid = sys.argv[1]
shell_version = "50"

try:
    shell_output = subprocess.check_output(
        ["gnome-shell", "--version"],
        text=True,
    ).strip()
    match = re.search(r"(\d+)", shell_output)
    if match:
        shell_version = match.group(1)
except Exception:
    pass

info_url = (
    "https://extensions.gnome.org/extension-info/"
    "?uuid={{uuid}}&shell_version={{shell_version}}"
).format(
    uuid=urllib.parse.quote(uuid),
    shell_version=urllib.parse.quote(shell_version),
)

with urllib.request.urlopen(info_url, timeout=20) as response:
    data = json.load(response)

download_url = urllib.parse.urljoin(
    "https://extensions.gnome.org",
    data["download_url"],
)

fd, archive_path = tempfile.mkstemp(suffix=".shell-extension.zip")
os.close(fd)

try:
    urllib.request.urlretrieve(download_url, archive_path)
    subprocess.check_call(
        ["gnome-extensions", "install", "--force", archive_path]
    )
finally:
    try:
        os.unlink(archive_path)
    except FileNotFoundError:
        pass
PY

gnome-extensions enable '{extension_id}'
gnome-extensions info '{extension_id}' || true
"#,
        extension_id = extension_id,
    ));

    command
}

fn build_clipboard_profile_command() -> String {
    build_gnome_extension_profile_command(CLIPBOARD_PROFILE_EXTENSION_ID, &[])
}

fn build_gsconnect_profile_command() -> String {
    build_gnome_extension_profile_command(
        GSCONNECT_EXTENSION_ID,
        &[r#"pkexec sh -lc 'dnf install -y nautilus-python || true'"#],
    )
}

fn build_desktop_icons_profile_command() -> String {
    build_gnome_extension_profile_command(DESKTOP_ICONS_EXTENSION_ID, &[])
}

fn build_dash_to_dock_profile_command() -> String {
    let mut command = format!(
        r#"set -euo pipefail
schema="{schema}"
extension_id="{extension_id}"

if ! rpm -q gnome-shell-extension-dash-to-dock >/dev/null 2>&1; then
  pkexec sh -lc 'dnf install -y gnome-shell-extension-dash-to-dock'
fi

if ! gsettings list-schemas | grep -qx "$schema"; then
  echo "A extensão Dash to Dock não está instalada neste sistema." >&2
  exit 1
fi

if command -v gnome-extensions >/dev/null 2>&1; then
  gnome-extensions enable "$extension_id" >/dev/null 2>&1 || true
fi

"#,
        schema = DASH_TO_DOCK_SCHEMA,
        extension_id = DASH_TO_DOCK_EXTENSION_ID
    );

    for (key, value, _) in DASH_TO_DOCK_PROFILE_SETTINGS {
        command.push_str(&format!("gsettings set \"$schema\" {key} {value}\n"));
    }

    command.push_str(
        r#"gsettings get "$schema" dock-position
gsettings get "$schema" autohide
gsettings get "$schema" click-action
gsettings get "$schema" show-trash
gsettings get "$schema" show-mounts
"#,
    );

    command
}

fn detect_nvidia_check() -> CheckItem {
    let akmod_installed = rpm_installed("akmod-nvidia");
    let modules_loaded = read_trimmed("/proc/modules")
        .map(|modules| modules.lines().any(|line| line.starts_with("nvidia ")))
        .unwrap_or(false);
    let smi_installed = rpm_installed("xorg-x11-drv-nvidia-cuda");

    if modules_loaded {
        let detail = if smi_installed {
            tr("Módulos NVIDIA carregados. O utilitário nvidia-smi também está instalado.")
        } else {
            tr(
                "Módulos NVIDIA carregados. O utilitário nvidia-smi continua opcional e não está instalado.",
            )
        };
        return CheckItem {
            title: "Driver NVIDIA",
            detail,
            health: Health::Good,
            code: "nvidia-ready",
        };
    }

    if akmod_installed {
        return CheckItem {
            title: "Driver NVIDIA",
            detail: tr(
                "O pacote akmod-nvidia está instalado, mas os módulos não estão carregados para o kernel atual.",
            ),
            health: Health::Warning,
            code: "nvidia-akmod-installed",
        };
    }

    CheckItem {
        title: "Driver NVIDIA",
        detail: tr(
            "O suporte NVIDIA ainda não foi instalado. O setup trata o akmod-nvidia como o pacote principal para esta etapa.",
        ),
        health: Health::Unknown,
        code: "nvidia-missing",
    }
}

fn detect_secure_boot_key_check() -> CheckItem {
    let title = "Chave do Secure Boot";

    let Some(secure_boot_enabled) = secure_boot_enabled() else {
        return CheckItem {
            title,
            detail: tr("O setup não conseguiu consultar o estado do Secure Boot/MOK neste sistema."),
            health: Health::Unknown,
            code: "mok-unavailable",
        };
    };

    if !secure_boot_enabled {
        return CheckItem {
            title,
            detail: tr("Secure Boot está desativado. O MOK do akmods não é necessário para carregar módulos externos neste boot."),
            health: Health::Good,
            code: "mok-not-needed",
        };
    }

    let public_key_exists = Path::new(AKMODS_PUBLIC_KEY_PATH).is_file();
    let private_key_exists = Path::new(AKMODS_PRIVATE_KEY_PATH).is_file();
    if !public_key_exists || !private_key_exists {
        return CheckItem {
            title,
            detail: tr("Secure Boot está ativo, mas o akmods ainda não tem a chave local pronta em /etc/pki/akmods. Prepare a chave do Secure Boot antes de instalar ou carregar módulos externos."),
            health: Health::Error,
            code: "mok-key-missing",
        };
    }

    let enrolled_output = Command::new("mokutil")
        .arg("--list-enrolled")
        .output()
        .ok()
        .map(|output| command_output_text(&output));
    if let (Some(key_cn), Some(output)) = (current_akmods_key_cn(), enrolled_output.as_deref()) {
        if mok_enrolled_list_contains_key(output, &key_cn) {
            return CheckItem {
                title,
                detail: tr("A chave pública do akmods já está inscrita no MOK. Os módulos externos podem ser assinados e aceitos pelo Secure Boot neste host."),
                health: Health::Good,
                code: "mok-enrolled",
            };
        }
    }

    let test_output = Command::new("mokutil")
        .args(["--test-key", AKMODS_PUBLIC_KEY_PATH])
        .output()
        .ok();
    if matches!(
        test_output.as_ref(),
        Some(output)
            if mokutil_test_key_reports_enrolled(
                output.status.success(),
                &command_output_text(output),
            )
    ) {
        return CheckItem {
            title,
            detail: tr("A chave pública do akmods já está inscrita no MOK. Os módulos externos podem ser assinados e aceitos pelo Secure Boot neste host."),
            health: Health::Good,
            code: "mok-enrolled",
        };
    }

    if mok_pending_request_exists() {
        return CheckItem {
            title,
            detail: tr("A chave do akmods já foi preparada e o pedido de importação está pendente no MOK. Reinicie o notebook e conclua 'Enroll MOK' na tela azul do boot."),
            health: Health::Warning,
            code: "mok-pending-enrollment",
        };
    }

    CheckItem {
        title,
        detail: tr("Secure Boot está ativo e a chave local do akmods existe, mas ainda não foi aceita no MOK. Use a ação rápida dedicada antes de tentar carregar módulos externos."),
        health: Health::Error,
        code: "mok-not-enrolled",
    }
}

fn detect_platform_profile_check() -> CheckItem {
    let current = read_trimmed("/sys/firmware/acpi/platform_profile");
    let choices = read_trimmed("/sys/firmware/acpi/platform_profile_choices");

    match (current, choices) {
        (Some(current), Some(choices)) if current == "balanced" => CheckItem {
            title: "Perfil de uso",
            detail: trf(
                "Ativo: balanced. Perfil recomendado para uso geral, equilibrando ruído da ventoinha, temperatura e desempenho. Opções disponíveis: {choices}",
                &[("choices", choices)],
            ),
            health: Health::Good,
            code: "platform-balanced",
        },
        (Some(current), Some(choices)) => CheckItem {
            title: "Perfil de uso",
            detail: trf(
                "Ativo: {current}. Para uso geral, o perfil balanced costuma ser o ponto mais estável entre ventoinha, temperatura e desempenho. Opções disponíveis: {choices}",
                &[("current", current), ("choices", choices)],
            ),
            health: Health::Warning,
            code: "platform-nonbalanced",
        },
        (Some(current), None) if current == "balanced" => CheckItem {
            title: "Perfil de uso",
            detail: tr(
                "Ativo: balanced. Perfil recomendado para uso geral, equilibrando ventoinha, temperatura e desempenho.",
            ),
            health: Health::Good,
            code: "platform-balanced",
        },
        (Some(current), None) => CheckItem {
            title: "Perfil de uso",
            detail: trf(
                "Ativo: {current}. O perfil balanced é o recomendado para uso geral neste notebook.",
                &[("current", current)],
            ),
            health: Health::Warning,
            code: "platform-nonbalanced",
        },
        _ => CheckItem {
            title: "Perfil de uso",
            detail: tr("Este sistema não expôs a interface ACPI de platform_profile."),
            health: Health::Unknown,
            code: "platform-unavailable",
        },
    }
}

fn detect_browser_camera_check(
    packages: &PackagePresence,
    libcamera_detected: bool,
    camera_source_ready: bool,
) -> CheckItem {
    if !packages.missing.is_empty() {
        return CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Faltando pacotes do bridge V4L2: {packages}",
                &[("packages", packages.missing.join(", "))],
            ),
            health: Health::Warning,
            code: "browser-packages-missing",
        };
    }

    let relay_active = systemd_unit_is_active("v4l2-relayd@icamerasrc.service");
    let relay_enabled = systemd_unit_enabled_state("v4l2-relayd@icamerasrc.service");
    let loopback_device = find_virtual_video_device_by_name("Intel MIPI Camera");
    let loopback_capture = loopback_device
        .as_deref()
        .map(v4l2_device_supports_capture)
        .unwrap_or(false);

    match (relay_active, relay_enabled.as_deref(), loopback_device.as_deref(), loopback_capture) {
        (true, Some("enabled"), Some(device), true) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Bridge V4L2 ativo em {device}. A webcam virtual já pode ser usada por Meet, Discord, Teams e outros apps.",
                &[("device", device.to_string())],
            ),
            health: Health::Good,
            code: "browser-ready",
        },
        (true, Some("enabled-runtime"), Some(device), true) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Bridge ativo em {device}, mas só habilitado para a sessão atual. Ative novamente pela ação rápida para persistir após reboot.",
                &[("device", device.to_string())],
            ),
            health: Health::Warning,
            code: "browser-runtime-only",
        },
        (true, _, Some(device), true) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Bridge ativo em {device}, mas o serviço ainda não está habilitado de forma persistente. Ative a câmera para navegador pela seção de ações rápidas.",
                &[("device", device.to_string())],
            ),
            health: Health::Warning,
            code: "browser-service-not-persistent",
        },
        (false, _, Some(device), true) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "A webcam virtual existe em {device}, mas o relay está parado. Ative a câmera para navegador para manter Meet, Discord e outros apps funcionando de forma previsível.",
                &[("device", device.to_string())],
            ),
            health: Health::Warning,
            code: "browser-relay-stopped",
        },
        (true, _, Some(device), false) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "O relay está ativo, mas {device} ainda não expôs um nó de captura utilizável. Reaplique a ação rápida da câmera para navegador.",
                &[("device", device.to_string())],
            ),
            health: Health::Warning,
            code: "browser-device-not-capture",
        },
        _ if camera_source_ready => CheckItem {
            title: "Navegador e comunicadores",
            detail: tr("A câmera já aparece nas fontes do sistema, mas a webcam virtual ainda não foi ativada. Use a ação rápida para expor a câmera como dispositivo V4L2 para Meet, Discord, Teams e outros apps WebRTC."),
            health: Health::Warning,
            code: "browser-system-source-ready",
        },
        _ if libcamera_detected => CheckItem {
            title: "Navegador e comunicadores",
            detail: tr("A câmera base já está funcional no libcamera, mas o bridge V4L2 para navegador ainda não foi ativado."),
            health: Health::Warning,
            code: "browser-libcamera-ready",
        },
        _ => CheckItem {
            title: "Navegador e comunicadores",
            detail: tr("O bridge V4L2 para navegador ainda não foi ativado e a câmera também não apareceu nas fontes do sistema. Use a ação rápida para configurar a webcam virtual e reavalie o estado da câmera base se o problema persistir."),
            health: Health::Warning,
            code: "browser-missing",
        },
    }
}

fn detect_speakers_check() -> CheckItem {
    let max98390_present = has_max98390_device();
    if !max98390_present {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: tr("Este sistema não expôs amplificadores MAX98390 via ACPI ou I2C, então o setup não aplicou o fluxo específico dos alto-falantes Galaxy Book."),
            health: Health::Unknown,
            code: "speakers-unsupported",
        };
    }

    let packages = package_presence(&[
        "galaxybook-max98390-kmod-common",
        "akmod-galaxybook-max98390",
    ]);
    let modules = read_trimmed("/proc/modules").unwrap_or_default();
    let core_loaded = modules
        .lines()
        .any(|line| line.starts_with("snd_hda_scodec_max98390 "));
    let i2c_loaded = modules
        .lines()
        .any(|line| line.starts_with("snd_hda_scodec_max98390_i2c "));
    let core_module_path = command_text("modinfo", &["-n", "snd-hda-scodec-max98390"]).ok();
    let i2c_module_path = command_text("modinfo", &["-n", "snd-hda-scodec-max98390-i2c"]).ok();
    let modules_indexed = core_module_path.is_some() && i2c_module_path.is_some();
    let modules_load_failure = speaker_modules_missing_in_boot();
    let setup_service = "max98390-hda-i2c-setup.service";
    let setup_active = systemd_unit_is_active(setup_service);
    let setup_enabled = systemd_unit_enabled_state(setup_service)
        .map(|state| state.starts_with("enabled"))
        .unwrap_or(false);

    if !packages.missing.is_empty() {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "O hardware MAX98390 foi detectado, mas ainda faltam pacotes do suporte de speakers: {packages}",
                &[("packages", packages.missing.join(", "))],
            ),
            health: Health::Warning,
            code: "speakers-packages-missing",
        };
    }

    if !modules_indexed {
        let detail = if modules_load_failure {
            tr("O suporte MAX98390 foi instalado, mas o kernel atual ainda não expõe os módulos snd-hda-scodec-max98390 e snd-hda-scodec-max98390-i2c. O boot já registrou falha ao procurar esses módulos, então o próximo passo é reconstruir e instalar manualmente o driver pela seção de ações rápidas.")
        } else {
            tr("O suporte MAX98390 foi instalado, mas o kernel atual ainda não expõe os módulos snd-hda-scodec-max98390 e snd-hda-scodec-max98390-i2c via modinfo. Reconstrua e instale manualmente o driver pela seção de ações rápidas antes de testar a saída Speaker novamente.")
        };
        return CheckItem {
            title: "Alto-falantes internos",
            detail,
            health: Health::Error,
            code: "speakers-modules-missing",
        };
    }

    if core_loaded && i2c_loaded && (setup_active || setup_enabled) {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: tr("O suporte MAX98390 está instalado, os módulos dos amplificadores estão carregados e o serviço de I2C está pronto para o boot."),
            health: Health::Good,
            code: "speakers-ready",
        };
    }

    if core_loaded && i2c_loaded {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: tr("Os módulos MAX98390 estão carregados, mas o serviço que cria os dispositivos I2C no boot ainda não está ativo de forma persistente."),
            health: Health::Warning,
            code: "speakers-service-disabled",
        };
    }

    CheckItem {
        title: "Alto-falantes internos",
        detail: tr("O hardware MAX98390 foi detectado, mas os módulos dos amplificadores ainda não estão carregados. Ative o suporte dos alto-falantes pela seção de ações rápidas."),
        health: Health::Warning,
        code: "speakers-modules-unloaded",
    }
}

fn speaker_modules_missing_in_boot() -> bool {
    command_text("journalctl", &["-b", "--no-pager", "-u", "systemd-modules-load"])
        .map(|output| {
            output.contains("Failed to find module 'snd_hda_scodec_max98390'")
                || output.contains("Failed to find module 'snd_hda_scodec_max98390_i2c'")
        })
        .unwrap_or(false)
}

fn module_origin_from_path(path: Option<&str>) -> ModuleOrigin {
    match path {
        Some(path) if path.contains("/extra/") || path.contains("/updates/") => ModuleOrigin::Patched,
        Some(path) if path.contains("/kernel/") => ModuleOrigin::InTree,
        Some(_) => ModuleOrigin::Unknown,
        None => ModuleOrigin::Missing,
    }
}

fn detect_clock_error(kernel_log: &str) -> bool {
    kernel_log.contains("external clock 26000000 is not supported")
        || kernel_log.contains("probe with driver ov02c10 failed with error -22")
}

fn has_max98390_device() -> bool {
    fs::read_dir("/sys/bus/acpi/devices")
        .ok()
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("MAX98390:")
            })
        })
        .unwrap_or(false)
        || fs::read_dir("/sys/bus/i2c/devices")
            .ok()
            .map(|entries| {
                entries.flatten().any(|entry| {
                    fs::read_to_string(entry.path().join("name"))
                        .map(|name| name.contains("MAX98390"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
}

fn detect_sound_app_check(sound_app_installed: bool) -> CheckItem {
    match sound_app_installed {
        true => CheckItem {
            title: "Galaxy Book Sound",
            detail: tr("O painel de som está instalado e pronto para equalizador, perfis e Atmos compatível via PipeWire."),
            health: Health::Good,
            code: "sound-app-ready",
        },
        false => CheckItem {
            title: "Galaxy Book Sound",
            detail: tr("O painel de som ainda não está instalado. Use-o para equalizador, perfis prontos e Atmos compatível depois que o MAX98390 estiver funcional."),
            health: Health::Unknown,
            code: "sound-app-missing",
        },
    }
}

fn collect_fingerprint_context() -> FingerprintContext {
    let packages = package_presence(&["fprintd", "libfprint"]);
    let sensor_line = detect_fingerprint_sensor_line();
    let authselect_output = command_text("authselect", &["current"]).ok();
    let authselect_enabled = authselect_output
        .as_deref()
        .map(authselect_has_fingerprint)
        .unwrap_or(false);
    let list_state = current_user_name()
        .and_then(|user| command_text("fprintd-list", &[user.as_str()]).ok())
        .map(|output| fingerprint_enrollment_state(&output))
        .unwrap_or(FingerprintEnrollmentState::Unavailable);

    FingerprintContext {
        sensor_line,
        missing_packages: packages.missing,
        authselect_enabled,
        list_state,
    }
}

fn detect_fingerprint_reader_check(context: &FingerprintContext) -> CheckItem {
    let Some(sensor_line) = context.sensor_line.as_ref() else {
        return CheckItem {
            title: "Leitor de digital",
            detail: tr("Nenhum leitor de digital compatível foi detectado via lsusb neste boot. Em modelos sem sensor integrado, esse estado é esperado."),
            health: Health::Unknown,
            code: "fingerprint-reader-missing",
        };
    };

    if !context.missing_packages.is_empty() {
        return CheckItem {
            title: "Leitor de digital",
            detail: trf(
                "Sensor detectado em {sensor}, mas o stack ainda está incompleto. Faltam pacotes: {packages}",
                &[
                    ("sensor", sensor_line.to_string()),
                    ("packages", context.missing_packages.join(", ")),
                ],
            ),
            health: Health::Warning,
            code: "fingerprint-stack-missing",
        };
    }

    let (detail, health, code) = match context.list_state {
        FingerprintEnrollmentState::Enrolled | FingerprintEnrollmentState::NotEnrolled => (
            trf(
                "Sensor detectado em {sensor}. O stack fprintd/libfprint respondeu normalmente para o usuário atual.",
                &[("sensor", sensor_line.to_string())],
            ),
            Health::Good,
            "fingerprint-reader-ready",
        ),
        FingerprintEnrollmentState::Busy => (
            trf(
                "Sensor detectado em {sensor}, mas o leitor respondeu como ocupado. Reaplique o stack antes de tentar um novo cadastro.",
                &[("sensor", sensor_line.to_string())],
            ),
            Health::Warning,
            "fingerprint-reader-busy",
        ),
        FingerprintEnrollmentState::NoDevice => (
            trf(
                "Sensor detectado em {sensor}, mas o fprintd não expôs um dispositivo utilizável nesta sessão.",
                &[("sensor", sensor_line.to_string())],
            ),
            Health::Warning,
            "fingerprint-reader-no-device",
        ),
        FingerprintEnrollmentState::Unavailable => (
            trf(
                "Sensor detectado em {sensor}, mas o setup não conseguiu validar a resposta do fprintd nesta sessão.",
                &[("sensor", sensor_line.to_string())],
            ),
            Health::Warning,
            "fingerprint-reader-unavailable",
        ),
    };

    CheckItem {
        title: "Leitor de digital",
        detail,
        health,
        code,
    }
}

fn detect_fingerprint_login_check(context: &FingerprintContext) -> CheckItem {
    if context.sensor_line.is_none() {
        return CheckItem {
            title: "Login por digital",
            detail: tr("Sem leitor de digital detectado neste boot, o setup não avaliou cadastro nem integração com authselect."),
            health: Health::Unknown,
            code: "fingerprint-login-unavailable",
        };
    }

    if !context.missing_packages.is_empty() {
        return CheckItem {
            title: "Login por digital",
            detail: tr("O leitor foi detectado, mas o stack de fingerprint ainda não está completo. Reinstale fprintd/libfprint antes de validar cadastro e login."),
            health: Health::Warning,
            code: "fingerprint-login-stack-missing",
        };
    }

    match (context.authselect_enabled, context.list_state) {
        (true, FingerprintEnrollmentState::Enrolled) => CheckItem {
            title: "Login por digital",
            detail: tr("Authselect já está com with-fingerprint ativo e o usuário atual tem pelo menos uma digital cadastrada."),
            health: Health::Good,
            code: "fingerprint-login-ready",
        },
        (true, FingerprintEnrollmentState::NotEnrolled) => CheckItem {
            title: "Login por digital",
            detail: tr("O stack já responde e o authselect está pronto, mas o usuário atual ainda não cadastrou nenhuma digital."),
            health: Health::Warning,
            code: "fingerprint-enrollment-missing",
        },
        (false, FingerprintEnrollmentState::Enrolled) => CheckItem {
            title: "Login por digital",
            detail: tr("Há digital cadastrada para o usuário atual, mas o authselect ainda não está com with-fingerprint habilitado."),
            health: Health::Warning,
            code: "fingerprint-auth-disabled",
        },
        (false, FingerprintEnrollmentState::NotEnrolled) => CheckItem {
            title: "Login por digital",
            detail: tr("O leitor já responde, mas ainda faltam dois passos: habilitar with-fingerprint no authselect e cadastrar a digital do usuário atual."),
            health: Health::Warning,
            code: "fingerprint-auth-and-enrollment-missing",
        },
        (_, FingerprintEnrollmentState::Busy) => CheckItem {
            title: "Login por digital",
            detail: tr("O leitor respondeu como ocupado. Reaplique o stack e depois abra o cadastro de usuários para refazer a digital com o sensor liberado."),
            health: Health::Warning,
            code: "fingerprint-login-busy",
        },
        (_, FingerprintEnrollmentState::NoDevice) => CheckItem {
            title: "Login por digital",
            detail: tr("O sensor apareceu no USB, mas o fprintd não expôs um dispositivo utilizável para validar cadastro e autenticação nesta sessão."),
            health: Health::Warning,
            code: "fingerprint-login-no-device",
        },
        (_, FingerprintEnrollmentState::Unavailable) => CheckItem {
            title: "Login por digital",
            detail: tr("O leitor foi detectado, mas o setup não conseguiu validar cadastro e authselect nesta sessão."),
            health: Health::Unknown,
            code: "fingerprint-login-unavailable",
        },
    }
}

fn recommend_next_step(
    secure_boot_key: &CheckItem,
    packages: &PackagePresence,
    akmods_failed: bool,
    module_origin: ModuleOrigin,
    manual_updates_override: bool,
    clock_error: bool,
    module_loaded: bool,
    libcamera_detected: bool,
    libcamera_permission_blocked: bool,
    camera_source_ready: bool,
    browser_camera_ready: bool,
    camera_app_installed: bool,
    speaker_supported: bool,
    speaker_ready: bool,
    sound_app_installed: bool,
) -> (String, String) {
    if matches!(secure_boot_key.code, "mok-key-missing" | "mok-not-enrolled") {
        return (
            tr("Prepare a chave do Secure Boot"),
            tr("O Secure Boot está ativo, mas a chave usada pelo akmods ainda não está pronta ou inscrita no MOK. Use a ação rápida dedicada antes de instalar ou carregar módulos externos do notebook."),
        );
    }

    if secure_boot_key.code == "mok-pending-enrollment" {
        return (
            tr("Concluir inscrição do MOK"),
            tr("O pedido de importação da chave do akmods já foi criado. Reinicie o notebook e conclua 'Enroll MOK' na tela azul do boot antes de repetir as ações do driver."),
        );
    }

    if !packages.missing.is_empty() {
        return (
            tr("Instalação pendente"),
            tr("Instale os pacotes principais da câmera pela própria seção de ações rápidas, reinicie o sistema e atualize o diagnóstico."),
        );
    }

    if akmods_failed {
        return (
            tr("O driver não foi gerado no boot"),
            tr("O akmods falhou ao construir o módulo para o kernel atual. Reexecute o reparo do driver, confira o log do akmods e reinicie antes de testar a câmera novamente."),
        );
    }

    if module_origin == ModuleOrigin::InTree && clock_error {
        return (
            tr("O sistema caiu para o driver in-tree"),
            tr("O boot registrou que o ov02c10 carregado foi o do kernel, que não suporta o clock de 26 MHz deste hardware. Ajuste a prioridade do driver corrigido pela seção de ações rápidas e reinicie."),
        );
    }

    if module_origin == ModuleOrigin::Patched && !module_loaded {
        return (
            tr("O driver corrigido não foi carregado"),
            tr("O módulo ov02c10 corrigido está instalado no sistema, mas não entrou no kernel. Habilite o carregamento automático do driver e reinicie para a câmera voltar a aparecer no grafo de mídia."),
        );
    }

    if libcamera_permission_blocked {
        return (
            tr("O bridge do navegador bloqueou o libcamera"),
            tr("Uma configuração antiga da câmera para navegador removeu o acesso do usuário aos nós crus do IPU6. Reaplique a ação rápida da câmera para navegador para migrar o bridge para o fluxo novo, que mantém o libcamera e a webcam virtual funcionando juntos."),
        );
    }

    if manual_updates_override && !libcamera_detected {
        return (
            tr("O override manual da câmera está atrapalhando o libcamera"),
            tr("A câmera do kernel parece estável, mas o caminho direto do Galaxy Book Câmera não encontrou o sensor enquanto um ov02c10 manual em /updates está ativo. O próximo passo é restaurar o stack Intel IPU6 pela seção de ações rápidas."),
        );
    }

    if !browser_camera_ready && (libcamera_detected || camera_source_ready) {
        return (
            tr("Compatibilidade com navegador pendente"),
            tr("A câmera base já está pronta para uso no sistema, mas a webcam virtual para Meet, Discord, Teams e outros apps ainda não está ativa. Use a ação rápida para expor a câmera como dispositivo V4L2."),
        );
    }

    if !libcamera_detected && !camera_source_ready {
        return (
            tr("A câmera ainda não apareceu no caminho direto do app"),
            tr("O driver e os pacotes principais parecem presentes, mas a câmera ainda não foi detectada nem no caminho direto do Galaxy Book Câmera nem nas fontes do sistema. O próximo passo é revisar os logs do boot e a pilha IPU6."),
        );
    }

    if !camera_app_installed {
        return (
            tr("Driver pronto, app da câmera ausente"),
            tr("A câmera já aparece no caminho direto do Galaxy Book Câmera. Instale o app para validar preview, foto e vídeo no fluxo final."),
        );
    }

    if speaker_supported && !speaker_ready {
        return (
            tr("Suporte dos alto-falantes pendente"),
            tr("A máquina expõe amplificadores MAX98390, mas o pacote de speakers ainda não está pronto ou os módulos não foram carregados. Ative o suporte dos alto-falantes internos pela seção de ações rápidas e teste a saída Speaker novamente."),
        );
    }

    if speaker_ready && !sound_app_installed {
        return (
            tr("Painel de som opcional ainda não instalado"),
            tr("O suporte MAX98390 já parece pronto. Se você quiser equalizador, perfis e Atmos compatível, instale o Galaxy Book Sound pela seção de ações rápidas."),
        );
    }

    (
        tr("Fluxo principal da câmera parece pronto"),
        tr("O módulo corrigido parece ativo, o caminho direto do Galaxy Book Câmera já vê a câmera e o app final está instalado. Se os alto-falantes também já estiverem prontos, o próximo passo é abrir o Galaxy Book Câmera ou o Galaxy Book Sound e validar o fluxo final."),
    )
}

fn extract_first_matching_line(output: &str, patterns: &[&str]) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| patterns.iter().any(|pattern| line.contains(pattern)))
        .map(ToOwned::to_owned)
}

fn libcamera_output_has_camera(output: &str) -> bool {
    output.contains("Internal front camera") || output.contains("'ov02c10'")
}

fn direct_camera_command_text(command: &str, args: &[&str]) -> Result<String, ()> {
    let mut command = Command::new(command);
    command.args(args);
    if Path::new(CAMERA_APP_TUNING_FILE).is_file() {
        command.env("LIBCAMERA_SIMPLE_TUNING_FILE", CAMERA_APP_TUNING_FILE);
    }

    let output = command.output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn command_text(command: &str, args: &[&str]) -> Result<String, ()> {
    let output = Command::new(command).args(args).output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(())
        } else {
            Ok(stderr)
        }
    } else {
        Ok(stdout)
    }
}

fn current_user_name() -> Option<String> {
    env::var("USER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn detect_fingerprint_sensor_line() -> Option<String> {
    command_text("lsusb", &[])
        .ok()
        .and_then(|output| extract_first_matching_line(&output, FINGERPRINT_SENSOR_PATTERNS))
}

fn authselect_has_fingerprint(output: &str) -> bool {
    output.lines().any(|line| line.trim() == "- with-fingerprint")
        || output.contains("with-fingerprint")
}

fn fingerprint_enrollment_state(output: &str) -> FingerprintEnrollmentState {
    let normalized = output.to_ascii_lowercase();
    if normalized.contains("has no fingers enrolled") {
        FingerprintEnrollmentState::NotEnrolled
    } else if normalized.contains("device or resource busy") {
        FingerprintEnrollmentState::Busy
    } else if normalized.contains("no devices available")
        || normalized.contains("found 0 devices")
        || normalized.contains("no devices found")
    {
        FingerprintEnrollmentState::NoDevice
    } else if normalized.contains("using device")
        || normalized.contains("found 1 devices")
        || normalized.contains("found 2 devices")
        || normalized.contains("finger")
    {
        FingerprintEnrollmentState::Enrolled
    } else {
        FingerprintEnrollmentState::Unavailable
    }
}

fn systemd_unit_is_active(unit: &str) -> bool {
    if !systemd_available() {
        return false;
    }

    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn detect_system_camera_source_ready() -> bool {
    command_text("wpctl", &["status"])
        .map(|output| {
            output.contains("Intel MIPI Camera")
                || output.contains("ov02c10 [libcamera]")
                || output.contains("Câmera frontal interna")
        })
        .unwrap_or(false)
}

fn ipu6_raw_video_devices() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/class/video4linux") else {
        return Vec::new();
    };

    let mut devices = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.join("name");
        let devnode = format!("/dev/{}", entry.file_name().to_string_lossy());
        let Ok(device_name) = fs::read_to_string(name) else {
            continue;
        };
        if device_name.starts_with("Intel IPU6 ISYS Capture")
            || device_name.starts_with("Intel IPU6 CSI2")
        {
            devices.push(devnode);
        }
    }

    devices.sort();
    devices
}

fn video_device_has_uaccess(devnode: &str) -> bool {
    command_text("udevadm", &["info", "-q", "property", "-n", devnode])
        .ok()
        .map(|output| {
            output.lines().any(|line| {
                line.strip_prefix("CURRENT_TAGS=")
                    .map(|tags| tags.contains("uaccess"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn browser_camera_rule_blocks_libcamera() -> bool {
    if !Path::new("/etc/udev/rules.d/90-hide-ipu6-v4l2.rules").is_file() {
        return false;
    }

    let raw_devices = ipu6_raw_video_devices();
    !raw_devices.is_empty()
        && raw_devices
            .iter()
            .all(|device| !video_device_has_uaccess(device))
}

fn systemd_unit_enabled_state(unit: &str) -> Option<String> {
    if !systemd_available() {
        return None;
    }

    let output = Command::new("systemctl")
        .args(["is-enabled", unit])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        Some(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        }
    }
}

fn systemd_available() -> bool {
    if !Path::new("/run/systemd/system").exists() {
        return false;
    }

    Command::new("systemctl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn rpm_installed(package: &str) -> bool {
    Command::new("rpm")
        .args(["-q", package])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn detect_sound_app_installed() -> bool {
    rpm_installed("galaxybook-sound") || command_exists("galaxybook-sound")
}

fn command_exists(command: &str) -> bool {
    Command::new("bash")
        .args(["-lc", &format!("command -v {command} >/dev/null 2>&1")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn rpm_owner_for_file(path: &str) -> Option<String> {
    let output = Command::new("rpm")
        .args(["-qf", path, "--qf", "%{NAME}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let owner = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if owner.is_empty() {
        None
    } else {
        Some(owner)
    }
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|text| text.trim().to_string())
}

fn find_virtual_video_device_by_name(card_label: &str) -> Option<String> {
    let entries = fs::read_dir("/sys/devices/virtual/video4linux").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(name) = fs::read_to_string(path.join("name")) {
            if name.trim() == card_label {
                return Some(format!("/dev/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    None
}

fn v4l2_device_supports_capture(device: &str) -> bool {
    command_text("v4l2-ctl", &["-D", "-d", device])
        .map(|output| output.contains("Video Capture"))
        .unwrap_or(false)
}

fn parse_secure_boot(output: &str) -> &'static str {
    if output.contains("SecureBoot enabled") {
        "Ativado"
    } else if output.contains("SecureBoot disabled") {
        "Desativado"
    } else {
        "Não foi possível verificar"
    }
}

fn command_output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
        (true, true) => String::new(),
    }
}

fn mokutil_test_key_reports_enrolled(success: bool, output: &str) -> bool {
    success || output.to_ascii_lowercase().contains("already enrolled")
}

fn akmods_key_cn_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
}

fn current_akmods_key_cn() -> Option<String> {
    fs::canonicalize(AKMODS_PUBLIC_KEY_PATH)
        .ok()
        .and_then(|path| akmods_key_cn_from_path(&path))
}

fn mok_enrolled_list_contains_key(output: &str, key_cn: &str) -> bool {
    output.contains(&format!("CN={key_cn}"))
}

fn mok_pending_request_exists() -> bool {
    Command::new("mokutil")
        .arg("--list-new")
        .output()
        .ok()
        .map(|output| {
            output.status.success()
                && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_origin_detects_external_driver_paths() {
        assert_eq!(
            module_origin_from_path(Some(
                "/lib/modules/6.19.10/extra/intel-ipu6/drivers/media/i2c/ov02c10.ko.xz"
            )),
            ModuleOrigin::Patched
        );
        assert_eq!(
            module_origin_from_path(Some(
                "/lib/modules/6.19.10/updates/ov02c10.ko"
            )),
            ModuleOrigin::Patched
        );
    }

    #[test]
    fn module_origin_detects_in_tree_driver_paths() {
        assert_eq!(
            module_origin_from_path(Some(
                "/lib/modules/6.19.10/kernel/drivers/media/i2c/ov02c10.ko.xz"
            )),
            ModuleOrigin::InTree
        );
    }

    #[test]
    fn clock_error_detection_matches_known_boot_failure() {
        let logs = "
            ov02c10 i2c-OVTI02C1:00: error -EINVAL: external clock 26000000 is not supported
            ov02c10 i2c-OVTI02C1:00: probe with driver ov02c10 failed with error -22
        ";
        assert!(detect_clock_error(logs));
    }

    #[test]
    fn secure_boot_parser_understands_mokutil_output() {
        assert_eq!(parse_secure_boot("SecureBoot enabled"), "Ativado");
        assert_eq!(parse_secure_boot("SecureBoot disabled"), "Desativado");
        assert_eq!(parse_secure_boot("whatever"), "Não foi possível verificar");
    }

    #[test]
    fn mokutil_test_key_parser_accepts_already_enrolled_output() {
        assert!(mokutil_test_key_reports_enrolled(
            false,
            "/etc/pki/akmods/certs/public_key.der is already enrolled"
        ));
        assert!(!mokutil_test_key_reports_enrolled(
            false,
            "Failed to open /etc/pki/akmods/certs/public_key.der"
        ));
    }

    #[test]
    fn akmods_key_cn_parser_uses_canonical_cert_name() {
        let path = Path::new("/etc/pki/akmods/certs/fedora_1751571965_c906e85a.der");
        assert_eq!(
            akmods_key_cn_from_path(path).as_deref(),
            Some("fedora_1751571965_c906e85a")
        );
    }

    #[test]
    fn enrolled_list_parser_matches_current_akmods_cn() {
        let enrolled = "Subject: O=fedora, OU=fedora, emailAddress=akmods@fedora, L=None, ST=None, C=BR, CN=fedora_1751571965_c906e85a";
        assert!(mok_enrolled_list_contains_key(
            enrolled,
            "fedora_1751571965_c906e85a"
        ));
    }

    fn ok_secure_boot_key_item() -> CheckItem {
        CheckItem {
            title: "Chave do Secure Boot",
            detail: String::new(),
            health: Health::Good,
            code: "mok-enrolled",
        }
    }

    #[test]
    fn recommendation_prefers_install_when_packages_are_missing() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec!["akmod-galaxybook-ov02c10".into()],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Missing,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(title, "Instalação pendente");
    }

    #[test]
    fn recommendation_detects_unloaded_patched_driver() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "O driver corrigido não foi carregado");
    }

    #[test]
    fn parse_gsettings_array_extracts_extension_ids() {
        let parsed = parse_gsettings_string_array(
            "['ding@rastersoft.com', 'gsconnect@andyholmes.github.io']",
        );
        assert_eq!(
            parsed,
            vec![
                "ding@rastersoft.com".to_string(),
                "gsconnect@andyholmes.github.io".to_string()
            ]
        );
    }

    #[test]
    fn extension_check_marks_installed_but_disabled_as_warning() {
        let item = extension_check(
            "GSConnect",
            &[GSCONNECT_EXTENSION_ID],
            &[],
            &[GSCONNECT_EXTENSION_ID.to_string()],
            "missing",
        );
        assert_eq!(item.health, Health::Warning);
    }

    #[test]
    fn dash_to_dock_check_is_good_when_profile_matches() {
        let item = dash_to_dock_check_from_state(true, true, &[]);
        assert_eq!(item.health, Health::Good);
        assert_eq!(item.code, "dash-to-dock-ready");
    }

    #[test]
    fn dash_to_dock_check_warns_on_profile_drift() {
        let item = dash_to_dock_check_from_state(
            true,
            true,
            &["auto-ocultação", "lixeira visível"],
        );
        assert_eq!(item.health, Health::Warning);
        assert_eq!(item.code, "dash-to-dock-mismatch");
        assert!(item.detail.contains("auto-ocultação"));
        assert!(item.detail.contains("lixeira visível"));
    }

    #[test]
    fn browser_camera_check_warns_when_bridge_and_system_source_are_missing() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec![],
        };
        let item = detect_browser_camera_check(&packages, false, false);
        assert_eq!(item.health, Health::Warning);
        assert!(item.detail.contains("fontes do sistema"));
    }

    #[test]
    fn browser_camera_check_accepts_system_source_without_libcamera_direct() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec![],
        };
        let item = detect_browser_camera_check(&packages, false, true);
        assert_eq!(item.health, Health::Warning);
        assert!(item.detail.contains("já aparece nas fontes do sistema"));
    }

    #[test]
    fn browser_camera_recommendation_kicks_in_after_libcamera_is_ready() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "Compatibilidade com navegador pendente");
    }

    #[test]
    fn speaker_recommendation_appears_when_camera_flow_is_ready() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
        );
        assert_eq!(title, "Suporte dos alto-falantes pendente");
    }

    #[test]
    fn sound_app_recommendation_appears_after_speakers_are_ready() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            true,
            false,
        );
        assert_eq!(title, "Painel de som opcional ainda não instalado");
    }

    #[test]
    fn recommendation_prioritizes_secure_boot_key_before_packages() {
        let secure_boot_key = CheckItem {
            title: "Chave do Secure Boot",
            detail: String::new(),
            health: Health::Error,
            code: "mok-not-enrolled",
        };
        let packages = PackagePresence {
            installed: vec![],
            missing: vec!["akmod-galaxybook-ov02c10".into()],
        };
        let (title, _) = recommend_next_step(
            &secure_boot_key,
            &packages,
            false,
            ModuleOrigin::Missing,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(title, "Prepare a chave do Secure Boot");
    }

    #[test]
    fn recommendation_detects_browser_rule_blocking_libcamera() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "O bridge do navegador bloqueou o libcamera");
    }

    #[test]
    fn libcamera_detection_accepts_sensor_name_output() {
        let output = "Available cameras:\n1: 'ov02c10' (_SB_.PC00.LNK0)\n";
        assert!(libcamera_output_has_camera(output));
    }

    #[test]
    fn authselect_parser_detects_fingerprint_feature() {
        assert!(authselect_has_fingerprint(
            "Profile ID: sssd\nEnabled features:\n- with-fingerprint\n- with-mdns4\n"
        ));
        assert!(!authselect_has_fingerprint(
            "Profile ID: sssd\nEnabled features:\n- with-mdns4\n"
        ));
    }

    #[test]
    fn fingerprint_state_detects_not_enrolled_output() {
        let output = "found 1 devices\nUsing device /net/reactivated/Fprint/Device/0\nUser regiscaio has no fingers enrolled for Egis Technology (LighTuning) Match-on-Chip.";
        assert_eq!(
            fingerprint_enrollment_state(output),
            FingerprintEnrollmentState::NotEnrolled
        );
    }
}
