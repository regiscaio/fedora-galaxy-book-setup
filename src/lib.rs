mod i18n;
mod module_build_id;
mod updates;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use module_build_id::loaded_module_binary_matches_candidate;

pub const APP_ID: &str = "com.caioregis.GalaxyBookSetup";
pub const APP_NAME: &str = "Galaxy Book Setup";
pub use i18n::{init_i18n, tr, tr_mark, trf, trn};
pub use updates::package_update_names;
pub const CAMERA_APP_DESKTOP_ID: &str = "com.caioregis.GalaxyBookCamera.desktop";
pub const SOUND_APP_DESKTOP_ID: &str = "com.caioregis.GalaxyBookSound.desktop";
pub const BROWSER_SENSOR_UNBOUND: &str = "browser-sensor-unbound";
pub const AKMODS_PUBLIC_KEY_PATH: &str = "/etc/pki/akmods/certs/public_key.der";
pub const AKMODS_PRIVATE_KEY_PATH: &str = "/etc/pki/akmods/private/private_key.priv";
pub const AKMODS_KEY_CN_PATH: &str = "/var/lib/galaxybook-setup/akmods-key.cn";
pub const AKMODS_PUBLIC_CERTIFICATE_STATE_PATH: &str =
    "/var/lib/galaxybook-setup/akmods-public-key.der";
const LEGACY_MOK_PENDING_CN_PATH: &str = "/var/lib/galaxybook-setup/mok-pending.cn";
const OV_READY_DIRECTORY: &str = "/var/lib/galaxybook-ov02c10/ready";
const PROBE_TIMEOUT_COMMAND: &str = "/usr/bin/timeout";
const PROBE_TIMEOUT_DURATION: &str = "8s";
const PROBE_TIMEOUT_KILL_AFTER: &str = "2s";
const OPENSSL_COMMAND: &str = "/usr/bin/openssl";
const MAX_PUBLIC_CERTIFICATE_SIZE: u64 = 16 * 1024;
// Linux O_NOFOLLOW. Galaxy Book Setup is packaged only for Fedora/Linux.
const LINUX_O_NOFOLLOW: i32 = 0o400000;
const OPENSSL_CERTIFICATE_IDENTITY_ARGS: &[&str] = &[
    "x509",
    "-inform",
    "DER",
    "-noout",
    "-subject",
    "-nameopt",
    "RFC2253",
    "-fingerprint",
    "-sha1",
    "-serial",
];
const RPM_PACKAGE_QUERY_ALLOWLIST: &[&str] = &[
    "galaxybook-ov02c10-kmod-common",
    "akmod-galaxybook-ov02c10",
    "kmod-galaxybook-ov02c10",
    "galaxybook-camera",
    "v4l2-relayd",
    "v4l2loopback",
    "gstreamer1-plugins-icamerasrc",
    "v4l-utils",
    "galaxybook-max98390-kmod-common",
    "akmod-galaxybook-max98390",
    "kmod-galaxybook-max98390",
    "fprintd",
    "libfprint",
    "akmod-nvidia",
    "xorg-x11-drv-nvidia-cuda",
    "galaxybook-sound",
];
const RPM_DYNAMIC_KMOD_QUERY_PREFIXES: &[&str] =
    &["kmod-galaxybook-ov02c10-", "kmod-galaxybook-max98390-"];
pub const KMOD_RECONCILE_HELPER: &str = "/usr/libexec/galaxybook-setup/reconcile-kmods";
pub const OV02C10_KMOD_MIN_VERSION: &str = "1.00.25";
pub const MAX98390_KMOD_MIN_VERSION: &str = "1.00.12";
pub const INSTALL_MAIN_SUPPORT_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/driver-maintenance install-main";
pub const INSTALL_CAMERA_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/driver-maintenance install-camera";
pub const INSTALL_SOUND_APP_COMMAND: &str = "dnf install -y 'galaxybook-sound >= 1.00.18'";
pub const REPAIR_CAMERA_COMMAND: &str = FORCE_CAMERA_DRIVER_COMMAND;
pub const ENABLE_CAMERA_MODULE_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/reconcile-kmods ov02c10";
pub const FORCE_CAMERA_DRIVER_COMMAND: &str = INSTALL_CAMERA_COMMAND;

pub const ENABLE_BROWSER_CAMERA_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/driver-maintenance enable-browser-camera";
pub const ENABLE_SPEAKER_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/driver-maintenance enable-speakers";

pub const PREPARE_SECURE_BOOT_KEY_COMMAND: &str =
    "exec /usr/libexec/galaxybook-setup/prepare-secure-boot-key";
pub const REPAIR_NVIDIA_COMMAND: &str = "exec /usr/libexec/galaxybook-setup/repair-nvidia-kmod";
pub const SET_BALANCED_PROFILE_COMMAND: &str = r#"if [ -w /sys/firmware/acpi/platform_profile ]; then printf 'balanced' > /sys/firmware/acpi/platform_profile; cat /sys/firmware/acpi/platform_profile; else echo 'O perfil da plataforma não está disponível neste sistema.' >&2; exit 1; fi"#;
pub const REPAIR_FINGERPRINT_COMMAND: &str = r#"dnf reinstall -y fprintd libfprint && systemctl restart fprintd && rpm -q fprintd libfprint"#;
pub const ENABLE_FINGERPRINT_AUTH_COMMAND: &str = r#"authselect enable-feature with-fingerprint && authselect apply-changes && authselect current"#;
pub const OPEN_FINGERPRINT_SETTINGS_COMMAND: &str = r#"if ! command -v gnome-control-center >/dev/null 2>&1; then echo "O gnome-control-center não está disponível neste sistema." >&2; exit 1; fi
nohup gnome-control-center user-accounts >/dev/null 2>&1 &
echo "As configurações de Usuários foram abertas para gerenciar o cadastro de digitais."
"#;
pub const REBOOT_COMMAND: &str = r#"set -euo pipefail
probe_dir="$(mktemp -d /tmp/galaxybook-setup-reboot.XXXXXXXX)"
probe_stdout="${probe_dir}/stdout"
probe_stderr="${probe_dir}/stderr"
cleanup_reboot_probe() {
  rm -f -- "$probe_stdout" "$probe_stderr"
  rmdir -- "$probe_dir"
}
trap cleanup_reboot_probe EXIT

if ! systemctl list-units \
  --type=service \
  --state=activating,running \
  --plain \
  --no-legend \
  akmods.service 'akmods@*.service' >"$probe_stdout" 2>"$probe_stderr"; then
  echo "Não foi possível consultar os builds do akmods. O reboot não foi iniciado." >&2
  exit 1
fi
if [ -s "$probe_stderr" ]; then
  echo "A consulta dos builds do akmods produziu um aviso. O reboot não foi iniciado." >&2
  exit 1
fi
if [ -s "$probe_stdout" ]; then
  echo "Há builds do akmods em andamento. Aguarde a conclusão antes de reiniciar." >&2
  exit 1
