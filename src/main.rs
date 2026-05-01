mod actions;
mod diagnostics;
mod system;
mod ui;

use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use galaxybook_setup::{init_i18n, APP_ID, run_smoke_test};
use ui::SetupWindow;

fn main() -> glib::ExitCode {
    init_i18n();
    if std::env::args().any(|arg| arg == "--smoke-test") {
        return match run_smoke_test() {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                glib::ExitCode::FAILURE
            }
        };
    }

    adw::init().expect("Failed to initialize libadwaita");

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        let window = SetupWindow::new(app);
        window.present();
    });

    app.run()
}

#[cfg(test)]
mod tests {
    use crate::diagnostics::{
        diagnostic_alert_counts, diagnostic_counts_summary, suggested_actions,
    };
    use crate::actions::ActionKey;
    use crate::diagnostics::DiagnosticAlertCounts;
    use crate::ui::DiagnosticKey;
    use galaxybook_setup::{
        CheckItem, Health, SetupSnapshot, SystemSummary,
    };

    fn item(title: &'static str, health: Health) -> CheckItem {
        CheckItem {
            title,
            detail: String::new(),
            health,
            code: "",
        }
    }

    fn snapshot_with_healths(healths: [Health; 17]) -> SetupSnapshot {
        SetupSnapshot {
            system: SystemSummary {
                notebook: String::new(),
                fedora: String::new(),
                kernel: String::new(),
                secure_boot: String::new(),
            },
            packages: item("Pacotes principais", healths[0]),
            akmods: item("Akmods no boot", healths[1]),
            module: item("Módulo ativo", healths[2]),
            libcamera: item("Detecção no libcamera", healths[3]),
            browser_camera: item("Navegador e comunicadores", healths[4]),
            boot: item("Erros no boot", healths[5]),
            speakers: item("Alto-falantes internos", healths[6]),
            sound_app: item("Galaxy Book Sound", healths[7]),
            fingerprint_reader: item("Leitor de digital", healths[8]),
            fingerprint_login: item("Login por digital", healths[9]),
            gpu: item("Driver NVIDIA", healths[10]),
            secure_boot_key: item("Chave do Secure Boot", healths[11]),
            platform_profile: item("Perfil de uso", healths[12]),
            clipboard_extension: item(
                "Histórico da área de transferência",
                healths[13],
            ),
            gsconnect_extension: item("GSConnect", healths[14]),
            desktop_icons_extension: item("Ícones na área de trabalho", healths[15]),
            dock_extension: item("Dock do GNOME", healths[16]),
            recommendation_title: String::new(),
            recommendation_body: String::new(),
            install_main_support_command: String::new(),
            install_command: String::new(),
            install_sound_app_command: String::new(),
            repair_command: String::new(),
            enable_camera_module_command: String::new(),
            force_camera_command: String::new(),
            restore_intel_camera_command: String::new(),
            enable_browser_camera_command: String::new(),
            enable_speaker_command: String::new(),
            prepare_secure_boot_key_command: String::new(),
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
            camera_app_installed: false,
            sound_app_installed: false,
        }
    }

    #[test]
    fn counts_only_warnings_and_errors() {
        let snapshot = snapshot_with_healths([
            Health::Good,
            Health::Warning,
            Health::Error,
            Health::Unknown,
            Health::Warning,
            Health::Good,
            Health::Error,
            Health::Good,
            Health::Warning,
            Health::Good,
            Health::Good,
            Health::Warning,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Unknown,
        ]);

        assert_eq!(
            diagnostic_alert_counts(&snapshot),
            DiagnosticAlertCounts {
                warnings: 4,
                errors: 2,
            }
        );
    }

    #[test]
    fn summary_formats_errors_and_warnings() {
        assert_eq!(
            diagnostic_counts_summary(DiagnosticAlertCounts {
                warnings: 2,
                errors: 1,
            }),
            "1 erro e 2 alertas nos diagnósticos."
        );
    }

    #[test]
    fn packages_suggest_main_install_first() {
        let mut snapshot = snapshot_with_healths([
            Health::Warning,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
        ]);
        snapshot.packages.code = "packages-missing";

        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Packages),
            vec![ActionKey::InstallMainSupport]
        );
    }

    #[test]
    fn clipboard_suggests_clipboard_profile() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.clipboard_extension.health = Health::Warning;
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Clipboard),
            vec![ActionKey::ApplyClipboardProfile]
        );
    }

    #[test]
    fn gsconnect_suggests_gsconnect_profile() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.gsconnect_extension.health = Health::Warning;
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Gsconnect),
            vec![ActionKey::ApplyGsconnectProfile]
        );
    }

    #[test]
    fn desktop_icons_suggests_desktop_icons_profile() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.desktop_icons_extension.health = Health::Warning;
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::DesktopIcons),
            vec![ActionKey::ApplyDesktopIconsProfile]
        );
    }

    #[test]
    fn speakers_ready_has_no_suggested_actions() {
        let snapshot = snapshot_with_healths([
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Unknown,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
            Health::Good,
        ]);
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Speakers),
            Vec::<ActionKey>::new()
        );
    }

    #[test]
    fn sound_app_ready_suggests_open() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.sound_app.code = "sound-app-ready";
        snapshot.sound_app_installed = true;
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::SoundApp),
            Vec::<ActionKey>::new()
        );
    }

    #[test]
    fn fingerprint_login_missing_enrollment_suggests_settings_first() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.fingerprint_login.code = "fingerprint-enrollment-missing";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::FingerprintLogin),
            vec![
                ActionKey::OpenFingerprintSettings,
                ActionKey::EnableFingerprintAuth,
            ]
        );
    }

    #[test]
    fn secure_boot_key_suggests_prepare_then_reboot() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.secure_boot_key.health = Health::Error;
        snapshot.secure_boot_key.code = "mok-not-enrolled";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::SecureBootKey),
            vec![ActionKey::PrepareSecureBootKey, ActionKey::Reboot]
        );
    }

    #[test]
    fn good_packages_do_not_expose_secure_boot_actions_by_leak() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.packages.code = "packages-installed";
        snapshot.secure_boot_key.health = Health::Error;
        snapshot.secure_boot_key.code = "mok-not-enrolled";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Packages),
            Vec::<ActionKey>::new()
        );
    }

    #[test]
    fn libcamera_with_in_tree_module_suggests_force_driver_priority() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.libcamera.health = Health::Warning;
        snapshot.libcamera.code = "libcamera-missing";
        snapshot.module.health = Health::Warning;
        snapshot.module.code = "module-in-tree";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Libcamera),
            vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
        );
    }

    #[test]
    fn libcamera_permission_block_suggests_browser_camera_repair() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.libcamera.health = Health::Error;
        snapshot.libcamera.code = "libcamera-permission-blocked";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Libcamera),
            vec![ActionKey::EnableBrowserCamera]
        );
    }

    #[test]
    fn libcamera_clock_probe_failure_suggests_driver_priority() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.libcamera.health = Health::Error;
        snapshot.libcamera.code = "libcamera-clock-probe-failed";
        snapshot.module.code = "module-patched";
        snapshot.boot.health = Health::Error;
        snapshot.boot.code = "boot-clock-error";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Libcamera),
            vec![ActionKey::ForceDriverPriority, ActionKey::Reboot]
        );
    }

    #[test]
    fn balanced_platform_profile_has_no_suggested_action() {
        let mut snapshot = snapshot_with_healths([Health::Good; 17]);
        snapshot.platform_profile.code = "platform-balanced";
        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::PlatformProfile),
            Vec::<ActionKey>::new()
        );
    }
}
