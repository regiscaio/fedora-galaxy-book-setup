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
    if actions.is_empty() {
        return actions;
    }

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
        DiagnosticKey::Packages => {
            let actions = if item.code == "packages-missing" {
                vec![ActionKey::InstallMainSupport]
            } else {
                vec![]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::Akmods => {
            let actions = if item.code == "akmods-failed" {
                vec![ActionKey::RepairDriver, ActionKey::Reboot]
            } else {
                vec![]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::Module => {
            let actions = match item.code {
                "module-not-loaded" => {
                    vec![ActionKey::EnableCameraModule, ActionKey::Reboot]
                }
                "module-manual-override" => {
                    vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot]
                }
                "module-in-tree" => {
                    vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
                }
                "module-missing"
                | "module-unknown"
                | "module-patched-path-missing" => {
                    vec![ActionKey::RepairDriver, ActionKey::Reboot]
                }
                _ => vec![],
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::Libcamera => {
            let actions = if item.health == Health::Good {
                vec![]
            } else {
                match item.code {
                    "libcamera-permission-blocked" => {
                        vec![ActionKey::EnableBrowserCamera]
                    }
                    "libcamera-clock-probe-failed" => {
                        vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
                    }
                    _ => match diagnostic_item(snapshot, DiagnosticKey::Module).code {
                        "module-not-loaded" => {
                            vec![ActionKey::EnableCameraModule, ActionKey::Reboot]
                        }
                        "module-in-tree" => {
                            vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
                        }
                        "module-manual-override" | "module-patched" => {
                            vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot]
                        }
                        "module-missing"
                        | "module-unknown"
                        | "module-patched-path-missing" => {
                            vec![ActionKey::RepairDriver, ActionKey::Reboot]
                        }
                        _ => vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot],
                    },
                }
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::BrowserCamera => {
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::EnableBrowserCamera, ActionKey::Reboot]
            }
        }
        DiagnosticKey::Boot => {
            let actions = if item.health == Health::Good {
                vec![]
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
                vec![]
            } else if item.code == "speakers-unsupported" {
                vec![]
            } else if item.health == Health::Error {
                vec![ActionKey::EnableSpeakers]
            } else {
                vec![ActionKey::EnableSpeakers, ActionKey::Reboot]
            };
            with_secure_boot_actions(snapshot, actions)
        }
        DiagnosticKey::SoundApp => {
            if item.code == "sound-app-ready" {
                vec![]
            } else if snapshot.speakers.health != Health::Good
                && snapshot.speakers.code != "speakers-unsupported"
            {
                vec![ActionKey::EnableSpeakers, ActionKey::InstallSoundApp]
            } else {
                vec![ActionKey::InstallSoundApp]
            }
        }
        DiagnosticKey::FingerprintReader => {
            if item.code == "fingerprint-reader-ready" {
                vec![]
            } else {
                vec![ActionKey::RepairFingerprintStack]
            }
        }
        DiagnosticKey::FingerprintLogin => match item.code {
            "fingerprint-login-ready" => vec![],
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
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::RepairNvidia, ActionKey::Reboot]
            },
        ),
        DiagnosticKey::SecureBootKey => match item.code {
            "mok-enrolled" | "mok-not-needed" => vec![],
            "mok-pending-enrollment" => vec![ActionKey::Reboot],
            _ => vec![ActionKey::PrepareSecureBootKey, ActionKey::Reboot],
        },
        DiagnosticKey::PlatformProfile => {
            if item.code == "platform-balanced" {
                vec![]
            } else {
                vec![ActionKey::SetBalancedProfile]
            }
        }
        DiagnosticKey::Clipboard => {
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::ApplyClipboardProfile]
            }
        }
        DiagnosticKey::Gsconnect => {
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::ApplyGsconnectProfile]
            }
        }
        DiagnosticKey::DesktopIcons => {
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::ApplyDesktopIconsProfile]
            }
        }
        DiagnosticKey::Dock => {
            if item.health == Health::Good {
                vec![]
            } else {
                vec![ActionKey::ApplyDockProfile]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galaxybook_setup::SystemSummary;

    fn item(health: Health, code: &'static str) -> CheckItem {
        CheckItem {
            title: "Teste",
            detail: String::new(),
            health,
            code,
        }
    }

    fn test_snapshot() -> SetupSnapshot {
        let ok = item(Health::Good, "ready");
        SetupSnapshot {
            system: SystemSummary {
                notebook: String::new(),
                fedora: String::new(),
                kernel: String::new(),
                secure_boot: String::new(),
            },
            packages: ok.clone(),
            akmods: ok.clone(),
            module: ok.clone(),
            libcamera: ok.clone(),
            browser_camera: ok.clone(),
            boot: ok.clone(),
            speakers: ok.clone(),
            sound_app: item(Health::Warning, "sound-app-missing"),
            fingerprint_reader: ok.clone(),
            fingerprint_login: ok.clone(),
            gpu: ok.clone(),
            secure_boot_key: item(Health::Good, "mok-enrolled"),
            platform_profile: ok.clone(),
            clipboard_extension: ok.clone(),
            gsconnect_extension: ok.clone(),
            desktop_icons_extension: ok.clone(),
            dock_extension: ok,
            recommendation_title: String::new(),
            recommendation_body: String::new(),
            install_main_support_command: String::new(),
            install_command: String::new(),
            repair_command: String::new(),
            enable_camera_module_command: String::new(),
            force_camera_command: String::new(),
            restore_intel_camera_command: String::new(),
            enable_browser_camera_command: String::new(),
            enable_speaker_command: String::new(),
            prepare_secure_boot_key_command: String::new(),
            install_sound_app_command: String::new(),
            repair_nvidia_command: String::new(),
            set_balanced_profile_command: String::new(),
            repair_fingerprint_command: String::new(),
            enable_fingerprint_auth_command: String::new(),
            open_fingerprint_settings_command: String::new(),
            apply_clipboard_profile_command: String::new(),
            apply_gsconnect_profile_command: String::new(),
            apply_desktop_icons_profile_command: String::new(),
            apply_dock_profile_command: String::new(),
            reboot_command: String::new(),
            camera_app_installed: true,
            sound_app_installed: false,
        }
    }

    #[test]
    fn unsupported_speakers_do_not_offer_privileged_actions() {
        let mut snapshot = test_snapshot();
        snapshot.speakers = item(Health::Warning, "speakers-unsupported");

        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Speakers),
            Vec::<ActionKey>::new()
        );
    }

    #[test]
    fn missing_sound_app_suggests_speakers_first_when_speakers_need_work() {
        let mut snapshot = test_snapshot();
        snapshot.speakers = item(Health::Error, "speakers-driver-missing");

        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::SoundApp),
            vec![ActionKey::EnableSpeakers, ActionKey::InstallSoundApp]
        );
    }
}