fi
systemctl reboot --check-inhibitors=yes
"#;

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
    ("shift-click-action", "'minimize'", "Shift+clique minimiza"),
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
    ("show-mounts-network", "false", "ocultar unidades de rede"),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KernelJournalProbe {
    Clean,
    ClockError,
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
    pub reboot_ready: bool,
    pub driver_reboot_ready: bool,
    pub camera_app_installed: bool,
    pub sound_app_installed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PackagePresence {
    installed: Vec<String>,
    missing: Vec<String>,
    unknown: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RpmPackagePresence {
    Installed,
    Absent,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HardwareDiscovery {
    Present,
    Absent,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RpmFileOwner {
    Owned(String),
    Unowned,
    Unknown,
}

impl RpmFileOwner {
    fn owned_name(&self) -> Option<&str> {
        match self {
            Self::Owned(owner) => Some(owner.as_str()),
            Self::Unowned | Self::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ModulePayloadInventory {
    invalid_kernels: Vec<String>,
    unknown_rpm_kernels: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FingerprintEnrollmentState {
    Enrolled,
    NotEnrolled,
    Busy,
    NoDevice,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyFileState {
    Present,
    Missing,
    Protected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileFingerprint {
    device: u64,
    inode: u64,
    size: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AkmodsKeyIdentity {
    cn: String,
    fingerprint_sha1: String,
    certificate_serial: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AkmodsKeyMarker {
    V2(AkmodsKeyIdentity),
    LegacyCn(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FingerprintContext {
    sensor_line: Option<String>,
    missing_packages: Vec<String>,
    unknown_packages: Vec<String>,
    authselect_enabled: bool,
    list_state: FingerprintEnrollmentState,
}

pub fn collect_snapshot() -> SetupSnapshot {
    let secure_boot_state = secure_boot_enabled();
    let secure_boot_requires_signatures = secure_boot_state_requires_signatures(secure_boot_state);
    let expected_module_identity = if secure_boot_requires_signatures {
        current_akmods_key_identity()
    } else {
        None
    };
    let running_kernel = running_kernel_release();
    let system = SystemSummary {
        notebook: detect_notebook(),
        fedora: detect_fedora_release(),
        kernel: running_kernel.unwrap_or_else(|| "Desconhecido".into()),
        secure_boot: secure_boot_label(secure_boot_state),
    };
    let secure_boot_key_check =
        detect_secure_boot_key_check(secure_boot_state, expected_module_identity.as_ref());

    let packages = package_presence(&[
        "galaxybook-ov02c10-kmod-common",
        "akmod-galaxybook-ov02c10",
        "kmod-galaxybook-ov02c10",
        "galaxybook-camera",
    ]);
    let camera_app_installed = packages
        .installed
        .iter()
        .any(|pkg| pkg == "galaxybook-camera");

    let package_check = main_package_check(&packages);

    let kernel_inventory = installed_kernel_releases(&system.kernel);
    let kernel_inventory_complete = kernel_inventory.is_some();
    let kernels = kernel_inventory.unwrap_or_else(|| parse_kernel_releases(&system.kernel, ""));
    let akmods_log = command_text(
        "journalctl",
        &["-b", "-u", "akmods", "--no-pager", "--lines=2000"],
    )
    .unwrap_or_default();
    let akmods_failed = akmods_log.lines().any(|line| {
        line.contains("galaxybook-ov02c10")
            && (line.contains("[FAILED]")
                || line.contains("failed.log")
                || line.contains("Building rpms failed"))
    });
    let camera_akmod_presence = if packages
        .installed
        .iter()
        .any(|pkg| pkg == "akmod-galaxybook-ov02c10")
    {
        RpmPackagePresence::Installed
    } else if packages
        .missing
        .iter()
        .any(|pkg| pkg == "akmod-galaxybook-ov02c10")
    {
        RpmPackagePresence::Absent
    } else {
        RpmPackagePresence::Unknown
    };
    let camera_payload_inventory = module_payload_inventory(
        &kernels,
        "kmod-galaxybook-ov02c10",
        "ov02c10",
        "/extra/galaxybook-ov02c10/",
        secure_boot_requires_signatures,
        expected_module_identity.as_ref(),
    );
    let camera_payload_inventory_complete = camera_payload_inventory.unknown_rpm_kernels.is_empty();
    let invalid_camera_kernels = camera_payload_inventory.invalid_kernels;
    let akmods_payload_missing = camera_akmod_presence == RpmPackagePresence::Installed
        && (!kernel_inventory_complete || !invalid_camera_kernels.is_empty());
    let akmods_check = if camera_akmod_presence == RpmPackagePresence::Unknown {
        CheckItem {
            title: "Akmods no boot",
            detail: tr(
                "A consulta RPM do akmod OV02C10 não terminou de forma conclusiva. O setup não afirmou que o driver está ausente nem liberou o reboot específico dos drivers. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
            ),
            health: Health::Unknown,
            code: "akmods-package-query-unknown",
        }
    } else if camera_akmod_presence == RpmPackagePresence::Installed {
        if !kernel_inventory_complete {
            CheckItem {
                title: "Akmods no boot",
                detail: tr(
                    "Não foi possível enumerar todos os pacotes kernel-core instalados. O setup não liberou o reboot do driver porque não pode validar os payloads de kernels desconhecidos.",
                ),
                health: Health::Error,
                code: "akmods-kernel-inventory-unavailable",
            }
        } else if !camera_payload_inventory_complete {
            CheckItem {
                title: "Akmods no boot",
                detail: trf(
                    "A consulta RPM dos payloads OV02C10 por kernel não terminou de forma conclusiva em: {kernels}. O setup não classificou esses payloads como ausentes e bloqueou o reboot específico dos drivers. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                    &[(
                        "kernels",
                        camera_payload_inventory.unknown_rpm_kernels.join(", "),
                    )],
                ),
                health: Health::Unknown,
                code: "akmods-payload-package-query-unknown",
            }
        } else if akmods_payload_missing {
            CheckItem {
                title: "Akmods no boot",
                detail: if akmods_failed {
                    trf(
                        "O log registrou falha do OV02C10 e o estado final ainda não possui payload externo com dono RPM e vermagic correto em: {kernels}.",
                        &[("kernels", invalid_camera_kernels.join(", "))],
                    )
                } else {
                    trf(
                        "O akmods pode ter terminado, mas falta um payload OV02C10 externo com dono RPM e vermagic correto por kernel em: {kernels}.",
                        &[("kernels", invalid_camera_kernels.join(", "))],
                    )
                },
                health: Health::Error,
                code: "akmods-payload-missing",
            }
        } else if akmods_failed {
            CheckItem {
                title: "Akmods no boot",
                detail: trf(
                    "O log deste boot contém uma falha antiga, mas o estado final foi recuperado: payload OV02C10 externo com dono RPM e vermagic correto validado para {kernels}.",
                    &[("kernels", kernels.join(", "))],
                ),
                health: Health::Good,
                code: "akmods-recovered",
            }
        } else {
            CheckItem {
                title: "Akmods no boot",
                detail: trf(
                    "Payload OV02C10 externo, indexado, com dono RPM e vermagic correto validado para: {kernels}.",
                    &[("kernels", kernels.join(", "))],
                ),
                health: Health::Good,
                code: "akmods-ok",
            }
        }
    } else {
        CheckItem {
            title: "Akmods no boot",
            detail: tr("O driver ainda não foi instalado, então o akmods não executou esse fluxo."),
            health: Health::Unknown,
            code: "akmods-unavailable",
        }
    };

    let module_path = clean_command_text("modinfo", &["-n", "ov02c10"]).ok();
    let module_origin = module_origin_from_path(module_path.as_deref());
    let module_owner = module_path
        .as_deref()
        .map(rpm_owner_for_file)
        .unwrap_or(RpmFileOwner::Unknown);
    let module_loaded = read_trimmed("/proc/modules")
        .map(|modules| modules.lines().any(|line| line.starts_with("ov02c10 ")))
        .unwrap_or(false);
    let manual_updates_override = module_path
        .as_deref()
        .map(|path| manual_updates_override_is_proven(path, &module_owner))
        .unwrap_or(false);
    let kernel_journal = probe_kernel_journal();
    let clock_error = kernel_journal == KernelJournalProbe::ClockError;
    let running_module_matches_candidate = module_loaded
        && module_path.as_deref().is_some_and(|path| {
            let expected_package = format!("kmod-galaxybook-ov02c10-{}", system.kernel);
            if !module_payload_matches(
                &expected_package,
                Some(path),
                module_owner.owned_name(),
                "/extra/galaxybook-ov02c10/",
            ) {
                return false;
            }

            let loaded_srcversion = read_trimmed("/sys/module/ov02c10/srcversion");
            let candidate_srcversion =
                clean_command_text("modinfo", &["-F", "srcversion", path]).ok();
            let signer = clean_command_text("modinfo", &["-F", "signer", path]).ok();
            let sig_key = clean_command_text("modinfo", &["-F", "sig_key", path]).ok();
            let vermagic = clean_command_text("modinfo", &["-F", "vermagic", path]).ok();
            loaded_module_matches_candidate(
                loaded_module_binary_matches_candidate(
                    "ov02c10",
                    path,
                    loaded_srcversion.as_deref(),
                    candidate_srcversion.as_deref(),
                ),
                signer.as_deref(),
                sig_key.as_deref(),
                vermagic.as_deref(),
                &system.kernel,
                packaged_external_module_path("ov02c10", path),
                secure_boot_requires_signatures,
                expected_module_identity.as_ref(),
            )
        });
    let module_check = match (module_origin, module_path.as_deref()) {
        (ModuleOrigin::Patched, Some(path)) if clock_error => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "O rootfs agora resolve o OV02C10 empacotado em {path}, mas o erro de clock deste boot prova que o grafo IPU6 foi criado antes dessa correção ficar ativa. O módulo ocupado não será trocado ao vivo; valide os kernels e reinicie.",
                &[("path", path.to_string())],
            ),
            health: Health::Error,
            code: "module-reboot-required",
        },
        (ModuleOrigin::Patched, Some(path)) if !module_loaded => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "O módulo corrigido existe em {path}, mas ainda não foi carregado no kernel. Valide os payloads de todos os kernels pela ação rápida e reinicie normalmente; um módulo ocupado não será trocado ao vivo.",
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
        (ModuleOrigin::Patched, Some(path)) if running_module_matches_candidate => CheckItem {
            title: "Módulo ativo",
            detail: match &module_owner {
                RpmFileOwner::Owned(owner) => trf(
                    "O módulo carregado corresponde ao payload externo em {path}, fornecido por {owner}, com identidade binária comprovada primeiro por GNU build-id ou, quando ausente nos dois lados, por srcversion, além de vermagic e identidade de assinatura esperados.",
                    &[("path", path.to_string()), ("owner", owner.to_string())],
                ),
                RpmFileOwner::Unowned | RpmFileOwner::Unknown => trf(
                    "O módulo carregado corresponde ao payload externo em {path}.",
                    &[("path", path.to_string())],
                ),
            },
            health: Health::Good,
            code: "module-patched",
        },
        (ModuleOrigin::Patched, Some(path)) => CheckItem {
            title: "Módulo ativo",
            detail: trf(
                "O rootfs resolve o payload externo em {path}, mas o setup não conseguiu provar que o módulo já carregado é esse mesmo binário por GNU build-id ou, quando ausente nos dois lados, por srcversion, além de vermagic e identidade de assinatura. Valide os payloads e reinicie normalmente; um módulo ocupado não será trocado ao vivo.",
                &[("path", path.to_string())],
            ),
            health: Health::Warning,
            code: "module-reboot-required",
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
                    tr("A câmera interna apareceu no caminho direto usado pelo Galaxy Book Câmera.")
                }),
            health: Health::Good,
            code: "libcamera-ready",
        },
        Ok(_) if clock_error => libcamera_clock_probe_failed_check(),
        Ok(_) => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr(
                "A ferramenta cam executou, mas o caminho direto usado pelo Galaxy Book Câmera não listou a câmera interna. Isso não significa, por si só, que Snapshot, navegador ou o sistema também falharam.",
            ),
            health: Health::Warning,
            code: "libcamera-missing",
        },
        Err(_) if libcamera_permission_blocked => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr(
                "O caminho direto do libcamera perdeu acesso aos nós crus do IPU6 porque a regra UDev administrativa legada /etc/udev/rules.d/90-hide-ipu6-v4l2.rules remove o uaccess desses dispositivos. O setup preserva essa regra porque não pode removê-la automaticamente com segurança. Revise-a e, se apropriado, remova-a manualmente como administrador; depois execute novamente a ação da câmera para navegador.",
            ),
            health: Health::Error,
            code: "libcamera-permission-blocked",
        },
        Err(_) if clock_error => libcamera_clock_probe_failed_check(),
        Err(_) => CheckItem {
            title: "Caminho direto do Galaxy Book Câmera",
            detail: tr(
                "A ferramenta 'cam' não está disponível ou falhou ao executar, então o setup não conseguiu validar o caminho direto usado pelo Galaxy Book Câmera.",
            ),
            health: Health::Unknown,
            code: "libcamera-unavailable",
        },
    };

    let (browser_camera, camera_source_ready) =
        probe_browser_camera(Path::new("/sys/bus/i2c/devices"), libcamera_detected);
    let max_driver_packages = [
        "galaxybook-max98390-kmod-common",
        "akmod-galaxybook-max98390",
        "kmod-galaxybook-max98390",
    ];
    let max_driver_inventory = package_presence(&max_driver_packages);
    let speakers_check = detect_speakers_check(
        secure_boot_requires_signatures,
        expected_module_identity.as_ref(),
        &kernels,
        kernel_inventory_complete,
        &max_driver_inventory,
    );
    let camera_driver_packages = [
        "galaxybook-ov02c10-kmod-common",
        "akmod-galaxybook-ov02c10",
        "kmod-galaxybook-ov02c10",
    ];
    let camera_driver_present = camera_driver_packages.iter().any(|package| {
        packages
            .installed
            .iter()
            .any(|installed| installed == package)
    });
    let camera_driver_complete = camera_driver_packages.iter().all(|package| {
        packages
            .installed
            .iter()
            .any(|installed| installed == package)
    });
    let camera_driver_inventory_complete = camera_payload_inventory_complete
        && package_inventory_is_complete_for(&packages, &camera_driver_packages);
    let max_driver_inventory_complete =
        package_inventory_is_complete_for(&max_driver_inventory, &max_driver_packages);
    let max_driver_present = !max_driver_inventory.installed.is_empty();
    let camera_attestations_ready = !camera_driver_present
        || (camera_driver_complete
            && kernel_inventory_complete
            && invalid_camera_kernels.is_empty()
            && kernels
                .iter()
                .all(|kernel| ov02c10_readiness_attestation_is_current(kernel)));
    let camera_reboot_ready = camera_payload_state_allows_reboot(
        camera_driver_present,
        camera_driver_complete,
        &invalid_camera_kernels,
        camera_attestations_ready,
    );
    let max_reboot_ready = max_driver_state_allows_reboot(max_driver_present, speakers_check.code);
    let driver_reboot_ready = driver_reboot_state_allows_reboot(
        camera_driver_inventory_complete,
        camera_reboot_ready,
        max_driver_inventory_complete,
        max_reboot_ready,
    );
    let reboot_ready = global_reboot_ready(akmods_build_active());
    let sound_app_presence = detect_sound_app_presence();
    let sound_app_installed = sound_app_presence == RpmPackagePresence::Installed;
    let sound_app_check = detect_sound_app_check(sound_app_presence);
    let fingerprint = collect_fingerprint_context();
    let fingerprint_reader_check = detect_fingerprint_reader_check(&fingerprint);
    let fingerprint_login_check = detect_fingerprint_login_check(&fingerprint);

    let boot_check = boot_check_from_kernel_log(kernel_journal);

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
    let dock_check = detect_dash_to_dock_check(&enabled_extensions, &installed_extensions);

    let (recommendation_title, recommendation_body) = recommend_next_step(
        &secure_boot_key_check,
        &packages,
        akmods_payload_missing,
        module_origin,
        manual_updates_override,
        clock_error,
        module_loaded,
        running_module_matches_candidate,
        libcamera_detected,
        libcamera_permission_blocked,
        camera_source_ready,
        browser_camera.code,
        camera_app_installed,
        speakers_check.health != Health::Unknown,
        speakers_check.health == Health::Good,
        sound_app_presence != RpmPackagePresence::Absent,
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
        reboot_ready,
        driver_reboot_ready,
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
    read_trimmed("/etc/fedora-release").unwrap_or_else(|| tr("Fedora (versão não identificada)"))
}

fn secure_boot_label(state: Option<bool>) -> String {
    match state {
        Some(true) => tr("Ativado"),
        Some(false) => tr("Desativado"),
        None => tr("Não foi possível verificar"),
    }
}

fn secure_boot_enabled() -> Option<bool> {
    let output = probe_output("mokutil", &["--sb-state"], true).ok()?;
    classify_secure_boot_output(output.status.code(), &output.stdout, &output.stderr)
}

fn package_presence(packages: &[&str]) -> PackagePresence {
    package_presence_with(packages, rpm_package_presence)
}

fn main_package_check(packages: &PackagePresence) -> CheckItem {
    if !packages.unknown.is_empty() {
        CheckItem {
            title: "Pacotes principais",
            detail: trf(
                "Não foi possível confirmar o estado RPM de: {packages}. O setup não classificou esses pacotes como ausentes. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                &[("packages", packages.unknown.join(", "))],
            ),
            health: Health::Unknown,
            code: "packages-query-unknown",
        }
    } else if packages.missing.is_empty() {
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
    }
}

fn package_presence_with<F>(packages: &[&str], mut query: F) -> PackagePresence
where
    F: FnMut(&str) -> RpmPackagePresence,
{
    let mut inventory = PackagePresence::default();

    for package in packages {
        match query(package) {
            RpmPackagePresence::Installed => inventory.installed.push((*package).into()),
            RpmPackagePresence::Absent => inventory.missing.push((*package).into()),
            RpmPackagePresence::Unknown => inventory.unknown.push((*package).into()),
        }
    }

    inventory
}

fn package_inventory_is_complete_for(inventory: &PackagePresence, packages: &[&str]) -> bool {
    !packages
        .iter()
        .any(|package| inventory.unknown.iter().any(|unknown| unknown == package))
}

fn enabled_gnome_shell_extensions() -> Vec<String> {
    command_text(
        "gsettings",
        &["get", "org.gnome.shell", "enabled-extensions"],
    )
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
        .filter_map(
            |(key, expected, label)| match gsettings_value(DASH_TO_DOCK_SCHEMA, key) {
                Some(current) if current == *expected => None,
                Some(_) | None => Some(*label),
            },
        )
        .collect()
}

fn dash_to_dock_check_from_state(enabled: bool, installed: bool, mismatches: &[&str]) -> CheckItem {
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

fn build_gnome_extension_profile_command(extension_id: &str, prelude_commands: &[&str]) -> String {
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
        &[r#"pkexec /usr/bin/dnf install -y nautilus-python || true"#],
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
  pkexec /usr/bin/dnf install -y gnome-shell-extension-dash-to-dock
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
    let akmod_presence = rpm_package_presence("akmod-nvidia");
    let modules_loaded = read_trimmed("/proc/modules")
        .map(|modules| modules.lines().any(|line| line.starts_with("nvidia ")))
        .unwrap_or(false);
    let smi_presence = rpm_package_presence("xorg-x11-drv-nvidia-cuda");

    if modules_loaded {
        let (detail, health, code) = match smi_presence {
            RpmPackagePresence::Installed => (
                tr("Módulos NVIDIA carregados. O utilitário nvidia-smi também está instalado."),
                Health::Good,
                "nvidia-ready",
            ),
            RpmPackagePresence::Absent => (
                tr(
                    "Módulos NVIDIA carregados. O utilitário nvidia-smi continua opcional e não está instalado.",
                ),
                Health::Good,
                "nvidia-ready",
            ),
            RpmPackagePresence::Unknown => (
                tr(
                    "Módulos NVIDIA carregados, mas a consulta RPM não confirmou o estado do utilitário nvidia-smi. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                ),
                Health::Unknown,
                "nvidia-package-query-unknown",
            ),
        };
        return CheckItem {
            title: "Driver NVIDIA",
            detail,
            health,
            code,
        };
    }

    match akmod_presence {
        RpmPackagePresence::Installed => CheckItem {
            title: "Driver NVIDIA",
            detail: tr(
                "O pacote akmod-nvidia está instalado, mas os módulos não estão carregados para o kernel atual.",
            ),
            health: Health::Warning,
            code: "nvidia-akmod-installed",
        },
        RpmPackagePresence::Absent => CheckItem {
            title: "Driver NVIDIA",
            detail: tr(
                "O suporte NVIDIA ainda não foi instalado. O setup trata o akmod-nvidia como o pacote principal para esta etapa.",
            ),
            health: Health::Unknown,
            code: "nvidia-missing",
        },
        RpmPackagePresence::Unknown => CheckItem {
            title: "Driver NVIDIA",
            detail: tr(
                "A consulta RPM não confirmou se o akmod-nvidia está instalado. O setup não classificou o driver como ausente. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
            ),
            health: Health::Unknown,
            code: "nvidia-package-query-unknown",
        },
    }
}

fn detect_secure_boot_key_check(
    secure_boot_state: Option<bool>,
    current_key_identity: Option<&AkmodsKeyIdentity>,
) -> CheckItem {
    let title = "Chave do Secure Boot";

    let Some(secure_boot_enabled) = secure_boot_state else {
        return CheckItem {
            title,
            detail: tr(
                "O setup não conseguiu consultar o estado do Secure Boot/MOK neste sistema.",
            ),
            health: Health::Unknown,
            code: "mok-unavailable",
        };
    };

    if !secure_boot_enabled {
        return CheckItem {
            title,
            detail: tr(
                "Secure Boot está desativado. O MOK do akmods não é necessário para carregar módulos externos neste boot.",
            ),
            health: Health::Good,
            code: "mok-not-needed",
        };
    }

    if current_key_identity.is_some_and(mok_pending_request_exists) {
        return CheckItem {
            title,
            detail: tr(
                "A chave do akmods já foi preparada e o pedido de importação está pendente no MOK. Reinicie normalmente e conclua 'Enroll MOK' na tela azul do boot.",
            ),
            health: Health::Warning,
            code: "mok-pending-enrollment",
        };
    }

    if current_key_identity.is_some_and(signed_external_module_is_loaded) {
        return CheckItem {
            title,
            detail: tr(
                "Secure Boot aceitou um módulo externo assinado neste boot. Os diretórios privados do akmods não precisam ser legíveis pelo aplicativo sem privilégios.",
            ),
            health: Health::Good,
            code: "mok-module-accepted",
        };
    }

    // The certificate and private-key directories are normally 0750
    // root:akmods. Only a successful MOK query plus a v2 marker that exactly
    // matches the trusted public DER copy may establish enrollment.
    if current_key_identity.is_some_and(|identity| {
        probe_output("mokutil", &["--list-enrolled"], true).is_ok_and(|output| {
            mok_enrollment_is_proven(Some(identity), strict_success_stdout(&output), false)
        })
    }) {
        return CheckItem {
            title,
            detail: tr(
                "A chave pública do akmods já está inscrita no MOK. Os módulos externos podem ser assinados e aceitos pelo Secure Boot neste host.",
            ),
            health: Health::Good,
            code: "mok-enrolled",
        };
    }

    if legacy_akmods_marker_cn().is_some() {
        return CheckItem {
            title,
            detail: tr(
                "O setup encontrou apenas um marcador legado por nome da chave. Como nomes podem ser reutilizados após uma rotação, esse marcador não prova a identidade criptográfica atual; execute a ação administrativa para publicar a identidade v2 antes de liberar módulos ou reboot de reparo.",
            ),
            health: Health::Unknown,
            code: "mok-legacy-identity",
        };
    }

    if current_key_identity.is_some() {
        let test_output = probe_output(
            "mokutil",
            &["--test-key", AKMODS_PUBLIC_CERTIFICATE_STATE_PATH],
            true,
        )
        .ok();
        let public_certificate_test_succeeded = test_output
            .as_ref()
            .and_then(strict_success_stdout)
            .is_some();
        if mok_enrollment_is_proven(
            current_key_identity,
            None,
            public_certificate_test_succeeded,
        ) {
            return CheckItem {
                title,
                detail: tr(
                    "A chave pública do akmods já está inscrita no MOK. Os módulos externos podem ser assinados e aceitos pelo Secure Boot neste host.",
                ),
                health: Health::Good,
                code: "mok-enrolled",
            };
        }
    }

    let public_key_state = key_file_state(AKMODS_PUBLIC_KEY_PATH);
    let private_key_state = key_file_state(AKMODS_PRIVATE_KEY_PATH);
    if matches!(public_key_state, KeyFileState::Protected)
        || matches!(private_key_state, KeyFileState::Protected)
    {
        return CheckItem {
            title,
            detail: tr(
                "Secure Boot está ativo, mas os diretórios da chave do akmods são protegidos do processo sem privilégios. O setup não tratou essa falta de acesso como chave ausente; a ação administrativa fará a validação definitiva.",
            ),
            health: Health::Unknown,
            code: "mok-key-protected",
        };
    }

    if matches!(public_key_state, KeyFileState::Missing)
        || matches!(private_key_state, KeyFileState::Missing)
    {
        return CheckItem {
            title,
            detail: tr(
                "Secure Boot está ativo, mas o akmods ainda não tem a chave local pronta em /etc/pki/akmods. Prepare a chave do Secure Boot antes de instalar ou carregar módulos externos.",
            ),
            health: Health::Error,
            code: "mok-key-missing",
        };
    }

    if current_key_identity.is_none() {
        return CheckItem {
            title,
            detail: tr(
                "A chave local existe, mas o setup não conseguiu comparar o certificado público atual com uma identidade v2 root-owned contendo CN, fingerprint SHA-1 e serial. Execute a ação administrativa para republicar a identidade antes de liberar módulos ou reboot de reparo.",
            ),
            health: Health::Unknown,
            code: "mok-key-identity-unverified",
        };
    }

    CheckItem {
        title,
        detail: tr(
            "Secure Boot está ativo e a chave local do akmods existe, mas ainda não foi aceita no MOK. Use a ação rápida dedicada antes de tentar carregar módulos externos.",
        ),
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

// A missing driver on every present OV02C10 blocks the relay. Other sensors and
// unreadable sysfs state retain the normal browser diagnostics.
fn all_ov_sensors_unbound(devices: &Path) -> bool {
    let Ok(entries) = fs::read_dir(devices) else {
        return false;
    };
    let mut found = false;
    for entry in entries {
        let Ok(entry) = entry else { return false };
        if !entry
            .file_name()
            .as_encoded_bytes()
            .starts_with(b"i2c-OVTI02C1:")
        {
            continue;
        }
        let sensor = entry.path();
        match fs::symlink_metadata(sensor.join("driver")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && sensor.is_dir() => {
                found = true
            }
            _ => return false,
        }
    }
    found
}

fn probe_browser_camera(devices: &Path, libcamera_detected: bool) -> (CheckItem, bool) {
    if all_ov_sensors_unbound(devices) {
        return (
            CheckItem {
                title: "Navegador e comunicadores",
                detail: browser_sensor_unbound_detail(),
                health: Health::Error,
                code: BROWSER_SENSOR_UNBOUND,
            },
            false,
        );
    }
    let packages = package_presence(&[
        "v4l2-relayd",
        "v4l2loopback",
        "gstreamer1-plugins-icamerasrc",
        "v4l-utils",
    ]);
    let source_ready = detect_system_camera_source_ready();
    (
        detect_browser_camera_check(&packages, libcamera_detected, source_ready),
        source_ready,
    )
}

fn browser_sensor_unbound_detail() -> String {
    tr(
        "O sensor OV02C10 está presente, mas sem driver neste boot. Repare o driver da câmera e atualize o diagnóstico antes de ativar a webcam virtual.",
    )
}

fn detect_browser_camera_check(
    packages: &PackagePresence,
    libcamera_detected: bool,
    camera_source_ready: bool,
) -> CheckItem {
    if !packages.unknown.is_empty() {
        return CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Não foi possível confirmar o estado RPM de componentes do bridge V4L2: {packages}. O setup não os classificou como ausentes. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                &[("packages", packages.unknown.join(", "))],
            ),
            health: Health::Unknown,
            code: "browser-packages-query-unknown",
        };
    }

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

    match (
        relay_active,
        relay_enabled.as_deref(),
        loopback_device.as_deref(),
        loopback_capture,
    ) {
        (true, Some("enabled"), Some(device), true) => CheckItem {
            title: "Navegador e comunicadores",
            detail: trf(
                "Bridge V4L2 ativo em {device}, com capacidade Video Capture. Isso valida a integração e o dispositivo virtual, mas um teste real no app ainda é necessário para confirmar frames estáveis.",
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
            detail: tr(
                "A câmera já aparece nas fontes do sistema, mas a webcam virtual ainda não foi ativada. Use a ação rápida para expor a câmera como dispositivo V4L2 para Meet, Discord, Teams e outros apps WebRTC.",
            ),
            health: Health::Warning,
            code: "browser-system-source-ready",
        },
        _ if libcamera_detected => CheckItem {
            title: "Navegador e comunicadores",
            detail: tr(
                "A câmera base foi enumerada pelo libcamera, mas isso não garante captura estável e o bridge V4L2 para navegador ainda não foi ativado.",
            ),
            health: Health::Warning,
            code: "browser-libcamera-ready",
        },
        _ => CheckItem {
            title: "Navegador e comunicadores",
            detail: tr(
                "O bridge V4L2 para navegador ainda não foi ativado e a câmera também não apareceu nas fontes do sistema. Use a ação rápida para configurar a webcam virtual e reavalie o estado da câmera base se o problema persistir.",
            ),
            health: Health::Warning,
            code: "browser-missing",
        },
    }
}

fn libcamera_clock_probe_failed_check() -> CheckItem {
    CheckItem {
        title: "Caminho direto do Galaxy Book Câmera",
        detail: tr(
            "O boot registrou que o ov02c10 rejeitou o clock externo de 26 MHz, então o sensor não entrou no grafo de mídia usado pelo libcamera. Depois de uma atualização de kernel, isso costuma indicar que o módulo corrigido só ficou pronto depois da primeira tentativa do IPU6. Ajuste a prioridade do driver corrigido e reinicie para recriar o grafo com o sensor.",
        ),
        health: Health::Error,
        code: "libcamera-clock-probe-failed",
    }
}

fn detect_speakers_check(
    secure_boot_requires_signatures: bool,
    expected_module_identity: Option<&AkmodsKeyIdentity>,
    kernels: &[String],
    kernel_inventory_complete: bool,
    packages: &PackagePresence,
) -> CheckItem {
    detect_speakers_check_with_hardware(
        max98390_hardware_discovery(),
        secure_boot_requires_signatures,
        expected_module_identity,
        kernels,
        kernel_inventory_complete,
        packages,
    )
}

fn detect_speakers_check_with_hardware(
    hardware: HardwareDiscovery,
    secure_boot_requires_signatures: bool,
    expected_module_identity: Option<&AkmodsKeyIdentity>,
    kernels: &[String],
    kernel_inventory_complete: bool,
    packages: &PackagePresence,
) -> CheckItem {
    match hardware {
        HardwareDiscovery::Absent => {
            return CheckItem {
                title: "Alto-falantes internos",
                detail: tr(
                    "Este sistema não expôs amplificadores MAX98390 via ACPI ou I2C, então o setup não aplicou o fluxo específico dos alto-falantes Galaxy Book.",
                ),
                health: Health::Unknown,
                code: "speakers-unsupported",
            };
        }
        HardwareDiscovery::Unknown => {
            return CheckItem {
                title: "Alto-falantes internos",
                detail: tr(
                    "O setup não conseguiu concluir a leitura do hardware MAX98390 via ACPI e I2C. O estado dos módulos de alto-falante permaneceu desconhecido e o reboot específico dos drivers foi bloqueado.",
                ),
                health: Health::Unknown,
                code: "speakers-hardware-query-unknown",
            };
        }
        HardwareDiscovery::Present => {}
    }

    if !packages.unknown.is_empty() {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "O hardware MAX98390 foi detectado, mas a consulta RPM não confirmou o estado de: {packages}. O setup não tratou esses pacotes como ausentes e bloqueou o reboot específico dos drivers. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                &[("packages", packages.unknown.join(", "))],
            ),
            health: Health::Unknown,
            code: "speakers-packages-query-unknown",
        };
    }

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

    let core_payload_inventory = module_payload_inventory(
        kernels,
        "kmod-galaxybook-max98390",
        "snd-hda-scodec-max98390",
        "/extra/galaxybook-max98390/",
        secure_boot_requires_signatures,
        expected_module_identity,
    );
    let i2c_payload_inventory = module_payload_inventory(
        kernels,
        "kmod-galaxybook-max98390",
        "snd-hda-scodec-max98390-i2c",
        "/extra/galaxybook-max98390/",
        secure_boot_requires_signatures,
        expected_module_identity,
    );
    let mut invalid_kernels = core_payload_inventory.invalid_kernels;
    invalid_kernels.extend(i2c_payload_inventory.invalid_kernels);
    invalid_kernels.sort();
    invalid_kernels.dedup();
    let mut unknown_rpm_kernels = core_payload_inventory.unknown_rpm_kernels;
    unknown_rpm_kernels.extend(i2c_payload_inventory.unknown_rpm_kernels);
    unknown_rpm_kernels.sort();
    unknown_rpm_kernels.dedup();

    if !unknown_rpm_kernels.is_empty() {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "A consulta RPM dos payloads MAX98390 por kernel não terminou de forma conclusiva em: {kernels}. O setup não classificou esses payloads como ausentes e bloqueou o reboot específico dos drivers. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                &[("kernels", unknown_rpm_kernels.join(", "))],
            ),
            health: Health::Unknown,
            code: "speakers-payload-package-query-unknown",
        };
    }

    if !kernel_inventory_complete {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: tr(
                "Não foi possível enumerar todos os pacotes kernel-core instalados. O setup não declarou os payloads MAX98390 prontos porque kernels desconhecidos ainda não foram validados.",
            ),
            health: Health::Error,
            code: "speakers-kernel-payload-invalid",
        };
    }

    if !invalid_kernels.is_empty() {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "O akmods pode ter terminado com sucesso, mas faltam os dois payloads MAX98390 externos com dono RPM e vermagic correto por kernel em: {kernels}.",
                &[("kernels", invalid_kernels.join(", "))],
            ),
            health: Health::Error,
            code: "speakers-kernel-payload-invalid",
        };
    }

    let setup_service = "max98390-hda-i2c-setup.service";
    if systemd_unit_enabled_state(setup_service).as_deref() != Some("enabled") {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: tr(
                "Os payloads MAX98390 estão prontos, mas o serviço de I2C não está habilitado de forma persistente para o boot.",
            ),
            health: Health::Warning,
            code: "speakers-service-disabled",
        };
    }

    let setup_result = systemd_unit_property(setup_service, "Result");
    if setup_result.as_deref() != Some("success") || !systemd_unit_is_active(setup_service) {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "Os payloads MAX98390 estão prontos, mas o serviço de I2C não terminou com estado ativo e resultado success (resultado: {result}).",
                &[(
                    "result",
                    setup_result.unwrap_or_else(|| "desconhecido".into()),
                )],
            ),
            health: Health::Error,
            code: "speakers-service-failed",
        };
    }

    let binding_count = max98390_binding_count();
    if binding_count < 4 {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "O serviço terminou, mas somente {count} dos 4 vínculos MAX98390 esperados estão ativos. Um EPROBE_DEFER transitório só é ignorado quando os quatro vínculos aparecem ao final.",
                &[("count", binding_count.to_string())],
            ),
            health: Health::Error,
            code: "speakers-bindings-missing",
        };
    }

    let modules = read_trimmed("/proc/modules").unwrap_or_default();
    let core_loaded = modules
        .lines()
        .any(|line| line.starts_with("snd_hda_scodec_max98390 "));
    let i2c_loaded = modules
        .lines()
        .any(|line| line.starts_with("snd_hda_scodec_max98390_i2c "));
    if core_loaded && i2c_loaded {
        return CheckItem {
            title: "Alto-falantes internos",
            detail: trf(
                "Payloads RPM validados para {kernels}; módulos carregados, serviço I2C concluído e 4 vínculos MAX98390 ativos.",
                &[("kernels", kernels.join(", "))],
            ),
            health: Health::Good,
            code: "speakers-ready",
        };
    }

    CheckItem {
        title: "Alto-falantes internos",
        detail: tr(
            "Os payloads MAX98390 estão prontos para todos os kernels instalados, mas os módulos do kernel em execução ainda não estão carregados. Reinicie normalmente para usar os payloads empacotados.",
        ),
        health: Health::Warning,
        code: "speakers-modules-unloaded",
    }
}

