pub(crate) mod runtime;

use crate::actions::ActionKey;
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
        DiagnosticKey::Gpu => &snapshot.gpu,
        DiagnosticKey::PlatformProfile => &snapshot.platform_profile,
        DiagnosticKey::Clipboard => &snapshot.clipboard_extension,
        DiagnosticKey::Gsconnect => &snapshot.gsconnect_extension,
        DiagnosticKey::DesktopIcons => &snapshot.desktop_icons_extension,
        DiagnosticKey::Dock => &snapshot.dock_extension,
    }
}

fn diagnostic_items(snapshot: &SetupSnapshot) -> [&CheckItem; 14] {
    [
        &snapshot.packages,
        &snapshot.akmods,
        &snapshot.module,
        &snapshot.libcamera,
        &snapshot.browser_camera,
        &snapshot.boot,
        &snapshot.speakers,
        &snapshot.sound_app,
        &snapshot.gpu,
        &snapshot.platform_profile,
        &snapshot.clipboard_extension,
        &snapshot.gsconnect_extension,
        &snapshot.desktop_icons_extension,
        &snapshot.dock_extension,
    ]
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
        DiagnosticKey::Packages => vec![ActionKey::InstallMainSupport, ActionKey::OpenCamera],
        DiagnosticKey::Akmods => vec![ActionKey::RepairDriver, ActionKey::Reboot],
        DiagnosticKey::Module => {
            if item.code == "module-not-loaded" {
                vec![ActionKey::EnableCameraModule, ActionKey::Reboot]
            } else if item.code == "module-manual-override" {
                vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot]
            } else if item.code == "module-in-tree" {
                vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
            } else {
                vec![ActionKey::RepairDriver, ActionKey::Reboot]
            }
        }
        DiagnosticKey::Libcamera => {
            if item.health == Health::Good {
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
            }
        }
        DiagnosticKey::BrowserCamera => {
            if item.health == Health::Good {
                vec![ActionKey::OpenCamera]
            } else {
                vec![ActionKey::EnableBrowserCamera, ActionKey::Reboot]
            }
        }
        DiagnosticKey::Boot => {
            if item.health == Health::Good {
                vec![ActionKey::OpenCamera]
            } else {
                vec![
                    ActionKey::RepairDriver,
                    ActionKey::ForceDriverPriority,
                    ActionKey::Reboot,
                ]
            }
        }
        DiagnosticKey::Speakers => {
            if item.health == Health::Good {
                if snapshot.sound_app_installed {
                    vec![ActionKey::OpenSoundApp]
                } else {
                    vec![ActionKey::InstallSoundApp]
                }
            } else if item.health == Health::Error {
                vec![ActionKey::EnableSpeakers]
            } else {
                vec![ActionKey::EnableSpeakers, ActionKey::Reboot]
            }
        }
        DiagnosticKey::SoundApp => {
            if item.code == "sound-app-ready" {
                vec![ActionKey::OpenSoundApp]
            } else {
                vec![ActionKey::InstallSoundApp]
            }
        }
        DiagnosticKey::Gpu => vec![ActionKey::RepairNvidia, ActionKey::Reboot],
        DiagnosticKey::PlatformProfile => vec![ActionKey::SetBalancedProfile],
        DiagnosticKey::Clipboard => vec![ActionKey::ApplyClipboardProfile],
        DiagnosticKey::Gsconnect => vec![ActionKey::ApplyGsconnectProfile],
        DiagnosticKey::DesktopIcons => vec![ActionKey::ApplyDesktopIconsProfile],
        DiagnosticKey::Dock => vec![ActionKey::ApplyDockProfile],
    }
}
