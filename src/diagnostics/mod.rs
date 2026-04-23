pub(crate) mod runtime;

use crate::actions::{ActionKey, dedupe_action_keys};
use crate::ui::DiagnosticKey;
use galaxybook_setup::{APP_NAME, CheckItem, Health, SetupSnapshot, tr, trf, trn};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DiagnosticAlertCounts {
    pub(crate) warnings: u32,
    pub(crate) errors: u32,
}

impl DiagnosticAlertCounts {
    pub(crate) fn total(self) -> u32 {
        self.warnings + self.errors
    }

    pub(crate) fn is_clear(self) -> bool {
        self.total() == 0
    }
}

pub(crate) fn diagnostic_item(snapshot: &SetupSnapshot, key: DiagnosticKey) -> &CheckItem {
    match key {
        DiagnosticKey::Packages => &snapshot.packages,
        DiagnosticKey::Akmods => &snapshot.akmods,
        DiagnosticKey::Module => &snapshot.module,
        DiagnosticKey::Libcamera => &snapshot.libcamera,
        DiagnosticKey::BrowserCamera => &snapshot.browser_camera,
        DiagnosticKey::Boot => &snapshot.boot,
        DiagnosticKey::Speakers => &snapshot.speakers,
        DiagnosticKey::SoundApp => &snapshot.sound_app,
        DiagnosticKey::FingerprintReader => &snapshot.fingerprint_reader,
        DiagnosticKey::FingerprintLogin => &snapshot.fingerprint_login,
        DiagnosticKey::Gpu => &snapshot.gpu,
        DiagnosticKey::SecureBootKey => &snapshot.secure_boot_key,
        DiagnosticKey::PlatformProfile => &snapshot.platform_profile,
        DiagnosticKey::Clipboard => &snapshot.clipboard_extension,
        DiagnosticKey::Gsconnect => &snapshot.gsconnect_extension,
        DiagnosticKey::DesktopIcons => &snapshot.desktop_icons_extension,
        DiagnosticKey::Dock => &snapshot.dock_extension,
    }
}

fn diagnostic_items(snapshot: &SetupSnapshot) -> [&CheckItem; 17] {
    [
        &snapshot.packages,
        &snapshot.akmods,
        &snapshot.module,
        &snapshot.libcamera,
        &snapshot.browser_camera,
        &snapshot.boot,
        &snapshot.speakers,
        &snapshot.sound_app,
        &snapshot.fingerprint_reader,
        &snapshot.fingerprint_login,
        &snapshot.gpu,
        &snapshot.secure_boot_key,
        &snapshot.platform_profile,
        &snapshot.clipboard_extension,
        &snapshot.gsconnect_extension,
        &snapshot.desktop_icons_extension,
        &snapshot.dock_extension,
    ]
}

fn with_secure_boot_actions(
    snapshot: &SetupSnapshot,
    mut actions: Vec<ActionKey>,
) -> Vec<ActionKey> {
    match snapshot.secure_boot_key.code {
        "mok-key-missing" | "mok-not-enrolled" => {
            actions.insert(0, ActionKey::PrepareSecureBootKey);
            actions.push(ActionKey::Reboot);
        }
        "mok-pending-enrollment" => {
            actions.insert(0, ActionKey::Reboot);
        }
        _ => {}
    }

    dedupe_action_keys(&actions)
}

pub(crate) fn diagnostic_alert_counts(snapshot: &SetupSnapshot) -> DiagnosticAlertCounts {
    diagnostic_items(snapshot)
        .into_iter()
        .fold(DiagnosticAlertCounts::default(), |mut counts, item| {
            match item.health {
                Health::Warning => counts.warnings += 1,
                Health::Error => counts.errors += 1,
                Health::Good | Health::Unknown => {}
            }
            counts
        })
}

pub(crate) fn diagnostic_notification_title(counts: DiagnosticAlertCounts) -> String {
    trf(
        "{app_name}: {summary}",
        &[
            ("app_name", APP_NAME.to_string()),
            ("summary", diagnostic_counts_summary(counts)),
        ],
    )
}

pub(crate) fn diagnostic_notification_body(
    snapshot: &SetupSnapshot,
    counts: DiagnosticAlertCounts,
) -> String {
    trf(
        "{summary} Próximo passo: {next_step}",
        &[
            ("summary", diagnostic_counts_summary(counts)),
            ("next_step", snapshot.recommendation_body.clone()),
        ],
    )
}

pub(crate) fn diagnostic_counts_summary(counts: DiagnosticAlertCounts) -> String {
    let mut parts = Vec::new();

    if counts.errors > 0 {
        parts.push(trn("{count} erro", "{count} erros", counts.errors).replace(
            "{count}",
            &counts.errors.to_string(),
        ));
    }

    if counts.warnings > 0 {
        parts.push(
            trn("{count} alerta", "{count} alertas", counts.warnings)
                .replace("{count}", &counts.warnings.to_string()),
        );
    }

    if parts.is_empty() {
        tr("Nenhum alerta nos diagnósticos.")
    } else {
        trf(
            "{alerts} nos diagnósticos.",
            &[("alerts", parts.join(&tr(" e ")))],
        )
    }
}

