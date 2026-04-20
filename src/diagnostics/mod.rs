pub(crate) mod runtime;

use crate::{ActionKey, DiagnosticKey};
use galaxybook_setup::{APP_NAME, CheckItem, Health, SetupSnapshot};

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
        DiagnosticKey::Gpu => &snapshot.gpu,
        DiagnosticKey::PlatformProfile => &snapshot.platform_profile,
        DiagnosticKey::Clipboard => &snapshot.clipboard_extension,
        DiagnosticKey::Gsconnect => &snapshot.gsconnect_extension,
        DiagnosticKey::DesktopIcons => &snapshot.desktop_icons_extension,
    }
}

fn diagnostic_items(snapshot: &SetupSnapshot) -> [&CheckItem; 12] {
    [
        &snapshot.packages,
        &snapshot.akmods,
        &snapshot.module,
        &snapshot.libcamera,
        &snapshot.browser_camera,
        &snapshot.boot,
        &snapshot.speakers,
        &snapshot.gpu,
        &snapshot.platform_profile,
        &snapshot.clipboard_extension,
        &snapshot.gsconnect_extension,
        &snapshot.desktop_icons_extension,
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
    format!("{APP_NAME}: {}", diagnostic_counts_summary(counts))
}

pub(crate) fn diagnostic_notification_body(
    snapshot: &SetupSnapshot,
    counts: DiagnosticAlertCounts,
) -> String {
    format!(
        "{} Próximo passo: {}",
        diagnostic_counts_summary(counts),
        snapshot.recommendation_body
    )
}

pub(crate) fn diagnostic_counts_summary(counts: DiagnosticAlertCounts) -> String {
    let mut parts = Vec::new();

    if counts.errors > 0 {
        parts.push(format!(
            "{} {}",
            counts.errors,
            pluralize(counts.errors, "erro", "erros")
        ));
    }

    if counts.warnings > 0 {
        parts.push(format!(
            "{} {}",
            counts.warnings,
            pluralize(counts.warnings, "alerta", "alertas")
        ));
    }

    if parts.is_empty() {
        "Nenhum alerta nos diagnósticos.".into()
    } else {
        format!("{} nos diagnósticos.", parts.join(" e "))
    }
}

fn pluralize<'a>(value: u32, singular: &'a str, plural: &'a str) -> &'a str {
    if value == 1 {
        singular
    } else {
        plural
    }
}

pub(crate) fn suggested_actions(snapshot: &SetupSnapshot, key: DiagnosticKey) -> Vec<ActionKey> {
    let item = diagnostic_item(snapshot, key);
    match key {
        DiagnosticKey::Packages => vec![ActionKey::InstallMainSupport, ActionKey::OpenCamera],
        DiagnosticKey::Akmods => vec![ActionKey::RepairDriver, ActionKey::Reboot],
        DiagnosticKey::Module => {
            if item.detail.contains("não foi carregado no kernel") {
                vec![ActionKey::EnableCameraModule, ActionKey::Reboot]
            } else if item.detail.contains("override manual") {
                vec![ActionKey::RestoreIntelIpu6, ActionKey::Reboot]
            } else if item.detail.contains("in-tree") {
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
                if diagnostic_item(snapshot, DiagnosticKey::Module)
                    .detail
                    .contains("não foi carregado no kernel")
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
                Vec::new()
            } else if item.health == Health::Error {
                vec![ActionKey::EnableSpeakers]
            } else {
                vec![ActionKey::EnableSpeakers, ActionKey::Reboot]
            }
        }
        DiagnosticKey::Gpu => vec![ActionKey::RepairNvidia, ActionKey::Reboot],
        DiagnosticKey::PlatformProfile => vec![ActionKey::SetBalancedProfile],
        DiagnosticKey::Clipboard | DiagnosticKey::Gsconnect | DiagnosticKey::DesktopIcons => {
            Vec::new()
        }
    }
}

pub(crate) fn dedupe_action_keys(actions: &[ActionKey]) -> Vec<ActionKey> {
    let mut deduped = Vec::with_capacity(actions.len());
    for action in actions {
        if !deduped.contains(action) {
            deduped.push(*action);
        }
    }
    deduped
}
