use std::cell::RefCell;
use std::rc::Rc;

use libadwaita as adw;
use libadwaita::prelude::*;

use crate::actions::{ActionKey, build_action_row};
use crate::ui::{
    InfoRow, StatusRow, install_css, new_action_button,
};

use super::SetupWindow;
use super::pages::{
    build_actions_page, build_flow_page, build_future_page, build_sections_page,
    build_suggested_page,
};
use super::shell::build_window_shell;

impl SetupWindow {
    pub(crate) fn new(app: &adw::Application) -> Self {
        install_css();

        let shell = build_window_shell(app);
        let window = shell.window;
        let toast_overlay = shell.toast_overlay;
        let navigation_view = shell.navigation_view;
        let refresh_button = shell.refresh_button;

        let system_group = adw::PreferencesGroup::builder()
            .title("Sistema")
            .description("Notebook, Fedora, kernel e Secure Boot.")
            .build();
        let device_row = InfoRow::new("Notebook");
        let fedora_row = InfoRow::new("Fedora");
        let kernel_row = InfoRow::new("Kernel");
        let secure_boot_row = InfoRow::new("Secure Boot");
        system_group.add(&device_row.row);
        system_group.add(&fedora_row.row);
        system_group.add(&kernel_row.row);
        system_group.add(&secure_boot_row.row);

        let diagnostics_group = adw::PreferencesGroup::builder()
            .title("Diagnóstico atual")
            .description("Leitura consolidada do estado do notebook e do próximo passo recomendado.")
            .build();
        let recommendation_title_row = InfoRow::new("Estado");
        let recommendation_body_row = InfoRow::new("Próximo passo");
        diagnostics_group.add(&recommendation_title_row.row);
        diagnostics_group.add(&recommendation_body_row.row);

        let camera_group = adw::PreferencesGroup::builder()
            .title("Câmera")
            .description("Pacotes, driver, akmods, caminho direto do Galaxy Book Câmera, bridge para navegador e erros conhecidos do boot.")
            .build();
        let packages_row = StatusRow::new("Pacotes principais");
        let akmods_row = StatusRow::new("Driver gerado no boot");
        let module_row = StatusRow::new("Módulo ativo");
        let libcamera_row = StatusRow::new("Caminho direto do Galaxy Book Câmera");
        let browser_camera_row = StatusRow::new("Navegador e comunicadores");
        let boot_row = StatusRow::new("Erros no boot");
        camera_group.add(&packages_row.row);
        camera_group.add(&akmods_row.row);
        camera_group.add(&module_row.row);
        camera_group.add(&libcamera_row.row);
        camera_group.add(&browser_camera_row.row);
        camera_group.add(&boot_row.row);

        let audio_group = adw::PreferencesGroup::builder()
            .title("Áudio")
            .description("Validação do caminho MAX98390 usado pelos alto-falantes internos.")
            .build();
        let speakers_row = StatusRow::new("Alto-falantes internos");
        audio_group.add(&speakers_row.row);

        let gpu_group = adw::PreferencesGroup::builder()
            .title("GPU e plataforma")
            .description("Estabilidade do driver NVIDIA e perfil de uso balanceado da plataforma.")
            .build();
        let gpu_row = StatusRow::new("Driver NVIDIA");
        let platform_profile_row = StatusRow::new("Perfil de uso");
        gpu_group.add(&gpu_row.row);
        gpu_group.add(&platform_profile_row.row);

        let integrations_group = adw::PreferencesGroup::builder()
            .title("Integrações do desktop")
            .description("Checklist geral de extensões e integrações que o setup pode acompanhar.")
            .build();
        let clipboard_row = StatusRow::new("Histórico da área de transferência");
        let gsconnect_row = StatusRow::new("GSConnect");
        let desktop_icons_row = StatusRow::new("Ícones na área de trabalho");
        integrations_group.add(&clipboard_row.row);
        integrations_group.add(&gsconnect_row.row);
        integrations_group.add(&desktop_icons_row.row);

        let install_main_button = new_action_button("Instalar suporte principal");
        let install_button = new_action_button("Instalar suporte da câmera");
        let repair_button = new_action_button("Reparar o driver");
        let enable_camera_module_button =
            new_action_button("Habilitar driver da câmera");
        let force_driver_button =
            new_action_button("Ajustar prioridade do driver");
        let restore_camera_button =
            new_action_button("Restaurar stack Intel IPU6");
        let enable_browser_camera_button =
            new_action_button("Ativar câmera para navegador");
        let enable_speakers_button =
            new_action_button("Ativar alto-falantes internos");
        let repair_nvidia_button = new_action_button("Reparar suporte NVIDIA");
        let balanced_profile_button =
            new_action_button("Definir perfil balanceado");
        let reboot_button = new_action_button("Reiniciar o sistema");
        let open_camera_button = new_action_button("Abrir Galaxy Book Câmera");

        let actions_group = adw::PreferencesGroup::builder()
            .title("Ações rápidas")
            .description("Fluxos executáveis diretamente da interface, sem precisar digitar comandos.")
            .build();
        actions_group.add(&build_action_row(
            ActionKey::InstallMainSupport,
            &install_main_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::InstallCamera,
            &install_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::RepairDriver,
            &repair_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::EnableCameraModule,
            &enable_camera_module_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::ForceDriverPriority,
            &force_driver_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::RestoreIntelIpu6,
            &restore_camera_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::EnableBrowserCamera,
            &enable_browser_camera_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::EnableSpeakers,
            &enable_speakers_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::RepairNvidia,
            &repair_nvidia_button,
        ));
        actions_group.add(&build_action_row(
            ActionKey::SetBalancedProfile,
            &balanced_profile_button,
        ));
        actions_group.add(&build_action_row(ActionKey::Reboot, &reboot_button));
        actions_group.add(&build_action_row(
            ActionKey::OpenCamera,
            &open_camera_button,
        ));

        let future_group = adw::PreferencesGroup::builder()
            .title("Módulos futuros")
            .description("Estrutura reservada para outros fluxos do Galaxy Book no Fedora.")
            .build();
        for (title, subtitle) in [
            ("Fingerprint", "Planejado para uma etapa futura."),
            ("Sistema", "Planejado para uma etapa futura."),
        ] {
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(subtitle)
                .build();
            row.set_activatable(false);
            future_group.add(&row);
        }

        let root_page = build_sections_page(&navigation_view, &system_group);
        let flow_page = build_flow_page(
            &diagnostics_group,
            &camera_group,
            &audio_group,
            &gpu_group,
            &integrations_group,
        );
        let actions_page = build_actions_page(&actions_group);
        let suggested_page = build_suggested_page();
        let future_page = build_future_page(&future_group);

        navigation_view.add(&root_page);
        navigation_view.add(&flow_page);
        navigation_view.add(&actions_page);
        navigation_view.add(&suggested_page.page);
        navigation_view.add(&future_page);
        navigation_view.replace_with_tags(&["home"]);

        let snapshot = Rc::new(RefCell::new(None));
        let action_running = Rc::new(RefCell::new(false));
        let selected_diagnostic = Rc::new(RefCell::new(None));
        let suggested_action_rows = Rc::new(RefCell::new(Vec::new()));
        let notification_counts = Rc::new(RefCell::new(None));

        let instance = Self {
            app: app.clone(),
            window,
            navigation_view,
            toast_overlay,
            refresh_button,
            recommendation_title_row,
            recommendation_body_row,
            device_row,
            fedora_row,
            kernel_row,
            secure_boot_row,
            packages_row,
            akmods_row,
            module_row,
            libcamera_row,
            browser_camera_row,
            boot_row,
            speakers_row,
            gpu_row,
            platform_profile_row,
            clipboard_row,
            gsconnect_row,
            desktop_icons_row,
            suggested_title_row: suggested_page.title_row,
            suggested_status_row: suggested_page.status_row,
            suggested_detail_row: suggested_page.detail_row,
            suggested_actions_group: suggested_page.actions_group,
            suggested_action_rows,
            install_main_button,
            install_button,
            repair_button,
            enable_camera_module_button,
            force_driver_button,
            restore_camera_button,
            enable_browser_camera_button,
            enable_speakers_button,
            repair_nvidia_button,
            balanced_profile_button,
            reboot_button,
            open_camera_button,
            snapshot,
            action_running,
            selected_diagnostic,
            notification_counts,
        };

        instance.install_actions(app);
        instance.bind_events();
        instance.bind_diagnostic_navigation();
        instance
    }
}