pub(crate) fn suggested_actions(snapshot: &SetupSnapshot, key: DiagnosticKey) -> Vec<ActionKey> {
    let item = diagnostic_item(snapshot, key);
    match key {
        DiagnosticKey::Packages => with_secure_boot_actions(
            snapshot,
            vec![ActionKey::InstallMainSupport, ActionKey::OpenCamera],
        ),
        DiagnosticKey::Akmods => with_secure_boot_actions(
            snapshot,
            vec![ActionKey::RepairDriver, ActionKey::Reboot],
        ),
        DiagnosticKey::Module => {
            let actions = if item.code == "module-not-loaded" {
                vec![ActionKey::EnableCameraModule, ActionKey::Reboot]
            } else if item.code == "module-manual-override" {
                vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot]
            } else if item.code == "module-in-tree" {
                vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
            } else {
                vec![ActionKey::RepairDriver, ActionKey::Reboot]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::Libcamera => {
            let actions = if item.health == Health::Good {
                vec![ActionKey::EnableBrowserCamera, ActionKey::OpenCamera]
            } else {
                let mut actions = vec![ActionKey::RepairDriver, ActionKey::Reboot];
                if diagnostic_item(snapshot, DiagnosticKey::Module).code
                    == "module-not-loaded"
                {
                    actions.insert(0, ActionKey::EnableCameraModule);
                } else {
                    actions.insert(0, ActionKey::RestoreIntelIpu6);
                }
                actions
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::BrowserCamera => {
            if item.health == Health::Good {
                vec![ActionKey::OpenCamera]
            } else {
                vec![ActionKey::EnableBrowserCamera, ActionKey::Reboot]
            }
        }
        DiagnosticKey::Boot => {
            let actions = if item.health == Health::Good {
                vec![ActionKey::OpenCamera]
            } else {
                vec![
                    ActionKey::RepairDriver,
                    ActionKey::ForceDriverPriority,
                    ActionKey::Reboot,
                ]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::Speakers => {
            let actions = if item.health == Health::Good {
                if snapshot.sound_app_installed {
                    vec![ActionKey::OpenSoundApp]
                } else {
                    vec![ActionKey::InstallSoundApp]
                }
            } else if item.health == Health::Error {
                vec![ActionKey::EnableSpeakers]
            } else {
                vec![ActionKey::EnableSpeakers, ActionKey::Reboot]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::SoundApp => {
            if item.code == "sound-app-ready" {
                vec![ActionKey::OpenSoundApp]
            } else {
                vec![ActionKey::InstallSoundApp]
            }
        }
        DiagnosticKey::FingerprintReader => {
            if item.code == "fingerprint-reader-ready" {
                vec![ActionKey::OpenFingerprintSettings]
            } else {
                vec![ActionKey::RepairFingerprintStack]
            }
        }
        DiagnosticKey::FingerprintLogin => match item.code {
            "fingerprint-login-ready" => vec![ActionKey::OpenFingerprintSettings],
            "fingerprint-auth-disabled" => vec![
                ActionKey::EnableFingerprintAuth,
                ActionKey::OpenFingerprintSettings,
            ],
            "fingerprint-enrollment-missing"
            | "fingerprint-auth-and-enrollment-missing" => vec![
                ActionKey::OpenFingerprintSettings,
                ActionKey::EnableFingerprintAuth,
            ],
            _ => vec![
                ActionKey::RepairFingerprintStack,
                ActionKey::EnableFingerprintAuth,
                ActionKey::OpenFingerprintSettings,
            ],
        },
        DiagnosticKey::Gpu => with_secure_boot_actions(
            snapshot,
            vec![ActionKey::RepairNvidia, ActionKey::Reboot],
        ),
        DiagnosticKey::SecureBootKey => match item.code {
            "mok-enrolled" | "mok-not-needed" => vec![],
            "mok-pending-enrollment" => vec![ActionKey::Reboot],
            _ => vec![ActionKey::PrepareSecureBootKey, ActionKey::Reboot],
        },
        DiagnosticKey::PlatformProfile => vec![ActionKey::SetBalancedProfile],
        DiagnosticKey::Clipboard => vec![ActionKey::ApplyClipboardProfile],
        DiagnosticKey::Gsconnect => vec![ActionKey::ApplyGsconnectProfile],
        DiagnosticKey::DesktopIcons => vec![ActionKey::ApplyDesktopIconsProfile],
        DiagnosticKey::Dock => vec![ActionKey::ApplyDockProfile],
    }
}