fn module_origin_from_path(path: Option<&str>) -> ModuleOrigin {
    match path {
        Some(path)
            if path.contains("/extra/galaxybook-ov02c10/")
                || path.contains("/updates/ov02c10.") =>
        {
            ModuleOrigin::Patched
        }
        Some(path) if path.contains("/kernel/") => ModuleOrigin::InTree,
        Some(_) => ModuleOrigin::Unknown,
        None => ModuleOrigin::Missing,
    }
}

fn detect_clock_error(kernel_log: &str) -> bool {
    kernel_log.contains("external clock 26000000 is not supported")
        || kernel_log.contains("probe with driver ov02c10 failed with error -22")
}

fn classify_kernel_journal_probe(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> KernelJournalProbe {
    if exit_code != Some(0) || !stderr.is_empty() || stdout.is_empty() || !stdout.ends_with(b"\n") {
        return KernelJournalProbe::Unknown;
    }
    let Ok(kernel_log) = std::str::from_utf8(stdout) else {
        return KernelJournalProbe::Unknown;
    };
    if kernel_log.contains('\0') {
        return KernelJournalProbe::Unknown;
    }
    if detect_clock_error(kernel_log) {
        KernelJournalProbe::ClockError
    } else if kernel_log
        .lines()
        .any(|line| line.starts_with("Linux version "))
    {
        KernelJournalProbe::Clean
    } else {
        // The boot banner proves that the early kernel messages are accessible.
        KernelJournalProbe::Unknown
    }
}

fn probe_kernel_journal() -> KernelJournalProbe {
    let Ok(output) = probe_output(
        "journalctl",
        &[
            "-b",
            "-k",
            "--no-pager",
            "--output=cat",
            "--grep=^Linux version |external clock 26000000 is not supported|probe with driver ov02c10 failed with error -22",
        ],
        true,
    ) else {
        return KernelJournalProbe::Unknown;
    };
    classify_kernel_journal_probe(output.status.code(), &output.stdout, &output.stderr)
}

fn boot_check_from_kernel_log(kernel_log: KernelJournalProbe) -> CheckItem {
    match kernel_log {
        KernelJournalProbe::Unknown => CheckItem {
            title: "Erros no boot",
            detail: tr(
                "O setup não conseguiu consultar o journal do kernel deste boot dentro do prazo, então não afirmou que o boot está livre de erros do OV02C10.",
            ),
            health: Health::Unknown,
            code: "boot-log-unavailable",
        },
        KernelJournalProbe::ClockError => CheckItem {
            title: "Erros no boot",
            detail: tr(
                "O boot registrou que o driver in-tree não suporta o clock externo de 26 MHz nesta máquina.",
            ),
            health: Health::Error,
            code: "boot-clock-error",
        },
        KernelJournalProbe::Clean => CheckItem {
            title: "Erros no boot",
            detail: tr("Nenhum erro de clock/probe do ov02c10 foi encontrado no boot atual."),
            health: Health::Good,
            code: "boot-ok",
        },
    }
}

fn classify_max98390_identifier_scan(
    identifiers: Result<Vec<Result<String, ()>>, ()>,
) -> HardwareDiscovery {
    let Ok(identifiers) = identifiers else {
        return HardwareDiscovery::Unknown;
    };
    let mut present = false;
    for identifier in identifiers {
        let Ok(identifier) = identifier else {
            return HardwareDiscovery::Unknown;
        };
        present |= max98390_identifier_matches(&identifier);
    }
    if present {
        HardwareDiscovery::Present
    } else {
        HardwareDiscovery::Absent
    }
}

fn acpi_device_identifiers(directory: &Path) -> Result<Vec<Result<String, ()>>, ()> {
    fs::read_dir(directory).map_err(|_| ()).map(|entries| {
        entries
            .map(|entry| {
                entry
                    .map_err(|_| ())?
                    .file_name()
                    .into_string()
                    .map_err(|_| ())
            })
            .collect()
    })
}

fn i2c_device_identifiers(directory: &Path) -> Result<Vec<Result<String, ()>>, ()> {
    fs::read_dir(directory).map_err(|_| ()).map(|entries| {
        entries
            .map(|entry| {
                let entry = entry.map_err(|_| ())?;
                fs::read_to_string(entry.path().join("name")).map_err(|_| ())
            })
            .collect()
    })
}

fn max98390_hardware_discovery_in(
    acpi_directory: &Path,
    i2c_directory: &Path,
) -> HardwareDiscovery {
    match classify_max98390_identifier_scan(acpi_device_identifiers(acpi_directory)) {
        HardwareDiscovery::Present => HardwareDiscovery::Present,
        HardwareDiscovery::Unknown => HardwareDiscovery::Unknown,
        HardwareDiscovery::Absent => {
            classify_max98390_identifier_scan(i2c_device_identifiers(i2c_directory))
        }
    }
}

fn max98390_hardware_discovery() -> HardwareDiscovery {
    max98390_hardware_discovery_in(
        Path::new("/sys/bus/acpi/devices"),
        Path::new("/sys/bus/i2c/devices"),
    )
}

fn max98390_identifier_matches(identifier: &str) -> bool {
    let normalized = identifier.to_ascii_uppercase();
    normalized.contains("MAX98390") || normalized.contains("MX98390")
}

fn max_driver_state_allows_reboot(driver_present: bool, speakers_code: &str) -> bool {
    speakers_code != "speakers-hardware-query-unknown"
        && (!driver_present
            || !matches!(
                speakers_code,
                "speakers-packages-missing"
                    | "speakers-packages-query-unknown"
                    | "speakers-payload-package-query-unknown"
                    | "speakers-kernel-payload-invalid"
                    | "speakers-modules-missing"
            ))
}

fn detect_sound_app_check(sound_app_presence: RpmPackagePresence) -> CheckItem {
    match sound_app_presence {
        RpmPackagePresence::Installed => CheckItem {
            title: "Galaxy Book Sound",
            detail: tr(
                "O painel de som está instalado e pronto para equalizador, perfis e Atmos compatível via PipeWire.",
            ),
            health: Health::Good,
            code: "sound-app-ready",
        },
        RpmPackagePresence::Absent => CheckItem {
            title: "Galaxy Book Sound",
            detail: tr(
                "O painel de som ainda não está instalado. Use-o para equalizador, perfis prontos e Atmos compatível depois que o MAX98390 estiver funcional.",
            ),
            health: Health::Unknown,
            code: "sound-app-missing",
        },
        RpmPackagePresence::Unknown => CheckItem {
            title: "Galaxy Book Sound",
            detail: tr(
                "A consulta RPM não confirmou se o Galaxy Book Sound está instalado. O setup não classificou o painel como ausente. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
            ),
            health: Health::Unknown,
            code: "sound-app-package-query-unknown",
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
        unknown_packages: packages.unknown,
        authselect_enabled,
        list_state,
    }
}

fn detect_fingerprint_reader_check(context: &FingerprintContext) -> CheckItem {
    let Some(sensor_line) = context.sensor_line.as_ref() else {
        return CheckItem {
            title: "Leitor de digital",
            detail: tr(
                "Nenhum leitor de digital compatível foi detectado via lsusb neste boot. Em modelos sem sensor integrado, esse estado é esperado.",
            ),
            health: Health::Unknown,
            code: "fingerprint-reader-missing",
        };
    };

    if !context.unknown_packages.is_empty() {
        return CheckItem {
            title: "Leitor de digital",
            detail: trf(
                "Sensor detectado em {sensor}, mas a consulta RPM não confirmou o estado de: {packages}. O setup não classificou esses pacotes como ausentes. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
                &[
                    ("sensor", sensor_line.to_string()),
                    ("packages", context.unknown_packages.join(", ")),
                ],
            ),
            health: Health::Unknown,
            code: "fingerprint-packages-query-unknown",
        };
    }

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
            detail: tr(
                "Sem leitor de digital detectado neste boot, o setup não avaliou cadastro nem integração com authselect.",
            ),
            health: Health::Unknown,
            code: "fingerprint-login-unavailable",
        };
    }

    if !context.unknown_packages.is_empty() {
        return CheckItem {
            title: "Login por digital",
            detail: tr(
                "O leitor foi detectado, mas a consulta RPM do stack de fingerprint não terminou de forma conclusiva. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
            ),
            health: Health::Unknown,
            code: "fingerprint-login-packages-query-unknown",
        };
    }

    if !context.missing_packages.is_empty() {
        return CheckItem {
            title: "Login por digital",
            detail: tr(
                "O leitor foi detectado, mas o stack de fingerprint ainda não está completo. Reinstale fprintd/libfprint antes de validar cadastro e login.",
            ),
            health: Health::Warning,
            code: "fingerprint-login-stack-missing",
        };
    }

    match (context.authselect_enabled, context.list_state) {
        (true, FingerprintEnrollmentState::Enrolled) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "Authselect já está com with-fingerprint ativo e o usuário atual tem pelo menos uma digital cadastrada.",
            ),
            health: Health::Good,
            code: "fingerprint-login-ready",
        },
        (true, FingerprintEnrollmentState::NotEnrolled) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "O stack já responde e o authselect está pronto, mas o usuário atual ainda não cadastrou nenhuma digital.",
            ),
            health: Health::Warning,
            code: "fingerprint-enrollment-missing",
        },
        (false, FingerprintEnrollmentState::Enrolled) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "Há digital cadastrada para o usuário atual, mas o authselect ainda não está com with-fingerprint habilitado.",
            ),
            health: Health::Warning,
            code: "fingerprint-auth-disabled",
        },
        (false, FingerprintEnrollmentState::NotEnrolled) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "O leitor já responde, mas ainda faltam dois passos: habilitar with-fingerprint no authselect e cadastrar a digital do usuário atual.",
            ),
            health: Health::Warning,
            code: "fingerprint-auth-and-enrollment-missing",
        },
        (_, FingerprintEnrollmentState::Busy) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "O leitor respondeu como ocupado. Reaplique o stack e depois abra o cadastro de usuários para refazer a digital com o sensor liberado.",
            ),
            health: Health::Warning,
            code: "fingerprint-login-busy",
        },
        (_, FingerprintEnrollmentState::NoDevice) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "O sensor apareceu no USB, mas o fprintd não expôs um dispositivo utilizável para validar cadastro e autenticação nesta sessão.",
            ),
            health: Health::Warning,
            code: "fingerprint-login-no-device",
        },
        (_, FingerprintEnrollmentState::Unavailable) => CheckItem {
            title: "Login por digital",
            detail: tr(
                "O leitor foi detectado, mas o setup não conseguiu validar cadastro e authselect nesta sessão.",
            ),
            health: Health::Unknown,
            code: "fingerprint-login-unavailable",
        },
    }
}

