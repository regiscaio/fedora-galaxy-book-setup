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

    fn snapshot_with_healths(healths: [Health; 12]) -> SetupSnapshot {
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
            gpu: item("Driver NVIDIA", healths[7]),
            platform_profile: item("Perfil de uso", healths[8]),
            clipboard_extension: item("Histórico da área de transferência", healths[9]),
            gsconnect_extension: item("GSConnect", healths[10]),
            desktop_icons_extension: item("Ícones na área de trabalho", healths[11]),
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
            repair_nvidia_command: String::new(),
            set_balanced_profile_command: String::new(),
            reboot_command: String::new(),
            camera_app_installed: false,
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
            Health::Warning,
            Health::Good,
            Health::Good,
            Health::Warning,
            Health::Good,
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
        let snapshot = snapshot_with_healths([
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
        ]);

        assert_eq!(
            suggested_actions(&snapshot, DiagnosticKey::Packages),
            vec![ActionKey::InstallMainSupport, ActionKey::OpenCamera]
        );
    }
}
