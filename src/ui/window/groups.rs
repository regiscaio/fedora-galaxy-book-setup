use libadwaita as adw;
use libadwaita::prelude::*;

use crate::actions::{ActionKey, build_action_row};
use crate::ui::{InfoRow, StatusRow, new_action_button};

pub(super) struct SystemSection {
    pub(super) group: adw::PreferencesGroup,
    pub(super) device_row: InfoRow,
    pub(super) fedora_row: InfoRow,
    pub(super) kernel_row: InfoRow,
    pub(super) secure_boot_row: InfoRow,
}

pub(super) struct DiagnosticsSections {
    pub(super) diagnostics_group: adw::PreferencesGroup,
    pub(super) recommendation_title_row: InfoRow,
    pub(super) recommendation_body_row: InfoRow,
    pub(super) camera_group: adw::PreferencesGroup,
    pub(super) packages_row: StatusRow,
    pub(super) akmods_row: StatusRow,
    pub(super) module_row: StatusRow,
    pub(super) libcamera_row: StatusRow,
    pub(super) browser_camera_row: StatusRow,
    pub(super) boot_row: StatusRow,
    pub(super) audio_group: adw::PreferencesGroup,
    pub(super) speakers_row: StatusRow,
    pub(super) gpu_group: adw::PreferencesGroup,
    pub(super) gpu_row: StatusRow,
    pub(super) platform_profile_row: StatusRow,
    pub(super) integrations_group: adw::PreferencesGroup,
    pub(super) clipboard_row: StatusRow,
    pub(super) gsconnect_row: StatusRow,
    pub(super) desktop_icons_row: StatusRow,
}

pub(super) struct QuickActionsSection {
    pub(super) group: adw::PreferencesGroup,
    pub(super) install_main_button: gtk::Button,
    pub(super) install_button: gtk::Button,
    pub(super) repair_button: gtk::Button,
    pub(super) enable_camera_module_button: gtk::Button,
    pub(super) force_driver_button: gtk::Button,
    pub(super) restore_camera_button: gtk::Button,
    pub(super) enable_browser_camera_button: gtk::Button,
    pub(super) enable_speakers_button: gtk::Button,
    pub(super) repair_nvidia_button: gtk::Button,
    pub(super) balanced_profile_button: gtk::Button,
    pub(super) reboot_button: gtk::Button,
    pub(super) open_camera_button: gtk::Button,
}

pub(super) fn build_system_section() -> SystemSection {
    let group = adw::PreferencesGroup::builder()
        .title("Sistema")
        .description("Notebook, Fedora, kernel e Secure Boot.")
        .build();
    let device_row = InfoRow::new("Notebook");
    let fedora_row = InfoRow::new("Fedora");
    let kernel_row = InfoRow::new("Kernel");
    let secure_boot_row = InfoRow::new("Secure Boot");
    group.add(&device_row.row);
    group.add(&fedora_row.row);
    group.add(&kernel_row.row);
    group.add(&secure_boot_row.row);

    SystemSection {
        group,
        device_row,
        fedora_row,
        kernel_row,
        secure_boot_row,
    }
}

pub(super) fn build_diagnostics_sections() -> DiagnosticsSections {
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

    DiagnosticsSections {
        diagnostics_group,
        recommendation_title_row,
        recommendation_body_row,
        camera_group,
        packages_row,
        akmods_row,
        module_row,
        libcamera_row,
        browser_camera_row,
        boot_row,
        audio_group,
        speakers_row,
        gpu_group,
        gpu_row,
        platform_profile_row,
        integrations_group,
        clipboard_row,
        gsconnect_row,
        desktop_icons_row,
    }
}

pub(super) fn build_quick_actions_section() -> QuickActionsSection {
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
    let balanced_profile_button = new_action_button("Definir perfil balanceado");
    let reboot_button = new_action_button("Reiniciar o sistema");
    let open_camera_button = new_action_button("Abrir Galaxy Book Câmera");

    let group = adw::PreferencesGroup::builder()
        .title("Ações rápidas")
        .description("Fluxos executáveis diretamente da interface, sem precisar digitar comandos.")
        .build();
    group.add(&build_action_row(
        ActionKey::InstallMainSupport,
        &install_main_button,
    ));
    group.add(&build_action_row(ActionKey::InstallCamera, &install_button));
    group.add(&build_action_row(ActionKey::RepairDriver, &repair_button));
    group.add(&build_action_row(
        ActionKey::EnableCameraModule,
        &enable_camera_module_button,
    ));
    group.add(&build_action_row(
        ActionKey::ForceDriverPriority,
        &force_driver_button,
    ));
    group.add(&build_action_row(
        ActionKey::RestoreIntelIpu6,
        &restore_camera_button,
    ));
    group.add(&build_action_row(
        ActionKey::EnableBrowserCamera,
        &enable_browser_camera_button,
    ));
    group.add(&build_action_row(
        ActionKey::EnableSpeakers,
        &enable_speakers_button,
    ));
    group.add(&build_action_row(
        ActionKey::RepairNvidia,
        &repair_nvidia_button,
    ));
    group.add(&build_action_row(
        ActionKey::SetBalancedProfile,
        &balanced_profile_button,
    ));
    group.add(&build_action_row(ActionKey::Reboot, &reboot_button));
    group.add(&build_action_row(ActionKey::OpenCamera, &open_camera_button));

    QuickActionsSection {
        group,
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
    }
}

pub(super) fn build_future_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
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
        group.add(&row);
    }

    group
}