fn recommend_next_step(
    secure_boot_key: &CheckItem,
    packages: &PackagePresence,
    akmods_payload_missing: bool,
    module_origin: ModuleOrigin,
    manual_updates_override: bool,
    clock_error: bool,
    module_loaded: bool,
    running_module_matches_candidate: bool,
    libcamera_detected: bool,
    libcamera_permission_blocked: bool,
    camera_source_ready: bool,
    browser_camera_code: &str,
    camera_app_installed: bool,
    speaker_supported: bool,
    speaker_ready: bool,
    sound_app_installed: bool,
) -> (String, String) {
    if matches!(
        secure_boot_key.code,
        "mok-key-protected" | "mok-legacy-identity" | "mok-key-identity-unverified"
    ) {
        return (
            tr("Verifique a chave do Secure Boot"),
            tr(
                "O processo sem privilégios não pode ler os diretórios protegidos do akmods e, por isso, não afirmou que a chave está ausente. Use a ação administrativa para fazer a validação definitiva antes de carregar módulos externos.",
            ),
        );
    }

    if matches!(secure_boot_key.code, "mok-key-missing" | "mok-not-enrolled") {
        return (
            tr("Prepare a chave do Secure Boot"),
            tr(
                "O Secure Boot está ativo, mas a chave usada pelo akmods ainda não está pronta ou inscrita no MOK. Use a ação rápida dedicada antes de instalar ou carregar módulos externos do notebook.",
            ),
        );
    }

    if secure_boot_key.code == "mok-pending-enrollment" {
        return (
            tr("Concluir inscrição do MOK"),
            tr(
                "O pedido de importação da chave do akmods já foi criado. Reinicie o notebook e conclua 'Enroll MOK' na tela azul do boot antes de repetir as ações do driver.",
            ),
        );
    }

    if !packages.unknown.is_empty() {
        return (
            tr("Consulta de pacotes inconclusiva"),
            tr(
                "O RPM não confirmou o estado de todos os pacotes principais. O setup não os tratou como ausentes e manteve o reboot específico dos drivers bloqueado. Aguarde o fim de outra operação RPM/DNF e atualize o diagnóstico; se persistir, verifique a integridade da rpmdb.",
            ),
        );
    }

    if !packages.missing.is_empty() {
        return (
            tr("Instalação pendente"),
            tr(
                "Instale os pacotes principais da câmera pela própria seção de ações rápidas, reinicie o sistema e atualize o diagnóstico.",
            ),
        );
    }

    if akmods_payload_missing {
        return (
            tr("O driver não foi gerado no boot"),
            tr(
                "O estado final ainda não possui o payload kmod externo e pertencente ao RPM para todos os kernels instalados. Reexecute o reparo do driver e só reinicie depois que a reconciliação terminar sem erros.",
            ),
        );
    }

    if clock_error && !libcamera_detected {
        if module_origin == ModuleOrigin::InTree {
            return (
                tr("O sistema caiu para o driver in-tree"),
                tr(
                    "O boot registrou que o ov02c10 carregado foi o do kernel, que não suporta o clock de 26 MHz deste hardware. Ajuste a prioridade do driver corrigido pela seção de ações rápidas e reinicie.",
                ),
            );
        }

        return (
            tr("Reinicie com o driver corrigido priorizado"),
            tr(
                "O boot registrou que o ov02c10 rejeitou o clock externo de 26 MHz antes da câmera entrar no grafo de mídia. Depois de atualização de kernel, isso costuma acontecer quando o akmods só termina de gerar o módulo corrigido após a tentativa inicial do IPU6. Ajuste a prioridade do driver corrigido pela seção de ações rápidas e reinicie.",
            ),
        );
    }

    if module_origin == ModuleOrigin::InTree && clock_error {
        return (
            tr("O sistema caiu para o driver in-tree"),
            tr(
                "O boot registrou que o ov02c10 carregado foi o do kernel, que não suporta o clock de 26 MHz deste hardware. Ajuste a prioridade do driver corrigido pela seção de ações rápidas e reinicie.",
            ),
        );
    }

    if module_origin == ModuleOrigin::Patched && !module_loaded {
        return (
            tr("O driver corrigido não foi carregado"),
            tr(
                "O módulo OV02C10 corrigido está instalado, mas não entrou no kernel em execução. Valide os payloads e initramfs de todos os kernels pela ação rápida e reinicie normalmente para recriar o grafo de mídia; o setup não troca ao vivo um módulo ocupado.",
            ),
        );
    }

    if module_origin == ModuleOrigin::Patched && !running_module_matches_candidate {
        return (
            tr("Confirme o driver carregado após reiniciar"),
            tr(
                "O rootfs resolve o payload OV02C10 externo, mas o setup não conseguiu correlacioná-lo de forma forte com o binário já carregado. Valide os payloads e reinicie normalmente; o setup não substitui ao vivo um módulo ocupado.",
            ),
        );
    }

    if browser_camera_code == BROWSER_SENSOR_UNBOUND {
        return (tr("Reparar o driver"), browser_sensor_unbound_detail());
    }

    if libcamera_permission_blocked {
        return (
            tr("O bridge do navegador bloqueou o libcamera"),
            tr(
                "A regra UDev administrativa legada /etc/udev/rules.d/90-hide-ipu6-v4l2.rules removeu o acesso do usuário aos nós crus do IPU6. O setup preserva essa regra porque não pode removê-la automaticamente com segurança. Revise-a e, se apropriado, remova-a manualmente como administrador; depois execute novamente a ação da câmera para navegador.",
            ),
        );
    }

    if manual_updates_override && !libcamera_detected {
        return (
            tr("Migre o override manual da câmera"),
            tr(
                "O sistema ainda está usando um ov02c10 manual em /updates. Use a ação de prioridade do driver para atualizar o pacote OV02C10, remover o override órfão e voltar ao kmod gerenciado por RPM.",
            ),
        );
    }

    if browser_camera_code != "browser-ready" && (libcamera_detected || camera_source_ready) {
        return (
            tr("Compatibilidade com navegador pendente"),
            tr(
                "A câmera base foi enumerada no caminho direto ou nas fontes do sistema, sem que isso garanta frames estáveis. A webcam virtual para Meet, Discord, Teams e outros apps ainda não está ativa; use a ação rápida para expô-la como dispositivo V4L2 e faça um teste real no app.",
            ),
        );
    }

    if !libcamera_detected && !camera_source_ready {
        return (
            tr("A câmera ainda não apareceu no caminho direto do app"),
            tr(
                "O driver e os pacotes principais parecem presentes, mas a câmera ainda não foi detectada nem no caminho direto do Galaxy Book Câmera nem nas fontes do sistema. O próximo passo é revisar os logs do boot e a pilha IPU6.",
            ),
        );
    }

    if !camera_app_installed {
        return (
            tr("Driver pronto, app da câmera ausente"),
            tr(
                "A câmera já aparece no caminho direto do Galaxy Book Câmera. Instale o app para validar preview, foto e vídeo no fluxo final.",
            ),
        );
    }

    if speaker_supported && !speaker_ready {
        return (
            tr("Suporte dos alto-falantes pendente"),
            tr(
                "A máquina expõe amplificadores MAX98390, mas o pacote de speakers ainda não está pronto ou os módulos não foram carregados. Ative o suporte dos alto-falantes internos pela seção de ações rápidas e teste a saída Speaker novamente.",
            ),
        );
    }

    if speaker_ready && !sound_app_installed {
        return (
            tr("Painel de som opcional ainda não instalado"),
            tr(
                "O suporte MAX98390 já parece pronto. Se você quiser equalizador, perfis e Atmos compatível, instale o Galaxy Book Sound pela seção de ações rápidas.",
            ),
        );
    }

    (
        tr("Fluxo principal da câmera parece pronto"),
        tr(
            "O módulo corrigido parece ativo, o caminho direto do Galaxy Book Câmera já vê a câmera e o app final está instalado. Se os alto-falantes também já estiverem prontos, o próximo passo é abrir o Galaxy Book Câmera ou o Galaxy Book Sound e validar o fluxo final.",
        ),
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
    let output = probe_output(command, args, false)?;
    if !output.status.success() {
        return Err(());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn command_text(command: &str, args: &[&str]) -> Result<String, ()> {
    let output = probe_output(command, args, false)?;
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

fn classify_clean_command_text(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<String, ()> {
    if exit_code != Some(0) || !stderr.is_empty() {
        return Err(());
    }
    let stdout = std::str::from_utf8(stdout).map_err(|_| ())?;
    let value = stdout.strip_suffix('\n').unwrap_or(stdout);
    if value.is_empty()
        || value != value.trim()
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('\0')
    {
        return Err(());
    }
    Ok(value.to_string())
}

fn clean_command_text(command: &str, args: &[&str]) -> Result<String, ()> {
    let output = probe_output(command, args, false)?;
    classify_clean_command_text(output.status.code(), &output.stdout, &output.stderr)
}

fn classify_kernel_release_query(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<String> {
    if exit_code != Some(0) || !stderr.is_empty() {
        return None;
    }
    let output = std::str::from_utf8(stdout).ok()?;
    let kernel = output.strip_suffix('\n')?;
    if kernel.contains('\n')
        || kernel.contains('\r')
        || kernel.contains('\0')
        || !kernel_release_is_safe(kernel)
    {
        return None;
    }
    Some(kernel.to_string())
}

fn running_kernel_release() -> Option<String> {
    let output = probe_output("uname", &["-r"], true).ok()?;
    classify_kernel_release_query(output.status.code(), &output.stdout, &output.stderr)
}

fn classify_strict_success_stdout<'a>(
    exit_code: Option<i32>,
    stdout: &'a [u8],
    stderr: &[u8],
) -> Option<&'a str> {
    (exit_code == Some(0) && stderr.is_empty())
        .then(|| std::str::from_utf8(stdout).ok())
        .flatten()
}

fn strict_success_stdout(output: &std::process::Output) -> Option<&str> {
    classify_strict_success_stdout(output.status.code(), &output.stdout, &output.stderr)
}

fn probe_command_arguments(command: &str, args: &[&str]) -> Vec<OsString> {
    let mut bounded_args = Vec::with_capacity(args.len() + 4);
    bounded_args.push(OsString::from(format!(
        "--kill-after={PROBE_TIMEOUT_KILL_AFTER}"
    )));
    bounded_args.push(OsString::from(PROBE_TIMEOUT_DURATION));
    bounded_args.push(OsString::from(command));
    bounded_args.extend(args.iter().map(OsString::from));
    bounded_args
}

fn probe_output(command: &str, args: &[&str], c_locale: bool) -> Result<std::process::Output, ()> {
    let mut probe = Command::new(PROBE_TIMEOUT_COMMAND);
    probe.args(probe_command_arguments(command, args));
    if c_locale {
        probe.env("LC_ALL", "C");
    }
    probe.output().map_err(|_| ())
}

fn probe_output_with_stdin(
    command: &str,
    args: &[&str],
    c_locale: bool,
    input: &[u8],
) -> Result<std::process::Output, ()> {
    let mut probe = Command::new(PROBE_TIMEOUT_COMMAND);
    probe
        .args(probe_command_arguments(command, args))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if c_locale {
        probe.env("LC_ALL", "C");
    }

    let mut child = probe.spawn().map_err(|_| ())?;
    let write_result = child
        .stdin
        .take()
        .ok_or(())
        .and_then(|mut stdin| std::io::Write::write_all(&mut stdin, input).map_err(|_| ()));
    if write_result.is_err() {
        // The timeout wrapper remains responsible for terminating its child;
        // killing only the wrapper could orphan the actual probe.
        let _ = child.wait_with_output();
        return Err(());
    }
    child.wait_with_output().map_err(|_| ())
}

fn probe_status(command: &str, args: &[&str], c_locale: bool) -> bool {
    probe_output(command, args, c_locale)
        .map(|output| output.status.code() == Some(0) && output.stderr.is_empty())
        .unwrap_or(false)
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
    output
        .lines()
        .any(|line| line.trim() == "- with-fingerprint")
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

    probe_status("systemctl", &["is-active", "--quiet", unit], false)
}

fn systemd_unit_property(unit: &str, property: &str) -> Option<String> {
    if !systemd_available() {
        return None;
    }
    clean_command_text(
        "systemctl",
        &["show", unit, "--property", property, "--value"],
    )
    .ok()
    .filter(|value| !value.is_empty())
}

fn classify_akmods_build_query(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<bool> {
    if exit_code != Some(0) || !stderr.is_empty() {
        return None;
    }
    let output = std::str::from_utf8(stdout).ok()?;
    if output.is_empty() {
        Some(false)
    } else if output.trim().is_empty() {
        None
    } else {
        Some(true)
    }
}

fn akmods_build_active() -> Option<bool> {
    let output = probe_output(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--state=activating,running",
            "--plain",
            "--no-legend",
            "akmods.service",
            "akmods@*.service",
        ],
        false,
    )
    .ok()?;
    classify_akmods_build_query(output.status.code(), &output.stdout, &output.stderr)
}

fn global_reboot_ready(akmods_build_active: Option<bool>) -> bool {
    matches!(akmods_build_active, Some(false))
}

fn max98390_binding_count() -> usize {
    fs::read_dir("/sys/bus/i2c/drivers/max98390-hda")
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry.file_name() != "module"
                        && entry
                            .file_type()
                            .map(|kind| kind.is_symlink())
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
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

    clean_command_text("systemctl", &["is-enabled", unit]).ok()
}

fn systemd_available() -> bool {
    if !Path::new("/run/systemd/system").exists() {
        return false;
    }

    probe_status("systemctl", &["--version"], false)
}

fn rpm_package_name_is_allowed(package: &str) -> bool {
    RPM_PACKAGE_QUERY_ALLOWLIST.contains(&package)
        || RPM_DYNAMIC_KMOD_QUERY_PREFIXES.iter().any(|prefix| {
            package
                .strip_prefix(prefix)
                .is_some_and(kernel_release_is_safe)
        })
}

fn classify_rpm_package_query(
    package: &str,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> RpmPackagePresence {
    if exit_code == Some(0) && stderr.is_empty() {
        let Ok(stdout) = std::str::from_utf8(stdout) else {
            return RpmPackagePresence::Unknown;
        };
        if !stdout.is_empty()
            && stdout.ends_with('\n')
            && stdout
                .split_terminator('\n')
                .all(|reported_name| reported_name == package)
        {
            return RpmPackagePresence::Installed;
        }
    }

    let expected_absent = format!("package {package} is not installed\n");
    if exit_code == Some(1) && stdout == expected_absent.as_bytes() && stderr.is_empty() {
        RpmPackagePresence::Absent
    } else {
        RpmPackagePresence::Unknown
    }
}

fn rpm_package_query_arguments(package: &str) -> Option<[&str; 5]> {
    rpm_package_name_is_allowed(package).then_some(["-q", "--qf", "%{NAME}\n", "--", package])
}

fn rpm_package_presence(package: &str) -> RpmPackagePresence {
    let Some(arguments) = rpm_package_query_arguments(package) else {
        return RpmPackagePresence::Unknown;
    };

    let Ok(output) = probe_output("rpm", &arguments, true) else {
        return RpmPackagePresence::Unknown;
    };
    classify_rpm_package_query(
        package,
        output.status.code(),
        &output.stdout,
        &output.stderr,
    )
}

fn parse_kernel_releases(current: &str, rpm_output: &str) -> Vec<String> {
    let mut kernels = rpm_output
        .lines()
        .chain(std::iter::once(current))
        .map(str::trim)
        .filter(|kernel| !kernel.is_empty() && *kernel != "Desconhecido")
        .map(str::to_string)
        .collect::<Vec<_>>();
    kernels.sort();
    kernels.dedup();
    kernels
}

fn installed_kernel_releases(current: &str) -> Option<Vec<String>> {
    let output = probe_output(
        "rpm",
        &[
            "-q",
            "--qf",
            "%{VERSION}-%{RELEASE}.%{ARCH}\n",
            "--",
            "kernel-core",
        ],
        true,
    )
    .ok()?;
    classify_installed_kernel_inventory(
        current,
        output.status.code(),
        &output.stdout,
        &output.stderr,
    )
}

fn classify_installed_kernel_inventory(
    current: &str,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<Vec<String>> {
    if exit_code != Some(0) || !stderr.is_empty() || !kernel_release_is_safe(current) {
        return None;
    }
    let output = std::str::from_utf8(stdout).ok()?;
    if output.is_empty()
        || !output.ends_with('\n')
        || !output
            .split_terminator('\n')
            .all(|kernel| kernel_release_is_safe(kernel))
    {
        return None;
    }
    complete_kernel_inventory(current, Some(output))
}

fn complete_kernel_inventory(current: &str, rpm_output: Option<&str>) -> Option<Vec<String>> {
    let kernels = parse_kernel_releases(current, rpm_output?);
    (!kernels.is_empty()).then_some(kernels)
}

fn module_path_for_kernel(kernel: &str, module: &str) -> Option<String> {
    clean_command_text("modinfo", &["-k", kernel, "-n", module]).ok()
}

fn module_payload_matches(
    package: &str,
    path: Option<&str>,
    owner: Option<&str>,
    expected_directory: &str,
) -> bool {
    matches!(
        (path, owner),
        (Some(path), Some(owner))
            if path.contains(expected_directory) && owner == package
    )
}

fn manual_updates_override_is_proven(path: &str, owner: &RpmFileOwner) -> bool {
    path.contains("/updates/") && owner == &RpmFileOwner::Unowned
}

fn module_vermagic_matches(kernel: &str, vermagic: Option<&str>) -> bool {
    vermagic.and_then(|value| value.split_whitespace().next()) == Some(kernel)
}

fn module_security_metadata_matches(
    kernel: &str,
    vermagic: Option<&str>,
    signer: Option<&str>,
    sig_key: Option<&str>,
    secure_boot_requires_signature: bool,
    expected_identity: Option<&AkmodsKeyIdentity>,
) -> bool {
    module_vermagic_matches(kernel, vermagic)
        && (!secure_boot_requires_signature
            || expected_identity.is_some_and(|identity| {
                signer.map(str::trim) == Some(identity.cn.as_str())
                    && normalize_certificate_serial(sig_key.unwrap_or_default()).as_deref()
                        == Some(identity.certificate_serial.as_str())
            }))
}

fn secure_boot_state_requires_signatures(state: Option<bool>) -> bool {
    state != Some(false)
}

fn kernel_module_payload_is_valid(
    kernel: &str,
    package_prefix: &str,
    module: &str,
    expected_directory: &str,
    secure_boot_requires_signature: bool,
    expected_identity: Option<&AkmodsKeyIdentity>,
) -> Option<bool> {
    let package = format!("{package_prefix}-{kernel}");
    match rpm_package_presence(&package) {
        RpmPackagePresence::Installed => {}
        RpmPackagePresence::Absent => return Some(false),
        RpmPackagePresence::Unknown => return None,
    }

    let Some(path) = module_path_for_kernel(kernel, module) else {
        return Some(false);
    };
    let owner = rpm_owner_for_file(&path);
    if owner == RpmFileOwner::Unknown {
        return None;
    }
    let vermagic = clean_command_text("modinfo", &["-F", "vermagic", &path]).ok();
    let signer = clean_command_text("modinfo", &["-k", kernel, "-F", "signer", module]).ok();
    let sig_key = clean_command_text("modinfo", &["-k", kernel, "-F", "sig_key", module]).ok();
    Some(
        module_payload_matches(
            &package,
            Some(&path),
            owner.owned_name(),
            expected_directory,
        ) && module_security_metadata_matches(
            kernel,
            vermagic.as_deref(),
            signer.as_deref(),
            sig_key.as_deref(),
            secure_boot_requires_signature,
            expected_identity,
        ),
    )
}

fn module_payload_inventory(
    kernels: &[String],
    package_prefix: &str,
    module: &str,
    expected_directory: &str,
    secure_boot_requires_signature: bool,
    expected_identity: Option<&AkmodsKeyIdentity>,
) -> ModulePayloadInventory {
    let mut inventory = ModulePayloadInventory::default();
    for kernel in kernels {
        match kernel_module_payload_is_valid(
            kernel,
            package_prefix,
            module,
            expected_directory,
            secure_boot_requires_signature,
            expected_identity,
        ) {
            Some(true) => {}
            Some(false) => inventory.invalid_kernels.push(kernel.clone()),
            None => inventory.unknown_rpm_kernels.push(kernel.clone()),
        }
    }
    inventory
}

fn camera_payload_state_allows_reboot(
    driver_present: bool,
    driver_complete: bool,
    invalid_kernels: &[String],
    readiness_attestation_current: bool,
) -> bool {
    !driver_present
        || (driver_complete && invalid_kernels.is_empty() && readiness_attestation_current)
}

fn driver_reboot_state_allows_reboot(
    camera_inventory_complete: bool,
    camera_ready: bool,
    max_inventory_complete: bool,
    max_ready: bool,
) -> bool {
    camera_inventory_complete && camera_ready && max_inventory_complete && max_ready
}

fn kernel_release_is_safe(kernel: &str) -> bool {
    !kernel.is_empty()
        && kernel.len() <= 128
        && kernel.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && kernel
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !kernel.contains("..")
        && kernel
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
}

fn parse_find_timestamp(value: &str) -> Option<(i64, i64)> {
    let (seconds, fraction) = value.split_once('.')?;
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let seconds = seconds.parse().ok()?;
    let mut nanoseconds = fraction.chars().take(9).collect::<String>();
    while nanoseconds.len() < 9 {
        nanoseconds.push('0');
    }
    if fraction.chars().skip(9).any(|character| character != '0') {
        return None;
    }
    Some((seconds, nanoseconds.parse().ok()?))
}

fn parse_file_fingerprint(value: &str) -> Option<FileFingerprint> {
    let mut fields = value.split('|');
    let device = fields.next()?.parse().ok()?;
    let inode = fields.next()?.parse().ok()?;
    let size = fields.next()?.parse().ok()?;
    let (mtime_seconds, mtime_nanoseconds) = parse_find_timestamp(fields.next()?)?;
    let (ctime_seconds, ctime_nanoseconds) = parse_find_timestamp(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }

    Some(FileFingerprint {
        device,
        inode,
        size,
        mtime_seconds,
        mtime_nanoseconds,
        ctime_seconds,
        ctime_nanoseconds,
    })
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = fs::symlink_metadata(path).ok()?;
    metadata.is_file().then_some(FileFingerprint {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.size(),
        mtime_seconds: metadata.mtime(),
        mtime_nanoseconds: metadata.mtime_nsec(),
        ctime_seconds: metadata.ctime(),
        ctime_nanoseconds: metadata.ctime_nsec(),
    })
}

fn parse_ov_readiness_attestation(content: &str) -> Option<(FileFingerprint, FileFingerprint)> {
    let mut lines = content.lines();
    if lines.next()? != "version=1" {
        return None;
    }
    let initramfs = parse_file_fingerprint(lines.next()?.strip_prefix("initramfs=")?)?;
    let module = parse_file_fingerprint(lines.next()?.strip_prefix("module=")?)?;
    if lines.next().is_some() {
        return None;
    }
    Some((initramfs, module))
}

fn readiness_marker_permissions_are_trusted(uid: u32, gid: u32, mode: u32, nlink: u64) -> bool {
    uid == 0 && gid == 0 && mode & 0o7777 == 0o644 && nlink == 1
}

fn ov_readiness_attestation_matches_files(
    marker_path: &Path,
    initramfs_path: &Path,
    module_path: &Path,
    require_root_owned_marker: bool,
) -> bool {
    let Ok(marker_metadata) = fs::symlink_metadata(marker_path) else {
        return false;
    };
    if !marker_metadata.is_file() || marker_metadata.len() > 4096 || marker_metadata.nlink() != 1 {
        return false;
    }
    if require_root_owned_marker
        && !readiness_marker_permissions_are_trusted(
            marker_metadata.uid(),
            marker_metadata.gid(),
            marker_metadata.mode(),
            marker_metadata.nlink(),
        )
    {
        return false;
    }
    let Some((expected_initramfs, expected_module)) = fs::read_to_string(marker_path)
        .ok()
        .and_then(|content| parse_ov_readiness_attestation(&content))
    else {
        return false;
    };

    file_fingerprint(initramfs_path) == Some(expected_initramfs)
        && file_fingerprint(module_path) == Some(expected_module)
}

fn ov02c10_readiness_attestation_is_current(kernel: &str) -> bool {
    if !kernel_release_is_safe(kernel) {
        return false;
    }
    let Some(module_path) = module_path_for_kernel(kernel, "ov02c10") else {
        return false;
    };
    let marker = format!("{OV_READY_DIRECTORY}/{kernel}.state");
    let marker_path = Path::new(&marker);
    if !trusted_marker_hierarchy(
        marker_path,
        &[
            ("/var", 0o755),
            ("/var/lib", 0o755),
            ("/var/lib/galaxybook-ov02c10", 0o755),
            (OV_READY_DIRECTORY, 0o755),
        ],
    ) {
        return false;
    }
    let initramfs = format!("/boot/initramfs-{kernel}.img");

    ov_readiness_attestation_matches_files(
        marker_path,
        Path::new(&initramfs),
        Path::new(&module_path),
        true,
    )
}

fn detect_sound_app_presence() -> RpmPackagePresence {
    let rpm_presence = rpm_package_presence("galaxybook-sound");
    if rpm_presence == RpmPackagePresence::Installed || command_exists("galaxybook-sound") {
        RpmPackagePresence::Installed
    } else {
        rpm_presence
    }
}

fn command_exists(command: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|directory| {
        fs::metadata(directory.join(command))
            .map(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

fn rpm_owner_name_is_safe(owner: &str) -> bool {
    !owner.is_empty()
        && owner
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
}

fn classify_rpm_file_owner_query(
    path: &str,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> RpmFileOwner {
    if exit_code == Some(0) && stderr.is_empty() {
        let Ok(stdout) = std::str::from_utf8(stdout) else {
            return RpmFileOwner::Unknown;
        };
        let Some(owner) = stdout.strip_suffix('\n') else {
            return RpmFileOwner::Unknown;
        };
        if rpm_owner_name_is_safe(owner) {
            return RpmFileOwner::Owned(owner.to_string());
        }
        return RpmFileOwner::Unknown;
    }

    let expected_unowned = format!("file {path} is not owned by any package\n");
    if exit_code == Some(1) && stdout == expected_unowned.as_bytes() && stderr.is_empty() {
        RpmFileOwner::Unowned
    } else {
        RpmFileOwner::Unknown
    }
}

fn rpm_owner_for_file(path: &str) -> RpmFileOwner {
    if !Path::new(path).is_absolute()
        || path
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\n' | b'\r'))
    {
        return RpmFileOwner::Unknown;
    }

    let Ok(output) = probe_output("rpm", &["-qf", "--qf", "%{NAME}\n", "--", path], true) else {
        return RpmFileOwner::Unknown;
    };
    classify_rpm_file_owner_query(path, output.status.code(), &output.stdout, &output.stderr)
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
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

fn classify_v4l2_capture_probe(exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> bool {
    if exit_code != Some(0) || !stderr.is_empty() {
        return false;
    }
    let Ok(stdout) = std::str::from_utf8(stdout) else {
        return false;
    };
    if stdout.is_empty()
        || !stdout.ends_with('\n')
        || stdout.contains('\0')
        || stdout.contains('\r')
    {
        return false;
    }
    stdout
        .lines()
        .any(|line| line.trim_matches([' ', '\t']) == "Video Capture")
}

fn v4l2_device_supports_capture(device: &str) -> bool {
    let Ok(output) = probe_output("v4l2-ctl", &["-D", "-d", device], true) else {
        return false;
    };
    classify_v4l2_capture_probe(output.status.code(), &output.stdout, &output.stderr)
}

fn classify_secure_boot_output(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<bool> {
    if exit_code != Some(0) || !stderr.is_empty() {
        return None;
    }
    match std::str::from_utf8(stdout).ok()? {
        "SecureBoot enabled\n" => Some(true),
        "SecureBoot disabled\n" => Some(false),
        _ => None,
    }
}

fn key_file_state(path: &str) -> KeyFileState {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => KeyFileState::Present,
        Ok(_) => KeyFileState::Missing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => KeyFileState::Missing,
        Err(_) => KeyFileState::Protected,
    }
}

fn signed_external_module_is_loaded(expected_identity: &AkmodsKeyIdentity) -> bool {
    let loaded_modules = read_trimmed("/proc/modules").unwrap_or_default();
    let running_kernel = running_kernel_release().unwrap_or_default();
    [
        ("ov02c10", "ov02c10"),
        ("snd-hda-scodec-max98390", "snd_hda_scodec_max98390"),
        ("snd-hda-scodec-max98390-i2c", "snd_hda_scodec_max98390_i2c"),
    ]
    .iter()
    .any(|(modinfo_name, proc_name)| {
        if !loaded_modules
            .lines()
            .any(|line| line.starts_with(&format!("{proc_name} ")))
        {
            return false;
        }
        let Some(path) = clean_command_text("modinfo", &["-n", modinfo_name]).ok() else {
            return false;
        };
        let loaded_srcversion = read_trimmed(&format!("/sys/module/{proc_name}/srcversion"));
        let candidate_srcversion = clean_command_text("modinfo", &["-F", "srcversion", &path]).ok();
        let signer = clean_command_text("modinfo", &["-F", "signer", &path]).ok();
        let sig_key = clean_command_text("modinfo", &["-F", "sig_key", &path]).ok();
        let vermagic = clean_command_text("modinfo", &["-F", "vermagic", &path]).ok();

        loaded_module_matches_candidate(
            loaded_module_binary_matches_candidate(
                proc_name,
                &path,
                loaded_srcversion.as_deref(),
                candidate_srcversion.as_deref(),
            ),
            signer.as_deref(),
            sig_key.as_deref(),
            vermagic.as_deref(),
            &running_kernel,
            mok_proof_candidate_module_path(modinfo_name, &path),
            true,
            Some(expected_identity),
        )
    })
}

fn module_filename_matches(path: &str, module: &str) -> bool {
    let Some(file_name) = Path::new(path).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    [".ko", ".ko.xz", ".ko.zst", ".ko.gz"]
        .iter()
        .any(|suffix| file_name == format!("{module}{suffix}"))
}

fn packaged_external_module_path(module: &str, path: &str) -> bool {
    let expected_directory = match module {
        "ov02c10" => "/extra/galaxybook-ov02c10/",
        "snd-hda-scodec-max98390" | "snd-hda-scodec-max98390-i2c" => "/extra/galaxybook-max98390/",
        _ => return false,
    };

    module_filename_matches(path, module) && path.contains(expected_directory)
}

fn mok_proof_candidate_module_path(module: &str, path: &str) -> bool {
    // A signed legacy /updates copy can prove that the current MOK accepted the
    // module already loaded by this boot. It is deliberately never accepted by
    // the per-kernel payload/readiness checks, which require the RPM-owned
    // /extra/galaxybook-* path, package owner, vermagic and OV attestation.
    packaged_external_module_path(module, path)
        || (module_filename_matches(path, module) && path.contains("/updates/"))
}

fn loaded_module_matches_candidate(
    binary_identity_matches: bool,
    signer: Option<&str>,
    sig_key: Option<&str>,
    vermagic: Option<&str>,
    running_kernel: &str,
    expected_external_path: bool,
    secure_boot_requires_signature: bool,
    expected_identity: Option<&AkmodsKeyIdentity>,
) -> bool {
    expected_external_path
        && !running_kernel.trim().is_empty()
        && binary_identity_matches
        && module_security_metadata_matches(
            running_kernel.trim(),
            vermagic,
            signer,
            sig_key,
            secure_boot_requires_signature,
            expected_identity,
        )
}

fn certificate_cn_from_subject(subject: &str) -> Option<String> {
    subject
        .split(',')
        .find_map(|part| {
            part.trim()
                .strip_prefix("subject=")
                .unwrap_or(part.trim())
                .strip_prefix("CN=")
        })
        .map(str::trim)
        .filter(|cn| !cn.is_empty())
        .map(str::to_string)
}

fn certificate_identity_from_output(output: &str) -> Option<AkmodsKeyIdentity> {
    let mut lines = output.lines();
    let subject = lines.next()?;
    let fingerprint = lines.next()?;
    let serial = lines.next()?;
    if lines.next().is_some()
        || !subject.starts_with("subject=")
        || !fingerprint.starts_with("sha1 Fingerprint=")
        || !serial.starts_with("serial=")
    {
        return None;
    }
    let cn = certificate_cn_from_subject(subject)?;
    let fingerprint_sha1 = fingerprint
        .strip_prefix("sha1 Fingerprint=")
        .and_then(|value| normalize_colonized_sha1(value.trim()))?;
    let certificate_serial = serial
        .strip_prefix("serial=")
        .and_then(normalize_certificate_serial)?;
    Some(AkmodsKeyIdentity {
        cn,
        fingerprint_sha1,
        certificate_serial,
    })
}

fn public_certificate_permissions_are_trusted(uid: u32, gid: u32, mode: u32, nlink: u64) -> bool {
    uid == 0 && gid == 0 && mode & 0o7777 == 0o644 && nlink == 1
}

fn public_certificate_fingerprint_from_metadata(
    metadata: &fs::Metadata,
    require_root_owned: bool,
) -> Option<FileFingerprint> {
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_PUBLIC_CERTIFICATE_SIZE
        || metadata.nlink() != 1
        || (require_root_owned
            && !public_certificate_permissions_are_trusted(
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
                metadata.nlink(),
            ))
    {
        return None;
    }

    Some(FileFingerprint {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.size(),
        mtime_seconds: metadata.mtime(),
        mtime_nanoseconds: metadata.mtime_nsec(),
        ctime_seconds: metadata.ctime(),
        ctime_nanoseconds: metadata.ctime_nsec(),
    })
}

fn trusted_public_certificate_fingerprint(
    path: &Path,
    require_root_owned: bool,
    require_trusted_hierarchy: bool,
) -> Option<FileFingerprint> {
    if require_trusted_hierarchy
        && (path.parent() != Some(Path::new("/var/lib/galaxybook-setup"))
            || !trusted_marker_hierarchy(
                path,
                &[
                    ("/var", 0o755),
                    ("/var/lib", 0o755),
                    ("/var/lib/galaxybook-setup", 0o755),
                ],
            ))
    {
        return None;
    }

    let metadata = fs::symlink_metadata(path).ok()?;
    public_certificate_fingerprint_from_metadata(&metadata, require_root_owned)
}

struct OpenedPublicCertificate {
    file: fs::File,
    fingerprint: FileFingerprint,
    der: Vec<u8>,
}

fn opened_public_certificate_still_matches_path(
    certificate: &OpenedPublicCertificate,
    path: &Path,
    require_root_owned: bool,
    require_trusted_hierarchy: bool,
) -> bool {
    let descriptor_fingerprint = certificate.file.metadata().ok().and_then(|metadata| {
        public_certificate_fingerprint_from_metadata(&metadata, require_root_owned)
    });
    let path_fingerprint =
        trusted_public_certificate_fingerprint(path, require_root_owned, require_trusted_hierarchy);
    descriptor_fingerprint.as_ref() == Some(&certificate.fingerprint)
        && path_fingerprint.as_ref() == Some(&certificate.fingerprint)
}

fn open_trusted_public_certificate(
    path: &Path,
    require_root_owned: bool,
    require_trusted_hierarchy: bool,
) -> Option<OpenedPublicCertificate> {
    let path_fingerprint = trusted_public_certificate_fingerprint(
        path,
        require_root_owned,
        require_trusted_hierarchy,
    )?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(LINUX_O_NOFOLLOW)
        .open(path)
        .ok()?;
    let descriptor_fingerprint = file.metadata().ok().and_then(|metadata| {
        public_certificate_fingerprint_from_metadata(&metadata, require_root_owned)
    })?;
    if descriptor_fingerprint != path_fingerprint {
        return None;
    }

    let mut der = Vec::with_capacity(descriptor_fingerprint.size as usize);
    (&mut file)
        .take(MAX_PUBLIC_CERTIFICATE_SIZE + 1)
        .read_to_end(&mut der)
        .ok()?;
    if der.len() as u64 != descriptor_fingerprint.size {
        return None;
    }

    let certificate = OpenedPublicCertificate {
        file,
        fingerprint: descriptor_fingerprint,
        der,
    };
    opened_public_certificate_still_matches_path(
        &certificate,
        path,
        require_root_owned,
        require_trusted_hierarchy,
    )
    .then_some(certificate)
}

fn published_certificate_identity_from_path(
    path: &Path,
    require_root_owned: bool,
    require_trusted_hierarchy: bool,
) -> Option<AkmodsKeyIdentity> {
    let certificate =
        open_trusted_public_certificate(path, require_root_owned, require_trusted_hierarchy)?;
    let output = probe_output_with_stdin(
        OPENSSL_COMMAND,
        OPENSSL_CERTIFICATE_IDENTITY_ARGS,
        true,
        &certificate.der,
    )
    .ok()?;
    let certificate_unchanged = opened_public_certificate_still_matches_path(
        &certificate,
        path,
        require_root_owned,
        require_trusted_hierarchy,
    );
    if !certificate_unchanged {
        return None;
    }
    certificate_identity_from_output(strict_success_stdout(&output)?)
}

fn current_akmods_certificate_identity() -> Option<AkmodsKeyIdentity> {
    published_certificate_identity_from_path(
        Path::new(AKMODS_PUBLIC_CERTIFICATE_STATE_PATH),
        true,
        true,
    )
}

fn normalize_colonized_sha1(value: &str) -> Option<String> {
    let octets = value.split(':').collect::<Vec<_>>();
    if octets.len() != 20
        || octets
            .iter()
            .any(|octet| octet.len() != 2 || !octet.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return None;
    }
    Some(octets.join(":").to_ascii_uppercase())
}

fn normalize_certificate_serial(value: &str) -> Option<String> {
    let mut normalized = value
        .trim()
        .chars()
        .filter(|character| *character != ':')
        .collect::<String>();
    if normalized.is_empty()
        || normalized.len() % 2 != 0
        || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    normalized.make_ascii_uppercase();
    while normalized.len() > 2 && normalized.starts_with("00") {
        normalized.drain(..2);
    }
    if normalized.len() > 40 || !normalized.bytes().any(|byte| byte != b'0') {
        return None;
    }
    Some(normalized)
}

fn valid_key_cn(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value == value.trim()
        && !value.chars().any(char::is_control)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn parse_akmods_key_marker(content: &str) -> Option<AkmodsKeyMarker> {
    if content.contains('\r') {
        return None;
    }

    if content.starts_with("version=") || content.contains('\n') && !content.ends_with('\n') {
        let body = content.strip_suffix('\n')?;
        let lines = body.split('\n').collect::<Vec<_>>();
        if lines.len() != 4 || lines[0] != "version=2" {
            return None;
        }
        let cn = lines[1].strip_prefix("cn=")?;
        let fingerprint_value = lines[2].strip_prefix("fingerprint_sha1=")?;
        let fingerprint_sha1 = normalize_colonized_sha1(fingerprint_value)?;
        let serial_value = lines[3].strip_prefix("certificate_serial=")?;
        let certificate_serial = normalize_certificate_serial(serial_value)?;
        if !valid_key_cn(cn) {
            return None;
        }
        if fingerprint_sha1 != fingerprint_value || certificate_serial != serial_value {
            return None;
        }
        return Some(AkmodsKeyMarker::V2(AkmodsKeyIdentity {
            cn: cn.to_string(),
            fingerprint_sha1,
            certificate_serial,
        }));
    }

    let cn = content.strip_suffix('\n').unwrap_or(content);
    valid_key_cn(cn).then(|| AkmodsKeyMarker::LegacyCn(cn.to_string()))
}

fn read_akmods_key_marker(path: &str) -> Option<AkmodsKeyMarker> {
    read_trusted_root_file(path).and_then(|content| parse_akmods_key_marker(&content))
}

fn v2_marker_identity(marker: Option<AkmodsKeyMarker>) -> Option<AkmodsKeyIdentity> {
    match marker {
        Some(AkmodsKeyMarker::V2(identity)) => Some(identity),
        Some(AkmodsKeyMarker::LegacyCn(_)) | None => None,
    }
}

fn current_akmods_key_identity() -> Option<AkmodsKeyIdentity> {
    let certificate = current_akmods_certificate_identity();
    let marker = v2_marker_identity(read_akmods_key_marker(AKMODS_KEY_CN_PATH));
    validated_key_identity(certificate, marker)
}

fn validated_key_identity(
    certificate: Option<AkmodsKeyIdentity>,
    marker: Option<AkmodsKeyIdentity>,
) -> Option<AkmodsKeyIdentity> {
    match (certificate, marker) {
        (Some(certificate), Some(marker)) if certificate == marker => Some(certificate),
        _ => None,
    }
}

fn legacy_akmods_marker_cn() -> Option<String> {
    [AKMODS_KEY_CN_PATH, LEGACY_MOK_PENDING_CN_PATH]
        .iter()
        .find_map(|path| match read_akmods_key_marker(path) {
            Some(AkmodsKeyMarker::LegacyCn(cn)) => Some(cn),
            Some(AkmodsKeyMarker::V2(_)) | None => None,
        })
}

fn mok_list_contains_identity(output: &str, identity: &AkmodsKeyIdentity) -> bool {
    output.lines().any(|line| {
        line.trim()
            .strip_prefix("SHA1 Fingerprint:")
            .and_then(|value| normalize_colonized_sha1(value.trim()))
            .as_deref()
            == Some(identity.fingerprint_sha1.as_str())
    })
}

fn mok_list_matches_current_identity(output: &str, identity: Option<&AkmodsKeyIdentity>) -> bool {
    identity.is_some_and(|identity| mok_list_contains_identity(output, identity))
}

fn mok_enrollment_is_proven(
    identity: Option<&AkmodsKeyIdentity>,
    enrolled_list_output: Option<&str>,
    public_certificate_test_succeeded: bool,
) -> bool {
    identity.is_some_and(|identity| {
        public_certificate_test_succeeded
            || enrolled_list_output
                .is_some_and(|output| mok_list_contains_identity(output, identity))
    })
}

fn mok_pending_output_matches_current_identity(
    output_success: bool,
    output: &str,
    current_identity: Option<&AkmodsKeyIdentity>,
) -> bool {
    output_success && mok_list_matches_current_identity(output, current_identity)
}

fn mok_pending_request_exists(identity: &AkmodsKeyIdentity) -> bool {
    // The privileged helper publishes both the public DER and its v2 marker.
    // Require those two root-owned artifacts to prove the same current key
    // before matching its SHA-1 fingerprint against mokutil --list-new. This
    // proof is sufficient only to offer the enrollment reboot; it never
    // establishes enrollment, driver readiness or module ownership.
    let Ok(output) = probe_output("mokutil", &["--list-new"], true) else {
        return false;
    };
    let Some(output_text) = strict_success_stdout(&output) else {
        return false;
    };
    mok_pending_output_matches_current_identity(true, output_text, Some(identity))
}

fn read_trusted_file(path: &str, require_root_owned: bool) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file()
        || metadata.len() > 4096
        || metadata.nlink() != 1
        || (require_root_owned
            && !readiness_marker_permissions_are_trusted(
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
                metadata.nlink(),
            ))
    {
        return None;
    }

    fs::read_to_string(path).ok()
}

fn root_directory_metadata_is_trusted(
    is_directory: bool,
    uid: u32,
    gid: u32,
    mode: u32,
    expected_mode: u32,
) -> bool {
    is_directory && uid == 0 && gid == 0 && mode & 0o7777 == expected_mode
}

fn trusted_root_directory(path: &Path, expected_mode: u32) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    root_directory_metadata_is_trusted(
        metadata.is_dir(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode(),
        expected_mode,
    )
}

fn trusted_marker_hierarchy(path: &Path, expected_directories: &[(&str, u32)]) -> bool {
    expected_directories.iter().all(|(directory, mode)| {
        trusted_root_directory(Path::new(directory), *mode) && path.starts_with(directory)
    })
}

fn read_trusted_root_file(path: &str) -> Option<String> {
    let path = Path::new(path);
    trusted_marker_hierarchy(
        path,
        &[
            ("/var", 0o755),
            ("/var/lib", 0o755),
            ("/var/lib/galaxybook-setup", 0o755),
        ],
    )
    .then(|| read_trusted_file(path.to_str()?, true))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn assert_bash_syntax(label: &str, script: &str) {
        let mut child = Command::new("bash")
            .args(["--noprofile", "--norc", "-n"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("não foi possível iniciar bash para {label}: {error}"));
        child
            .stdin
            .as_mut()
            .expect("stdin do bash indisponível")
            .write_all(script.as_bytes())
            .unwrap_or_else(|error| panic!("não foi possível validar {label}: {error}"));
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("bash -n falhou para {label}: {error}"));
        assert!(
            output.status.success(),
            "sintaxe shell inválida em {label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn run_reboot_command_with_mock_systemctl(
        list_exit: i32,
        list_stdout: &str,
        list_stderr: &str,
    ) -> (std::process::Output, String) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio inválido")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "galaxybook-setup-reboot-test-{}-{nonce}",
            std::process::id()
        ));
        let mock_bin = test_root.join("bin");
        let trace = test_root.join("systemctl.trace");
        fs::create_dir_all(&mock_bin).expect("não foi possível criar diretório de mocks");
        let systemctl = mock_bin.join("systemctl");
        fs::write(
            &systemctl,
            r#"#!/usr/bin/bash
set -euo pipefail
case "${1-}" in
  list-units)
    printf '%s' "${MOCK_LIST_STDOUT-}"
    printf '%s' "${MOCK_LIST_STDERR-}" >&2
    exit "${MOCK_LIST_EXIT-0}"
    ;;
  reboot)
    printf 'reboot\n' >> "${MOCK_TRACE:?}"
    ;;
  *)
    exit 97
    ;;
esac
"#,
        )
        .expect("não foi possível escrever mock do systemctl");
        let mut permissions = fs::metadata(&systemctl)
            .expect("mock do systemctl ausente")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&systemctl, permissions)
            .expect("não foi possível tornar o mock executável");

        let output = Command::new("bash")
            .args(["--noprofile", "--norc", "-c", REBOOT_COMMAND])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", mock_bin.display()))
            .env("MOCK_LIST_EXIT", list_exit.to_string())
            .env("MOCK_LIST_STDOUT", list_stdout)
            .env("MOCK_LIST_STDERR", list_stderr)
            .env("MOCK_TRACE", &trace)
            .output()
            .expect("não foi possível executar o comando de reboot com mock");
        let trace_contents = fs::read_to_string(&trace).unwrap_or_default();
        fs::remove_dir_all(&test_root).expect("não foi possível remover mocks do reboot");
        (output, trace_contents)
    }

    #[test]
    fn embedded_shell_commands_have_valid_bash_syntax() {
        let scripts = [
            (
                "INSTALL_MAIN_SUPPORT_COMMAND",
                INSTALL_MAIN_SUPPORT_COMMAND.to_string(),
            ),
            ("INSTALL_CAMERA_COMMAND", INSTALL_CAMERA_COMMAND.to_string()),
            (
                "INSTALL_SOUND_APP_COMMAND",
                INSTALL_SOUND_APP_COMMAND.to_string(),
            ),
            (
                "ENABLE_CAMERA_MODULE_COMMAND",
                ENABLE_CAMERA_MODULE_COMMAND.to_string(),
            ),
            (
                "ENABLE_BROWSER_CAMERA_COMMAND",
                ENABLE_BROWSER_CAMERA_COMMAND.to_string(),
            ),
            ("ENABLE_SPEAKER_COMMAND", ENABLE_SPEAKER_COMMAND.to_string()),
            (
                "PREPARE_SECURE_BOOT_KEY_COMMAND",
                PREPARE_SECURE_BOOT_KEY_COMMAND.to_string(),
            ),
            ("REPAIR_NVIDIA_COMMAND", REPAIR_NVIDIA_COMMAND.to_string()),
            (
                "SET_BALANCED_PROFILE_COMMAND",
                SET_BALANCED_PROFILE_COMMAND.to_string(),
            ),
            (
                "REPAIR_FINGERPRINT_COMMAND",
                REPAIR_FINGERPRINT_COMMAND.to_string(),
            ),
            (
                "ENABLE_FINGERPRINT_AUTH_COMMAND",
                ENABLE_FINGERPRINT_AUTH_COMMAND.to_string(),
            ),
            (
                "OPEN_FINGERPRINT_SETTINGS_COMMAND",
                OPEN_FINGERPRINT_SETTINGS_COMMAND.to_string(),
            ),
            ("REBOOT_COMMAND", REBOOT_COMMAND.to_string()),
            ("clipboard profile", build_clipboard_profile_command()),
            ("GSConnect profile", build_gsconnect_profile_command()),
            (
                "desktop icons profile",
                build_desktop_icons_profile_command(),
            ),
            ("dock profile", build_dash_to_dock_profile_command()),
        ];

        for (label, script) in scripts {
            assert_bash_syntax(label, &script);
        }
    }

    #[test]
    fn user_actions_do_not_start_privileged_login_shells() {
        for command in [
            build_gsconnect_profile_command(),
            build_dash_to_dock_profile_command(),
        ] {
            assert!(!command.contains("pkexec sh"));
            assert!(!command.contains("pkexec /usr/bin/bash"));
            assert!(!command.contains(" -lc "));
        }
        assert!(
            build_gsconnect_profile_command()
                .contains("pkexec /usr/bin/dnf install -y nautilus-python")
        );
        assert!(
            build_dash_to_dock_profile_command()
                .contains("pkexec /usr/bin/dnf install -y gnome-shell-extension-dash-to-dock")
        );
    }

    #[test]
    fn command_lookup_does_not_invoke_a_shell() {
        assert!(command_exists("sh"));
        assert!(!command_exists("galaxybook-command-that-does-not-exist"));
    }

    #[test]
    fn secure_boot_key_command_delegates_to_hardened_helper() {
        assert_eq!(
            PREPARE_SECURE_BOOT_KEY_COMMAND,
            "exec /usr/libexec/galaxybook-setup/prepare-secure-boot-key"
        );
        assert!(!PREPARE_SECURE_BOOT_KEY_COMMAND.contains("/etc/pki/akmods/private"));
    }

    #[test]
    fn reboot_rechecks_akmods_and_respects_system_inhibitors() {
        assert!(REBOOT_COMMAND.contains("akmods@*.service"));
        assert!(REBOOT_COMMAND.contains("--state=activating,running"));
        assert!(REBOOT_COMMAND.contains("systemctl reboot --check-inhibitors=yes"));
        assert!(!REBOOT_COMMAND.contains(" --check-inhibitors=no"));
        assert!(!REBOOT_COMMAND.contains("systemctl reboot -i"));
    }

    #[test]
    fn reboot_requires_a_clean_canonically_empty_akmods_query() {
        for (label, exit_code, stdout, stderr) in [
            ("query-failure", 1, "", ""),
            ("stderr-warning", 0, "", "warning\n"),
            ("whitespace-stdout", 0, "\n", ""),
            (
                "active-build",
                0,
                "akmods.service loaded active running\n",
                "",
            ),
        ] {
            let (output, trace) = run_reboot_command_with_mock_systemctl(exit_code, stdout, stderr);
            assert!(!output.status.success(), "{label} não pode liberar reboot");
            assert!(trace.is_empty(), "{label} chamou reboot indevidamente");
        }

        let (output, trace) = run_reboot_command_with_mock_systemctl(0, "", "");
        assert!(output.status.success());
        assert_eq!(trace, "reboot\n");
    }

    #[test]
    fn module_origin_detects_external_driver_paths() {
        assert_eq!(
            module_origin_from_path(Some(
                "/lib/modules/6.19.10/extra/intel-ipu6/drivers/media/i2c/ov02c10.ko.xz"
            )),
            ModuleOrigin::Unknown
        );
        assert_eq!(
            module_origin_from_path(Some("/lib/modules/6.19.10/updates/ov02c10.ko")),
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
    fn force_camera_driver_command_delegates_multi_kernel_reconciliation() {
        assert_eq!(OV02C10_KMOD_MIN_VERSION, "1.00.25");
        assert_eq!(
            FORCE_CAMERA_DRIVER_COMMAND,
            "exec /usr/libexec/galaxybook-setup/driver-maintenance install-camera"
        );
        assert!(!FORCE_CAMERA_DRIVER_COMMAND.contains("/updates/ov02c10"));
        assert!(!FORCE_CAMERA_DRIVER_COMMAND.contains("modprobe ov02c10"));
        assert!(!FORCE_CAMERA_DRIVER_COMMAND.contains("dracut --force"));
    }

    #[test]
    fn max_driver_commands_require_the_packaged_finalizer_release() {
        let helper = include_str!("../scripts/driver-maintenance.sh");

        assert_eq!(MAX98390_KMOD_MIN_VERSION, "1.00.12");
        assert!(helper.contains("galaxybook-max98390-kmod-common >= 1.00.12"));
        assert!(helper.contains("akmod-galaxybook-max98390 >= 1.00.12"));
        assert!(helper.contains("kmod-galaxybook-max98390 >= 1.00.12"));
    }

    #[test]
    fn nvidia_command_delegates_locked_kernel_maintenance() {
        assert_eq!(
            REPAIR_NVIDIA_COMMAND,
            "exec /usr/libexec/galaxybook-setup/repair-nvidia-kmod"
        );
        assert!(!REPAIR_NVIDIA_COMMAND.contains("depmod"));
        assert!(!REPAIR_NVIDIA_COMMAND.contains("dracut"));
    }

    #[test]
    fn camera_and_sound_commands_require_the_current_application_contracts() {
        let helper = include_str!("../scripts/driver-maintenance.sh");

        assert!(helper.contains("galaxybook-camera >= 1.00.43"));
        assert!(INSTALL_SOUND_APP_COMMAND.contains("galaxybook-sound >= 1.00.18"));
    }

    #[test]
    fn browser_camera_command_delegates_safe_packaged_helper() {
        assert_eq!(
            ENABLE_BROWSER_CAMERA_COMMAND,
            "exec /usr/libexec/galaxybook-setup/driver-maintenance enable-browser-camera"
        );
        assert!(!ENABLE_BROWSER_CAMERA_COMMAND.contains("cat > /etc"));
        assert!(!ENABLE_BROWSER_CAMERA_COMMAND.contains("rm -f /etc/udev"));
    }

    #[test]
    fn module_install_commands_delegate_to_the_inhibited_coordinator() {
        assert_eq!(
            INSTALL_MAIN_SUPPORT_COMMAND,
            "exec /usr/libexec/galaxybook-setup/driver-maintenance install-main"
        );
        assert_eq!(
            INSTALL_CAMERA_COMMAND,
            "exec /usr/libexec/galaxybook-setup/driver-maintenance install-camera"
        );
        assert_eq!(
            ENABLE_SPEAKER_COMMAND,
            "exec /usr/libexec/galaxybook-setup/driver-maintenance enable-speakers"
        );
    }

    #[test]
    fn clock_error_detection_matches_known_boot_failure() {
        let logs = "
            ov02c10 i2c-OVTI02C1:00: error -EINVAL: external clock 26000000 is not supported
            ov02c10 i2c-OVTI02C1:00: probe with driver ov02c10 failed with error -22
        ";
        assert!(detect_clock_error(logs));
        assert_eq!(
            boot_check_from_kernel_log(KernelJournalProbe::ClockError).health,
            Health::Error
        );
        let unavailable = boot_check_from_kernel_log(KernelJournalProbe::Unknown);
        assert_eq!(unavailable.health, Health::Unknown);
        assert_eq!(unavailable.code, "boot-log-unavailable");
        assert_ne!(unavailable.code, "boot-ok");
    }

    #[test]
    fn kernel_journal_probe_is_strict_and_never_turns_partial_evidence_green() {
        let clean_log = b"Linux version 7.3.4 (test kernel)\n";
        let clock_log = b"ov02c10: external clock 26000000 is not supported\n";
        assert_eq!(
            classify_kernel_journal_probe(Some(0), clean_log, b""),
            KernelJournalProbe::Clean
        );
        assert_eq!(
            classify_kernel_journal_probe(Some(0), clock_log, b""),
            KernelJournalProbe::ClockError
        );
        assert_eq!(
            boot_check_from_kernel_log(KernelJournalProbe::Clean).code,
            "boot-ok"
        );

        for (label, exit_code, stdout, stderr) in [
            ("empty", Some(0), &b""[..], &b""[..]),
            (
                "no-accessible-entries",
                Some(0),
                &b"-- No entries --\n"[..],
                &b""[..],
            ),
            (
                "early-boot-missing",
                Some(0),
                &b"kernel: ordinary line\n"[..],
                &b""[..],
            ),
            ("warning", Some(0), &clean_log[..], &b"warning\n"[..]),
            ("query-failure", Some(1), &clean_log[..], &b""[..]),
            ("timeout", Some(124), &clean_log[..], &b""[..]),
            ("partial-line", Some(0), &b"kernel: partial"[..], &b""[..]),
            ("invalid-utf8", Some(0), &b"kernel: \xff\n"[..], &b""[..]),
            (
                "embedded-nul",
                Some(0),
                &b"kernel: ok\0tail\n"[..],
                &b""[..],
            ),
        ] {
            let probe = classify_kernel_journal_probe(exit_code, stdout, stderr);
            assert_eq!(probe, KernelJournalProbe::Unknown, "{label}");
            let check = boot_check_from_kernel_log(probe);
            assert_eq!(check.health, Health::Unknown, "{label}");
            assert_ne!(check.code, "boot-ok", "{label}");
        }
    }

    #[test]
    fn diagnostic_probes_are_wrapped_in_a_bounded_timeout() {
        let args = probe_command_arguments("mokutil", &["--list-new"]);
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec!["--kill-after=2s", "8s", "mokutil", "--list-new",]
        );
        assert_eq!(PROBE_TIMEOUT_COMMAND, "/usr/bin/timeout");
        assert_eq!(OPENSSL_COMMAND, "/usr/bin/openssl");
        assert!(!OPENSSL_CERTIFICATE_IDENTITY_ARGS.contains(&"-in"));
        let openssl_args =
            probe_command_arguments(OPENSSL_COMMAND, OPENSSL_CERTIFICATE_IDENTITY_ARGS);
        assert_eq!(openssl_args[0], "--kill-after=2s");
        assert_eq!(openssl_args[1], "8s");
        assert_eq!(openssl_args[2], OPENSSL_COMMAND);
    }

    #[test]
    fn critical_probe_text_requires_clean_single_line_output() {
        assert_eq!(
            classify_clean_command_text(Some(0), b"expected value\n", b""),
            Ok("expected value".to_string())
        );

        for (label, exit_code, stdout, stderr) in [
            ("empty", Some(0), &b""[..], &b""[..]),
            (
                "warning",
                Some(0),
                &b"expected value\n"[..],
                &b"warning\n"[..],
            ),
            (
                "multiple-lines",
                Some(0),
                &b"expected value\nother value\n"[..],
                &b""[..],
            ),
            (
                "leading-blank-line",
                Some(0),
                &b"\nexpected value\n"[..],
                &b""[..],
            ),
            (
                "trailing-extra-newline",
                Some(0),
                &b"expected value\n\n"[..],
                &b""[..],
            ),
            (
                "leading-whitespace",
                Some(0),
                &b" expected value\n"[..],
                &b""[..],
            ),
            (
                "trailing-whitespace",
                Some(0),
                &b"expected value \n"[..],
                &b""[..],
            ),
            ("invalid-utf8", Some(0), &b"value\xff"[..], &b""[..]),
            ("embedded-nul", Some(0), &b"value\0tail\n"[..], &b""[..]),
            ("failure", Some(1), &b"expected value\n"[..], &b""[..]),
            ("timeout", Some(124), &b"expected value\n"[..], &b""[..]),
        ] {
            assert!(
                classify_clean_command_text(exit_code, stdout, stderr).is_err(),
                "{label} não pode comprovar metadados críticos",
            );
        }
    }

    #[test]
    fn strict_success_output_and_akmods_state_fail_closed() {
        assert_eq!(
            classify_strict_success_stdout(Some(0), b"trusted\n", b""),
            Some("trusted\n")
        );
        assert!(classify_strict_success_stdout(Some(0), b"trusted\n", b"warning\n").is_none());
        assert!(classify_strict_success_stdout(Some(1), b"trusted\n", b"").is_none());
        assert!(classify_strict_success_stdout(Some(0), b"trusted\xff", b"").is_none());

        assert_eq!(classify_akmods_build_query(Some(0), b"", b""), Some(false));
        assert_eq!(classify_akmods_build_query(Some(0), b"\n", b""), None);
        assert_eq!(classify_akmods_build_query(Some(0), b" \t\n", b""), None);
        assert_eq!(
            classify_akmods_build_query(Some(0), b"akmods.service loaded active running\n", b""),
            Some(true)
        );
        assert_eq!(
            classify_akmods_build_query(Some(0), b"", b"warning\n"),
            None
        );
        assert_eq!(classify_akmods_build_query(Some(1), b"", b""), None);
        assert_eq!(classify_akmods_build_query(Some(0), b"\xff", b""), None);
        assert!(global_reboot_ready(Some(false)));
        assert!(!global_reboot_ready(Some(true)));
        assert!(!global_reboot_ready(None));
    }

    #[test]
    fn running_kernel_query_requires_one_canonical_safe_line() {
        let kernel = "7.1.8-200.fc44.x86_64";
        assert_eq!(
            classify_kernel_release_query(Some(0), format!("{kernel}\n").as_bytes(), b""),
            Some(kernel.to_string())
        );

        for (label, exit_code, stdout, stderr) in [
            ("empty", Some(0), &b""[..], &b""[..]),
            (
                "missing-final-newline",
                Some(0),
                kernel.as_bytes(),
                &b""[..],
            ),
            (
                "extra-newline",
                Some(0),
                &b"7.1.8-200.fc44.x86_64\n\n"[..],
                &b""[..],
            ),
            (
                "multiple-lines",
                Some(0),
                &b"7.1.8-200.fc44.x86_64\n7.1.9-200.fc44.x86_64\n"[..],
                &b""[..],
            ),
            (
                "leading-whitespace",
                Some(0),
                &b" 7.1.8-200.fc44.x86_64\n"[..],
                &b""[..],
            ),
            ("unsafe-release", Some(0), &b"../../kernel\n"[..], &b""[..]),
            (
                "stderr-warning",
                Some(0),
                &b"7.1.8-200.fc44.x86_64\n"[..],
                &b"warning\n"[..],
            ),
            (
                "invalid-utf8",
                Some(0),
                &b"7.1.8-200.fc44.x86_64\xff\n"[..],
                &b""[..],
            ),
            (
                "query-failure",
                Some(1),
                &b"7.1.8-200.fc44.x86_64\n"[..],
                &b""[..],
            ),
        ] {
            assert_eq!(
                classify_kernel_release_query(exit_code, stdout, stderr),
                None,
                "{label} não pode comprovar o kernel em execução",
            );
        }
    }

    #[test]
    fn rpm_package_query_classifies_only_conclusive_results() {
        let package = "akmod-galaxybook-ov02c10";

        assert_eq!(
            classify_rpm_package_query(package, Some(0), b"akmod-galaxybook-ov02c10\n", b"",),
            RpmPackagePresence::Installed
        );
        assert_eq!(
            classify_rpm_package_query(
                package,
                Some(0),
                b"akmod-galaxybook-ov02c10\nakmod-galaxybook-ov02c10\n",
                b"",
            ),
            RpmPackagePresence::Installed
        );
        assert_eq!(
            classify_rpm_package_query(
                package,
                Some(1),
                b"package akmod-galaxybook-ov02c10 is not installed\n",
                b"",
            ),
            RpmPackagePresence::Absent
        );

        for (label, exit_code, stdout, stderr) in [
            ("timeout", Some(124), &b""[..], &b""[..]),
            (
                "rpmdb-error",
                Some(1),
                &b""[..],
                &b"error: cannot open Packages database\n"[..],
            ),
            ("interrupted", None, &b""[..], &b""[..]),
            (
                "partial",
                Some(1),
                &b"package akmod-galaxybook-ov02c10 is not instal"[..],
                &b""[..],
            ),
            (
                "malformed-extra-output",
                Some(1),
                &b"package akmod-galaxybook-ov02c10 is not installed\nwarning\n"[..],
                &b""[..],
            ),
            (
                "malformed-stderr",
                Some(1),
                &b"package akmod-galaxybook-ov02c10 is not installed\n"[..],
                &b"warning\n"[..],
            ),
            ("installed-empty", Some(0), &b""[..], &b""[..]),
            (
                "installed-stderr",
                Some(0),
                &b"akmod-galaxybook-ov02c10\n"[..],
                &b"warning\n"[..],
            ),
            (
                "installed-invalid-utf8",
                Some(0),
                &b"akmod-galaxybook-ov02c10\n\xff"[..],
                &b""[..],
            ),
            (
                "installed-other-name",
                Some(0),
                &b"galaxybook-camera\n"[..],
                &b""[..],
            ),
            (
                "installed-mixed-names",
                Some(0),
                &b"akmod-galaxybook-ov02c10\ngalaxybook-camera\n"[..],
                &b""[..],
            ),
        ] {
            assert_eq!(
                classify_rpm_package_query(package, exit_code, stdout, stderr),
                RpmPackagePresence::Unknown,
                "{label} não pode ser classificado como ausência",
            );
        }
    }

    #[test]
    fn rpm_presence_queries_are_allowlisted_and_use_safe_dynamic_kmod_names() {
        assert!(rpm_package_name_is_allowed("galaxybook-camera"));
        assert!(rpm_package_name_is_allowed(
            "kmod-galaxybook-ov02c10-7.1.8-200.fc44.x86_64"
        ));
        assert!(!rpm_package_name_is_allowed(
            "kmod-galaxybook-ov02c10-../../etc/passwd"
        ));
        assert!(!rpm_package_name_is_allowed("kmod-galaxybook-ov02c10-.."));
        assert_eq!(
            rpm_package_query_arguments("akmod-galaxybook-ov02c10"),
            Some(["-q", "--qf", "%{NAME}\n", "--", "akmod-galaxybook-ov02c10",])
        );
        assert!(!rpm_package_name_is_allowed(
            "kmod-galaxybook-ov02c10-.hidden"
        ));
        assert!(!rpm_package_name_is_allowed("arbitrary-package"));
        assert_eq!(
            rpm_package_query_arguments("galaxybook-camera"),
            Some(["-q", "--qf", "%{NAME}\n", "--", "galaxybook-camera"])
        );
        assert_eq!(rpm_package_query_arguments("arbitrary-package"), None);
    }

    #[test]
    fn rpm_file_owner_requires_conclusive_owned_or_unowned_output() {
        let path = "/lib/modules/7.1.8/updates/ov02c10.ko.zst";
        assert_eq!(
            classify_rpm_file_owner_query(path, Some(0), b"kmod-galaxybook-ov02c10-7.1.8\n", b"",),
            RpmFileOwner::Owned("kmod-galaxybook-ov02c10-7.1.8".into())
        );
        assert_eq!(
            classify_rpm_file_owner_query(
                path,
                Some(1),
                b"file /lib/modules/7.1.8/updates/ov02c10.ko.zst is not owned by any package\n",
                b"",
            ),
            RpmFileOwner::Unowned
        );

        for (exit_code, stdout, stderr) in [
            (Some(124), &b""[..], &b""[..]),
            (Some(1), &b""[..], &b"rpmdb unavailable\n"[..]),
            (None, &b""[..], &b""[..]),
            (
                Some(1),
                &b"file /lib/modules/7.1.8/updates/ov02c10.ko.zst is not owned"[..],
                &b""[..],
            ),
            (Some(0), &b"kmod-galaxybook-ov02c10-7.1.8"[..], &b""[..]),
            (
                Some(0),
                &b"kmod-galaxybook-ov02c10-7.1.8\nsecond-owner\n"[..],
                &b""[..],
            ),
        ] {
            assert_eq!(
                classify_rpm_file_owner_query(path, exit_code, stdout, stderr),
                RpmFileOwner::Unknown
            );
        }

        assert!(manual_updates_override_is_proven(
            path,
            &RpmFileOwner::Unowned,
        ));
        assert!(!manual_updates_override_is_proven(
            path,
            &RpmFileOwner::Unknown,
        ));
    }

    #[test]
    fn partial_package_inventory_stays_unknown_and_blocks_driver_reboot() {
        let packages = [
            "galaxybook-ov02c10-kmod-common",
            "akmod-galaxybook-ov02c10",
            "kmod-galaxybook-ov02c10",
        ];
        let inventory = package_presence_with(&packages, |package| match package {
            "galaxybook-ov02c10-kmod-common" => RpmPackagePresence::Installed,
            "akmod-galaxybook-ov02c10" => RpmPackagePresence::Absent,
            "kmod-galaxybook-ov02c10" => RpmPackagePresence::Unknown,
            _ => unreachable!("pacote inesperado no inventário de teste"),
        });

        assert_eq!(inventory.installed, vec!["galaxybook-ov02c10-kmod-common"]);
        assert_eq!(inventory.missing, vec!["akmod-galaxybook-ov02c10"]);
        assert_eq!(inventory.unknown, vec!["kmod-galaxybook-ov02c10"]);
        assert!(!package_inventory_is_complete_for(&inventory, &packages));
        assert!(!driver_reboot_state_allows_reboot(false, true, true, true,));

        let summary = main_package_check(&inventory);
        assert_eq!(summary.health, Health::Unknown);
        assert_eq!(summary.code, "packages-query-unknown");
        assert!(
            summary
                .detail
                .contains("não classificou esses pacotes como ausentes")
        );
    }

    #[test]
    fn secure_boot_summary_and_mok_share_the_snapshot_state() {
        for (state, label, code) in [
            (None, "Não foi possível verificar", "mok-unavailable"),
            (Some(false), "Desativado", "mok-not-needed"),
        ] {
            assert_eq!(secure_boot_label(state), tr(label));
            assert_eq!(detect_secure_boot_key_check(state, None).code, code);
        }
    }

    #[test]
    fn secure_boot_probe_accepts_only_a_clean_canonical_response() {
        assert_eq!(
            classify_secure_boot_output(Some(0), b"SecureBoot enabled\n", b""),
            Some(true)
        );
        assert_eq!(
            classify_secure_boot_output(Some(0), b"SecureBoot disabled\n", b""),
            Some(false)
        );

        for (label, exit_code, stdout, stderr) in [
            ("unknown", Some(0), &b"whatever\n"[..], &b""[..]),
            (
                "disabled-with-warning",
                Some(0),
                &b"SecureBoot disabled\n"[..],
                &b"warning\n"[..],
            ),
            (
                "disabled-on-stderr",
                Some(0),
                &b""[..],
                &b"SecureBoot disabled\n"[..],
            ),
            (
                "ambiguous-multiline",
                Some(0),
                &b"SecureBoot enabled\nSecureBoot disabled\n"[..],
                &b""[..],
            ),
            (
                "invalid-utf8",
                Some(0),
                &b"SecureBoot disabled\n\xff"[..],
                &b""[..],
            ),
            (
                "failed-status",
                Some(1),
                &b"SecureBoot disabled\n"[..],
                &b""[..],
            ),
        ] {
            assert_eq!(
                classify_secure_boot_output(exit_code, stdout, stderr),
                None,
                "{label} não pode afrouxar a exigência de assinatura",
            );
        }
    }

    #[test]
    fn certificate_subject_parser_extracts_cn() {
        assert_eq!(
            certificate_cn_from_subject("subject=CN=fedora_example,O=fedora").as_deref(),
            Some("fedora_example")
        );
    }

    #[test]
    fn v2_mok_marker_is_strict_and_legacy_cn_remains_parseable() {
        let marker = "version=2\ncn=galaxybook-akmods\nfingerprint_sha1=00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\ncertificate_serial=112233445566778899AABBCCDDEEFF0011223344\n";
        let AkmodsKeyMarker::V2(identity) =
            parse_akmods_key_marker(marker).expect("marker v2 válido rejeitado")
        else {
            panic!("marker v2 classificado como legado")
        };
        assert_eq!(identity.cn, "galaxybook-akmods");
        assert!(matches!(
            parse_akmods_key_marker("legacy-akmods-cn\n"),
            Some(AkmodsKeyMarker::LegacyCn(cn)) if cn == "legacy-akmods-cn"
        ));
        assert!(parse_akmods_key_marker(marker.trim_end()).is_none());
        assert!(parse_akmods_key_marker(&marker.replace("version=2", "version=3")).is_none());
        assert!(parse_akmods_key_marker(&format!("{marker}extra=true\n")).is_none());
        assert!(parse_akmods_key_marker(&marker.replace("AA:BB", "aa:bb")).is_none());
        assert!(
            parse_akmods_key_marker(
                &marker.replace("certificate_serial=1122", "certificate_serial=11:22")
            )
            .is_none()
        );
        assert!(
            parse_akmods_key_marker(&marker.replace("galaxybook-akmods", "Galaxy Book akmods"))
                .is_none()
        );
        assert!(
            parse_akmods_key_marker(&marker.replace(
                "certificate_serial=112233445566778899AABBCCDDEEFF0011223344",
                "certificate_serial=0000"
            ))
            .is_none()
        );
    }

    #[test]
    fn trusted_mok_marker_reader_rejects_hardlinks() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio inválido")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "galaxybook-setup-mok-marker-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&test_root).expect("não foi possível criar diretório temporário");
        let marker = test_root.join("akmods-key.cn");
        let hardlink = test_root.join("akmods-key-copy.cn");
        fs::write(
            &marker,
            "version=2\ncn=galaxybook-akmods\nfingerprint_sha1=00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\ncertificate_serial=80\n",
        )
        .expect("não foi possível criar marcador MOK de teste");
        assert!(read_trusted_file(marker.to_str().expect("path UTF-8"), false).is_some());
        fs::hard_link(&marker, &hardlink).expect("não foi possível criar hardlink de teste");
        assert!(read_trusted_file(marker.to_str().expect("path UTF-8"), false).is_none());
        fs::remove_file(&hardlink).expect("não foi possível remover hardlink de teste");
        fs::remove_file(&marker).expect("não foi possível remover marcador de teste");
        fs::remove_dir(&test_root).expect("não foi possível remover diretório temporário");
    }

    #[test]
    fn published_certificate_requires_a_bounded_regular_single_link_file() {
        assert!(public_certificate_permissions_are_trusted(
            0, 0, 0o100644, 1
        ));
        assert!(!public_certificate_permissions_are_trusted(
            1000, 0, 0o100644, 1
        ));
        assert!(!public_certificate_permissions_are_trusted(
            0, 1000, 0o100644, 1
        ));
        assert!(!public_certificate_permissions_are_trusted(
            0, 0, 0o100664, 1
        ));
        assert!(!public_certificate_permissions_are_trusted(
            0, 0, 0o100644, 2
        ));

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio inválido")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "galaxybook-setup-public-certificate-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&test_root).expect("não foi possível criar diretório temporário");
        let certificate = test_root.join("akmods-public-key.der");
        let hardlink = test_root.join("akmods-public-key-hardlink.der");
        let symlink = test_root.join("akmods-public-key-symlink.der");
        let replacement = test_root.join("akmods-public-key-replacement.der");
        fs::write(&certificate, b"not-a-der-certificate")
            .expect("não foi possível criar certificado público de teste");

        assert!(trusted_public_certificate_fingerprint(&certificate, false, false).is_some());
        let opened = open_trusted_public_certificate(&certificate, false, false)
            .expect("certificado público regular foi rejeitado antes do parse");
        assert_eq!(opened.der, b"not-a-der-certificate");
        assert!(opened_public_certificate_still_matches_path(
            &opened,
            &certificate,
            false,
            false,
        ));
        assert!(published_certificate_identity_from_path(&certificate, false, false).is_none());

        fs::hard_link(&certificate, &hardlink)
            .expect("não foi possível criar hardlink de certificado de teste");
        assert!(trusted_public_certificate_fingerprint(&certificate, false, false).is_none());
        assert!(open_trusted_public_certificate(&certificate, false, false).is_none());
        assert!(!opened_public_certificate_still_matches_path(
            &opened,
            &certificate,
            false,
            false,
        ));
        fs::remove_file(&hardlink).expect("não foi possível remover hardlink de teste");

        std::os::unix::fs::symlink(&certificate, &symlink)
            .expect("não foi possível criar symlink de certificado de teste");
        assert!(trusted_public_certificate_fingerprint(&symlink, false, false).is_none());
        assert!(open_trusted_public_certificate(&symlink, false, false).is_none());
        fs::remove_file(&symlink).expect("não foi possível remover symlink de teste");

        fs::write(&replacement, b"atomically-replaced-public-certificate")
            .expect("não foi possível criar substituto de certificado de teste");
        fs::rename(&replacement, &certificate)
            .expect("não foi possível substituir certificado de teste");
        assert!(!opened_public_certificate_still_matches_path(
            &opened,
            &certificate,
            false,
            false,
        ));
        drop(opened);

        fs::File::create(&certificate)
            .and_then(|file| file.set_len(MAX_PUBLIC_CERTIFICATE_SIZE + 1))
            .expect("não foi possível redimensionar certificado de teste");
        assert!(trusted_public_certificate_fingerprint(&certificate, false, false).is_none());
        assert!(open_trusted_public_certificate(&certificate, false, false).is_none());

        fs::File::create(&certificate).expect("não foi possível zerar certificado de teste");
        assert!(trusted_public_certificate_fingerprint(&certificate, false, false).is_none());
        assert!(open_trusted_public_certificate(&certificate, false, false).is_none());

        fs::remove_file(&certificate).expect("não foi possível remover certificado de teste");
        fs::remove_dir(&test_root).expect("não foi possível remover diretório temporário");
    }

    #[test]
    fn public_certificate_identity_parser_requires_cn_fingerprint_and_serial() {
        let output = "subject=CN=Galaxy Book akmods,O=fedora\nsha1 Fingerprint=00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\nserial=112233445566778899AABBCCDDEEFF0011223344";
        let identity = certificate_identity_from_output(output)
            .expect("identidade completa do certificado foi rejeitada");
        assert_eq!(identity.cn, "Galaxy Book akmods");
        assert!(
            certificate_identity_from_output(&output.replace("sha1 Fingerprint=", "")).is_none()
        );
        assert!(certificate_identity_from_output(&output.replace("serial=", "other=")).is_none());
        assert!(certificate_identity_from_output(&format!("{output}\nextra=true")).is_none());
        assert!(
            certificate_identity_from_output(&format!("subject=CN=duplicate,O=fedora\n{output}"))
                .is_none()
        );
        assert!(
            certificate_identity_from_output(&output.replace(
                "serial=112233445566778899AABBCCDDEEFF0011223344",
                "serial=112233445566778899AABBCCDDEEFF0011223344\nserial=01"
            ))
            .is_none()
        );
        assert!(
            certificate_identity_from_output(&output.replacen('\n', "\nwarning\n", 1)).is_none()
        );
        assert_eq!(
            normalize_certificate_serial("11:22:33:44"),
            Some("11223344".into())
        );
        // OpenSSL and kmod both omit ASN.1 INTEGER sign padding. Canonicalize
        // it here as well so marker and modinfo comparisons stay numeric.
        assert_eq!(normalize_certificate_serial("00:80"), Some("80".into()));
        assert_eq!(normalize_certificate_serial("00:01"), Some("01".into()));
        assert!(normalize_certificate_serial("1").is_none());
        assert!(normalize_certificate_serial("0000").is_none());
        assert!(normalize_certificate_serial(&"AA".repeat(21)).is_none());
    }

    #[test]
    fn stale_marker_is_rejected_even_when_cn_was_reused_after_rotation() {
        let current = AkmodsKeyIdentity {
            cn: "same-cn".into(),
            fingerprint_sha1: "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "112233445566778899AABBCCDDEEFF0011223344".into(),
        };
        let rotated_marker = AkmodsKeyIdentity {
            cn: current.cn.clone(),
            fingerprint_sha1: "FF:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "AA2233445566778899AABBCCDDEEFF0011223344".into(),
        };
        assert_eq!(
            validated_key_identity(Some(current.clone()), Some(current.clone())),
            Some(current.clone())
        );
        let identity_after_rotation = validated_key_identity(Some(current), Some(rotated_marker));
        assert_eq!(identity_after_rotation, None);
        assert!(!module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            Some("same-cn"),
            Some("112233445566778899AABBCCDDEEFF0011223344"),
            true,
            identity_after_rotation.as_ref(),
        ));
        assert_eq!(validated_key_identity(None, None), None);
        assert_eq!(
            v2_marker_identity(Some(AkmodsKeyMarker::LegacyCn("same-cn".into()))),
            None
        );
    }

    #[test]
    fn published_identity_proves_enrollment_with_protected_original_paths() {
        let published_certificate = AkmodsKeyIdentity {
            cn: "galaxybook-akmods".into(),
            fingerprint_sha1: "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "112233445566778899AABBCCDDEEFF0011223344".into(),
        };
        let published_marker = published_certificate.clone();
        let identity =
            validated_key_identity(Some(published_certificate.clone()), Some(published_marker));
        let enrolled =
            "SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\n";

        assert_eq!(identity, Some(published_certificate));
        // Enrollment is proven from the trusted public DER plus its v2 marker;
        // the protected original certificate/private-key paths are deliberately
        // not inputs to this decision.
        assert!(mok_enrollment_is_proven(
            identity.as_ref(),
            Some(enrolled),
            false,
        ));
        assert!(mok_enrollment_is_proven(identity.as_ref(), None, true));
        assert!(!mok_enrollment_is_proven(None, Some(enrolled), true));
    }

    #[test]
    fn v2_mok_matching_uses_fingerprint_not_a_reused_cn() {
        let identity = AkmodsKeyIdentity {
            cn: "same-cn".into(),
            fingerprint_sha1: "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "112233445566778899AABBCCDDEEFF0011223344".into(),
        };
        let wrong_certificate = "[key 1]\nSHA1 Fingerprint: FF:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\nSubject: CN=same-cn,O=fedora";
        assert!(!mok_list_contains_identity(wrong_certificate, &identity));
        let matching_certificate = "[key 2]\nSHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33\nSubject: CN=another-cn,O=fedora";
        assert!(mok_list_contains_identity(matching_certificate, &identity));
        assert!(!mok_list_matches_current_identity(
            "SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33\nSubject: CN=same-cn",
            None,
        ));
        assert!(mok_pending_output_matches_current_identity(
            true,
            matching_certificate,
            Some(&identity),
        ));
        assert!(!mok_pending_output_matches_current_identity(
            false,
            matching_certificate,
            Some(&identity),
        ));
        assert!(!mok_pending_output_matches_current_identity(
            true,
            wrong_certificate,
            Some(&identity),
        ));
        assert!(!mok_pending_output_matches_current_identity(
            true,
            "",
            Some(&identity),
        ));
        assert!(!mok_pending_output_matches_current_identity(
            true,
            matching_certificate,
            None,
        ));
        let legacy_marker = v2_marker_identity(Some(AkmodsKeyMarker::LegacyCn("same-cn".into())));
        assert!(!mok_pending_output_matches_current_identity(
            true,
            matching_certificate,
            legacy_marker.as_ref(),
        ));

        let rotated_marker = AkmodsKeyIdentity {
            cn: identity.cn.clone(),
            fingerprint_sha1: "FF:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "AA2233445566778899AABBCCDDEEFF0011223344".into(),
        };
        let stale_identity = validated_key_identity(Some(identity), Some(rotated_marker));
        assert_eq!(stale_identity, None);
        assert!(!mok_pending_output_matches_current_identity(
            true,
            matching_certificate,
            stale_identity.as_ref(),
        ));
    }

    #[test]
    fn trusted_state_hierarchy_requires_root_root_and_exact_modes() {
        assert!(root_directory_metadata_is_trusted(
            true, 0, 0, 0o40755, 0o755,
        ));
        assert!(!root_directory_metadata_is_trusted(
            true, 0, 1, 0o40755, 0o755,
        ));
        assert!(!root_directory_metadata_is_trusted(
            true, 0, 0, 0o40750, 0o755,
        ));
        assert!(!root_directory_metadata_is_trusted(
            false, 0, 0, 0o100755, 0o755,
        ));
    }

    #[test]
    fn loaded_module_requires_binary_identity_signer_vermagic_and_expected_path() {
        let identity = AkmodsKeyIdentity {
            cn: "Galaxy Book akmods".into(),
            fingerprint_sha1: "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "112233445566778899AABBCCDDEEFF0011223344".into(),
        };
        assert!(mok_proof_candidate_module_path(
            "ov02c10",
            "/lib/modules/7.1.8/updates/ov02c10.ko.zst"
        ));
        assert!(!packaged_external_module_path(
            "ov02c10",
            "/lib/modules/7.1.8/updates/ov02c10.ko.zst"
        ));
        assert!(!module_payload_matches(
            "kmod-galaxybook-ov02c10-7.1.8",
            Some("/lib/modules/7.1.8/updates/ov02c10.ko.zst"),
            None,
            "/extra/galaxybook-ov02c10/",
        ));
        assert!(loaded_module_matches_candidate(
            true,
            Some("Galaxy Book akmods"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            Some("7.1.8 SMP preempt mod_unload"),
            "7.1.8",
            true,
            true,
            Some(&identity),
        ));
        assert!(!loaded_module_matches_candidate(
            false,
            Some("Galaxy Book akmods"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            Some("7.1.8 SMP preempt mod_unload"),
            "7.1.8",
            true,
            true,
            Some(&identity),
        ));
        assert!(!loaded_module_matches_candidate(
            true,
            Some("Galaxy Book akmods"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            Some("7.1.9 SMP preempt mod_unload"),
            "7.1.8",
            true,
            true,
            Some(&identity),
        ));
        assert!(!loaded_module_matches_candidate(
            false,
            Some("Galaxy Book akmods"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            Some("7.1.8 SMP preempt mod_unload"),
            "7.1.8",
            true,
            true,
            Some(&identity),
        ));
        assert!(!loaded_module_matches_candidate(
            true,
            Some("Galaxy Book akmods"),
            Some("AA:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            Some("7.1.8 SMP preempt mod_unload"),
            "7.1.8",
            true,
            true,
            Some(&identity),
        ));
    }

    #[test]
    fn kernel_release_parser_includes_running_and_installed_kernels() {
        assert_eq!(
            parse_kernel_releases(
                "7.1.6-201.fc44.x86_64",
                "7.1.8-200.fc44.x86_64\n7.1.6-201.fc44.x86_64\n"
            ),
            vec![
                "7.1.6-201.fc44.x86_64".to_string(),
                "7.1.8-200.fc44.x86_64".to_string()
            ]
        );
        assert_eq!(
            complete_kernel_inventory("7.1.6-201.fc44.x86_64", Some("7.1.8-200.fc44.x86_64\n"),)
                .expect("inventário completo ausente")
                .len(),
            2,
        );
        assert!(complete_kernel_inventory("7.1.6-201.fc44.x86_64", None).is_none());
    }

    #[test]
    fn installed_kernel_inventory_requires_conclusive_canonical_rpm_output() {
        let current = "7.1.6-201.fc44.x86_64";
        let complete = "7.1.6-201.fc44.x86_64\n7.1.8-200.fc44.x86_64\n";
        assert_eq!(
            classify_installed_kernel_inventory(current, Some(0), complete.as_bytes(), b""),
            Some(vec![
                "7.1.6-201.fc44.x86_64".to_string(),
                "7.1.8-200.fc44.x86_64".to_string(),
            ])
        );

        for (label, exit_code, stdout, stderr) in [
            ("empty", Some(0), &b""[..], &b""[..]),
            (
                "warning-after-partial-inventory",
                Some(0),
                &b"7.1.6-201.fc44.x86_64\n"[..],
                &b"rpm database warning\n"[..],
            ),
            (
                "missing-final-newline",
                Some(0),
                &b"7.1.6-201.fc44.x86_64"[..],
                &b""[..],
            ),
            ("unsafe-release", Some(0), &b"../../kernel\n"[..], &b""[..]),
            (
                "whitespace-padded-release",
                Some(0),
                &b" 7.1.6-201.fc44.x86_64\n"[..],
                &b""[..],
            ),
            (
                "invalid-utf8",
                Some(0),
                &b"7.1.6-201.fc44.x86_64\n\xff"[..],
                &b""[..],
            ),
            (
                "query-failure",
                Some(1),
                &b"7.1.6-201.fc44.x86_64\n"[..],
                &b""[..],
            ),
        ] {
            assert!(
                classify_installed_kernel_inventory(current, exit_code, stdout, stderr).is_none(),
                "{label} não pode produzir um inventário completo",
            );
        }
    }

    #[test]
    fn final_camera_payload_state_wins_over_stale_boot_log() {
        assert!(camera_payload_state_allows_reboot(true, true, &[], true));
        assert!(!camera_payload_state_allows_reboot(
            true,
            true,
            &["7.1.8-200.fc44.x86_64".to_string()],
            true,
        ));
        assert!(!camera_payload_state_allows_reboot(true, true, &[], false));
    }

    #[test]
    fn module_metadata_requires_vermagic_and_secure_boot_signature() {
        let identity = AkmodsKeyIdentity {
            cn: "Galaxy Book akmods".into(),
            fingerprint_sha1: "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33".into(),
            certificate_serial: "112233445566778899AABBCCDDEEFF0011223344".into(),
        };
        assert!(!secure_boot_state_requires_signatures(Some(false)));
        assert!(secure_boot_state_requires_signatures(Some(true)));
        assert!(secure_boot_state_requires_signatures(None));
        assert!(module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            None,
            None,
            false,
            None,
        ));
        assert!(!module_security_metadata_matches(
            "7.1.8",
            Some("7.1.7 SMP preempt mod_unload"),
            Some("Galaxy Book akmods"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            false,
            None,
        ));
        assert!(!module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            None,
            None,
            true,
            Some(&identity),
        ));
        assert!(module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            Some("Galaxy Book akmods"),
            Some("112233445566778899AABBCCDDEEFF0011223344"),
            true,
            Some(&identity),
        ));
        assert!(!module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            Some("Old akmods key"),
            Some("11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            true,
            Some(&identity),
        ));
        assert!(!module_security_metadata_matches(
            "7.1.8",
            Some("7.1.8 SMP preempt mod_unload"),
            Some("Galaxy Book akmods"),
            Some("AA:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44"),
            true,
            Some(&identity),
        ));
    }

    #[test]
    fn readiness_attestation_rejects_missing_symlink_and_stale_markers() {
        fn marker_value(fingerprint: FileFingerprint) -> String {
            format!(
                "{}|{}|{}|{}.{:09}0|{}.{:09}0",
                fingerprint.device,
                fingerprint.inode,
                fingerprint.size,
                fingerprint.mtime_seconds,
                fingerprint.mtime_nanoseconds,
                fingerprint.ctime_seconds,
                fingerprint.ctime_nanoseconds,
            )
        }

        assert!(readiness_marker_permissions_are_trusted(0, 0, 0o100644, 1));
        assert!(!readiness_marker_permissions_are_trusted(
            1000, 0, 0o100644, 1
        ));
        assert!(!readiness_marker_permissions_are_trusted(0, 0, 0o100664, 1));
        assert!(!readiness_marker_permissions_are_trusted(0, 0, 0o104644, 1));
        assert!(!readiness_marker_permissions_are_trusted(0, 0, 0o100644, 2));

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio inválido")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "galaxybook-setup-attestation-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&test_root).expect("não foi possível criar diretório temporário");
        let initramfs = test_root.join("initramfs.img");
        let module = test_root.join("ov02c10.ko.zst");
        let marker = test_root.join("kernel.state");
        let marker_symlink = test_root.join("symlink.state");
        let marker_hardlink = test_root.join("hardlink.state");
        fs::write(&initramfs, b"initramfs externo validado")
            .expect("não foi possível criar initramfs de teste");
        fs::write(&module, b"payload externo").expect("não foi possível criar módulo de teste");

        assert!(!ov_readiness_attestation_matches_files(
            &marker, &initramfs, &module, false,
        ));

        let content = format!(
            "version=1\ninitramfs={}\nmodule={}\n",
            marker_value(file_fingerprint(&initramfs).expect("fingerprint do initramfs")),
            marker_value(file_fingerprint(&module).expect("fingerprint do módulo")),
        );
        fs::write(&marker, content).expect("não foi possível criar marcador de teste");
        assert!(ov_readiness_attestation_matches_files(
            &marker, &initramfs, &module, false,
        ));

        fs::hard_link(&marker, &marker_hardlink).expect("não foi possível criar hardlink de teste");
        assert!(!ov_readiness_attestation_matches_files(
            &marker_hardlink,
            &initramfs,
            &module,
            false,
        ));
        fs::remove_file(&marker_hardlink).expect("não foi possível remover hardlink de teste");

        std::os::unix::fs::symlink(&marker, &marker_symlink)
            .expect("não foi possível criar symlink de teste");
        assert!(!ov_readiness_attestation_matches_files(
            &marker_symlink,
            &initramfs,
            &module,
            false,
        ));

        fs::write(&module, b"payload externo alterado e obsoleto")
            .expect("não foi possível alterar módulo de teste");
        assert!(!ov_readiness_attestation_matches_files(
            &marker, &initramfs, &module, false,
        ));
        let refreshed_content = format!(
            "version=1\ninitramfs={}\nmodule={}\n",
            marker_value(file_fingerprint(&initramfs).expect("fingerprint do initramfs")),
            marker_value(file_fingerprint(&module).expect("fingerprint do módulo")),
        );
        fs::write(&marker, refreshed_content)
            .expect("não foi possível atualizar marcador de teste");
        assert!(ov_readiness_attestation_matches_files(
            &marker, &initramfs, &module, false,
        ));
        fs::write(&initramfs, b"rebuilt initramfs after attestation")
            .expect("não foi possível alterar initramfs de teste");
        assert!(!ov_readiness_attestation_matches_files(
            &marker, &initramfs, &module, false,
        ));
        assert!(!kernel_release_is_safe("../../kernel"));
        assert!(kernel_release_is_safe("7.1.8-200.fc44.x86_64"));

        fs::remove_dir_all(&test_root).expect("não foi possível limpar diretório temporário");
    }

    #[test]
    fn payload_match_rejects_manual_updates_and_wrong_owner() {
        let package = "kmod-galaxybook-max98390-7.1.8-200.fc44.x86_64";
        assert!(!module_payload_matches(
            package,
            Some("/lib/modules/7.1.8-200.fc44.x86_64/updates/snd-hda-scodec-max98390.ko"),
            None,
            "/extra/galaxybook-max98390/"
        ));
        assert!(!module_payload_matches(
            package,
            Some(
                "/lib/modules/7.1.8-200.fc44.x86_64/extra/galaxybook-max98390/snd-hda-scodec-max98390.ko.xz"
            ),
            Some("another-package"),
            "/extra/galaxybook-max98390/"
        ));
        assert!(module_payload_matches(
            package,
            Some(
                "/lib/modules/7.1.8-200.fc44.x86_64/extra/galaxybook-max98390/snd-hda-scodec-max98390.ko.xz"
            ),
            Some(package),
            "/extra/galaxybook-max98390/"
        ));
    }

    #[test]
    fn max98390_identifier_accepts_acpi_aliases() {
        assert!(max98390_identifier_matches("MAX98390:00"));
        assert!(max98390_identifier_matches("MX98390:00"));
        assert!(!max98390_identifier_matches("MAX98389:00"));
    }

    #[test]
    fn max98390_discovery_fails_closed_on_directory_entry_and_name_errors() {
        assert_eq!(
            classify_max98390_identifier_scan(Err(())),
            HardwareDiscovery::Unknown
        );
        assert_eq!(
            classify_max98390_identifier_scan(Ok(vec![Err(())])),
            HardwareDiscovery::Unknown
        );
        assert_eq!(
            classify_max98390_identifier_scan(Ok(vec![])),
            HardwareDiscovery::Absent
        );
        assert_eq!(
            classify_max98390_identifier_scan(Ok(vec![Ok("MX98390:00".into())])),
            HardwareDiscovery::Present
        );

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio inválido")
            .as_nanos();
        let test_root = std::env::temp_dir().join(format!(
            "galaxybook-setup-max-discovery-{}-{nonce}",
            std::process::id()
        ));
        let acpi = test_root.join("acpi");
        let i2c = test_root.join("i2c");
        let i2c_device = i2c.join("i2c-test");
        fs::create_dir_all(&acpi).expect("não foi possível criar fixture ACPI");
        fs::create_dir_all(&i2c_device).expect("não foi possível criar fixture I2C");

        assert_eq!(
            max98390_hardware_discovery_in(&acpi, &i2c),
            HardwareDiscovery::Unknown,
            "a falha ao ler name de uma entrada I2C virou ausência"
        );
        fs::write(i2c_device.join("name"), b"MAX98390\n").expect("não foi possível criar name I2C");
        assert_eq!(
            max98390_hardware_discovery_in(&acpi, &i2c),
            HardwareDiscovery::Present
        );
        assert_eq!(
            max98390_hardware_discovery_in(&test_root.join("missing"), &i2c),
            HardwareDiscovery::Unknown,
            "a falha de read_dir ACPI virou ausência"
        );
        fs::remove_dir_all(&test_root).expect("não foi possível remover fixture MAX98390");
    }

    #[test]
    fn unknown_max98390_hardware_blocks_an_installed_driver_family_reboot() {
        let packages = PackagePresence {
            installed: vec![
                "galaxybook-max98390-kmod-common".into(),
                "akmod-galaxybook-max98390".into(),
                "kmod-galaxybook-max98390".into(),
            ],
            missing: vec![],
            unknown: vec![],
        };
        let check = detect_speakers_check_with_hardware(
            HardwareDiscovery::Unknown,
            false,
            None,
            &[],
            true,
            &packages,
        );

        assert_eq!(check.code, "speakers-hardware-query-unknown");
        assert_eq!(check.health, Health::Unknown);
        assert!(!max_driver_state_allows_reboot(true, check.code));
        assert!(!max_driver_state_allows_reboot(false, check.code));
    }

    #[test]
    fn v4l2_capture_probe_requires_clean_stdout_and_an_exact_capability_line() {
        let valid = b"Driver Info:\n\tCapabilities:\n\t\tVideo Capture\n";
        assert!(classify_v4l2_capture_probe(Some(0), valid, b""));
        for (exit_code, stdout, stderr) in [
            (Some(1), valid.as_slice(), b"".as_slice()),
            (Some(0), valid.as_slice(), b"warning\n".as_slice()),
            (Some(0), b"".as_slice(), b"Video Capture\n".as_slice()),
            (Some(0), b"Video Capture".as_slice(), b"".as_slice()),
            (Some(0), b"Not Video Capture\n".as_slice(), b"".as_slice()),
            (Some(0), b"Video Capture\0\n".as_slice(), b"".as_slice()),
            (Some(0), b"Video Capture\r\n".as_slice(), b"".as_slice()),
            (Some(0), b"\xff\n".as_slice(), b"".as_slice()),
        ] {
            assert!(
                !classify_v4l2_capture_probe(exit_code, stdout, stderr),
                "uma consulta V4L2 inconclusiva provou capacidade de captura"
            );
        }
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
            unknown: vec![],
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
            "browser-missing",
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
            unknown: vec![],
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
            "browser-ready",
            false,
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
        let item =
            dash_to_dock_check_from_state(true, true, &["auto-ocultação", "lixeira visível"]);
        assert_eq!(item.health, Health::Warning);
        assert_eq!(item.code, "dash-to-dock-mismatch");
        assert!(item.detail.contains("auto-ocultação"));
        assert!(item.detail.contains("lixeira visível"));
    }

    #[test]
    fn browser_sensor_uses_live_driver_links_for_all_present_ov_devices() {
        let root =
            std::env::temp_dir().join(format!("galaxybook-browser-sensor-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        assert!(!all_ov_sensors_unbound(
            &root.join("unreadable-or-absent-bus")
        ));
        fs::create_dir(root.join("i2c-OTHER:00")).unwrap();
        assert!(!all_ov_sensors_unbound(&root));
        let sensor = root.join("i2c-OVTI02C1:00");
        let other_ov = root.join("i2c-OVTI02C1:01");
        fs::create_dir(&sensor).unwrap();
        fs::create_dir(&other_ov).unwrap();
        let (check, source_ready) = probe_browser_camera(&root, true);
        assert_eq!(check.code, BROWSER_SENSOR_UNBOUND);
        assert_eq!(check.health, Health::Error);
        assert!(!source_ready);
        std::os::unix::fs::symlink(&root, other_ov.join("driver")).unwrap();
        assert!(!all_ov_sensors_unbound(&root));
        fs::remove_file(other_ov.join("driver")).unwrap();
        assert!(all_ov_sensors_unbound(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn browser_camera_check_warns_when_bridge_and_system_source_are_missing() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec![],
            unknown: vec![],
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
            unknown: vec![],
        };
        let item = detect_browser_camera_check(&packages, false, true);
        assert_eq!(item.health, Health::Warning);
        assert!(item.detail.contains("já aparece nas fontes do sistema"));
    }

    #[test]
    fn package_check_does_not_report_an_unknown_query_as_missing() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec![],
            unknown: vec!["v4l2-relayd".into()],
        };
        let item = detect_browser_camera_check(&packages, false, false);
        assert_eq!(item.health, Health::Unknown);
        assert_eq!(item.code, "browser-packages-query-unknown");
        assert!(!item.detail.contains("Faltando pacotes"));
    }

    #[test]
    fn recommendation_keeps_unknown_main_packages_actionable() {
        let packages = PackagePresence {
            installed: vec![],
            missing: vec![],
            unknown: vec!["akmod-galaxybook-ov02c10".into()],
        };
        let (title, detail) = recommend_next_step(
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
            "browser-missing",
            false,
            false,
            false,
            false,
        );
        assert_eq!(title, "Consulta de pacotes inconclusiva");
        assert!(detail.contains("não os tratou como ausentes"));
    }

    #[test]
    fn browser_camera_recommendation_kicks_in_after_libcamera_is_ready() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
            unknown: vec![],
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
            true,
            false,
            true,
            "browser-missing",
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "Compatibilidade com navegador pendente");
        let (title, detail) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            false,
            true,
            true,
            true,
            false,
            true,
            BROWSER_SENSOR_UNBOUND,
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "Reparar o driver");
        assert_eq!(detail, browser_sensor_unbound_detail());
    }

    #[test]
    fn speaker_recommendation_appears_when_camera_flow_is_ready() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
            unknown: vec![],
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
            true,
            false,
            true,
            "browser-ready",
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
            unknown: vec![],
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
            true,
            false,
            true,
            "browser-ready",
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
            unknown: vec![],
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
            "browser-missing",
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
            unknown: vec![],
        };
        let (title, detail) = recommend_next_step(
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
            "browser-ready",
            true,
            false,
            false,
            false,
        );
        assert_eq!(title, "O bridge do navegador bloqueou o libcamera");
        assert!(detail.contains("remova-a manualmente como administrador"));
        assert!(!detail.contains("Reaplique a ação rápida"));
        assert!(!detail.contains("migrar o bridge"));
    }

    #[test]
    fn recommendation_detects_late_driver_after_kernel_update() {
        let packages = PackagePresence {
            installed: vec!["akmod-galaxybook-ov02c10".into()],
            missing: vec![],
            unknown: vec![],
        };
        let (title, _) = recommend_next_step(
            &ok_secure_boot_key_item(),
            &packages,
            false,
            ModuleOrigin::Patched,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            "browser-ready",
            true,
            true,
            true,
            true,
        );
        assert_eq!(title, "Reinicie com o driver corrigido priorizado");
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
