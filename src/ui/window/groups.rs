use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{tr, tr_mark};

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
    pub(super) sound_app_row: StatusRow,
    pub(super) gpu_group: adw::PreferencesGroup,
    pub(super) gpu_row: StatusRow,
    pub(super) platform_profile_row: StatusRow,
    pub(super) integrations_group: adw::PreferencesGroup,
    pub(super) clipboard_row: StatusRow,
    pub(super) gsconnect_row: StatusRow,
    pub(super) desktop_icons_row: StatusRow,
    pub(super) dock_row: StatusRow,
}

pub(super) struct QuickActionsSection {
    pub(super) group: adw::PreferencesGroup,
    pub(super) install_main_button: gtk::Button,
    pub(super) install_button: gtk::Button,
    pub(super) install_sound_button: gtk::Button,
    pub(super) repair_button: gtk::Button,
    pub(super) enable_camera_module_button: gtk::Button,
    pub(super) force_driver_button: gtk::Button,
    pub(super) restore_camera_button: gtk::Button,
    pub(super) enable_browser_camera_button: gtk::Button,
    pub(super) enable_speakers_button: gtk::Button,
    pub(super) repair_nvidia_button: gtk::Button,
    pub(super) balanced_profile_button: gtk::Button,
    pub(super) clipboard_profile_button: gtk::Button,
    pub(super) gsconnect_profile_button: gtk::Button,
    pub(super) desktop_icons_profile_button: gtk::Button,
    pub(super) dock_profile_button: gtk::Button,
    pub(super) reboot_button: gtk::Button,
    pub(super) open_camera_button: gtk::Button,
    pub(super) open_sound_button: gtk::Button,
}

pub(super) fn build_system_section() -> SystemSection {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Sistema"))
        .description(tr("Notebook, Fedora, kernel e Secure Boot."))
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
        .title(tr("Diagnóstico atual"))
        .description(tr("Leitura consolidada do estado do notebook e do próximo passo recomendado."))
        .build();
    let recommendation_title_row = InfoRow::new("Estado");
    let recommendation_body_row = InfoRow::new("Próximo passo");
    diagnostics_group.add(&recommendation_title_row.row);
    diagnostics_group.add(&recommendation_body_row.row);

    let camera_group = adw::PreferencesGroup::builder()
        .title(tr("Câmera"))
        .description(tr("Pacotes, driver, akmods, caminho direto do Galaxy Book Câmera, bridge para navegador e erros conhecidos do boot."))
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
        .title(tr("Áudio"))
        .description(tr("Validação do caminho MAX98390 e do painel Galaxy Book Sound usado no ajuste fino do áudio."))
        .build();
    let speakers_row = StatusRow::new("Alto-falantes internos");
    let sound_app_row = StatusRow::new("Galaxy Book Sound");
    audio_group.add(&speakers_row.row);
    audio_group.add(&sound_app_row.row);

    let gpu_group = adw::PreferencesGroup::builder()
        .title(tr("GPU e plataforma"))
        .description(tr("Estabilidade do driver NVIDIA e perfil de uso balanceado da plataforma."))
        .build();
    let gpu_row = StatusRow::new("Driver NVIDIA");
    let platform_profile_row = StatusRow::new("Perfil de uso");
    gpu_group.add(&gpu_row.row);
    gpu_group.add(&platform_profile_row.row);

    let integrations_group = adw::PreferencesGroup::builder()
        .title(tr("Integrações do desktop"))
        .description(tr("Checklist geral de extensões e integrações do desktop, incluindo a dock usada neste notebook."))
        .build();
    let clipboard_row = StatusRow::new("Histórico da área de transferência");
    let gsconnect_row = StatusRow::new("GSConnect");
    let desktop_icons_row = StatusRow::new("Ícones na área de trabalho");
    let dock_row = StatusRow::new("Dock do GNOME");
    integrations_group.add(&clipboard_row.row);
    integrations_group.add(&gsconnect_row.row);
    integrations_group.add(&desktop_icons_row.row);
    integrations_group.add(&dock_row.row);

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
        sound_app_row,
        gpu_group,
        gpu_row,
        platform_profile_row,
        integrations_group,
        clipboard_row,
        gsconnect_row,
        desktop_icons_row,
        dock_row,
    }
}

pub(super) fn build_quick_actions_section() -> QuickActionsSection {
    let install_main_button = new_action_button(&tr("Instalar suporte principal"));
    let install_button = new_action_button(&tr("Instalar suporte da câmera"));
    let install_sound_button = new_action_button(&tr("Instalar Galaxy Book Sound"));
    let repair_button = new_action_button(&tr("Reparar o driver"));
    let enable_camera_module_button =
        new_action_button(&tr("Habilitar driver da câmera"));
    let force_driver_button =
        new_action_button(&tr("Ajustar prioridade do driver"));
    let restore_camera_button =
        new_action_button(&tr("Restaurar stack Intel IPU6"));
    let enable_browser_camera_button =
        new_action_button(&tr("Ativar câmera para navegador"));
    let enable_speakers_button =
        new_action_button(&tr("Ativar alto-falantes internos"));
    let repair_nvidia_button = new_action_button(&tr("Reparar suporte NVIDIA"));
    let balanced_profile_button = new_action_button(&tr("Definir perfil balanceado"));
    let clipboard_profile_button =
        new_action_button(&tr("Ativar histórico da área de transferência"));
    let gsconnect_profile_button = new_action_button(&tr("Ativar GSConnect"));
    let desktop_icons_profile_button =
        new_action_button(&tr("Ativar ícones na área de trabalho"));
    let dock_profile_button = new_action_button(&tr("Aplicar perfil da dock"));
    let reboot_button = new_action_button(&tr("Reiniciar o sistema"));
    let open_camera_button = new_action_button(&tr("Abrir Galaxy Book Câmera"));
    let open_sound_button = new_action_button(&tr("Abrir Galaxy Book Sound"));

    let group = adw::PreferencesGroup::builder()
        .title(tr("Ações rápidas"))
        .description(tr("Fluxos executáveis diretamente da interface, sem precisar digitar comandos."))
        .build();
    group.add(&build_action_row(
        ActionKey::InstallMainSupport,
        &install_main_button,
    ));
    group.add(&build_action_row(ActionKey::InstallCamera, &install_button));
    group.add(&build_action_row(
        ActionKey::InstallSoundApp,
        &install_sound_button,
    ));
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
    group.add(&build_action_row(
        ActionKey::ApplyClipboardProfile,
        &clipboard_profile_button,
    ));
    group.add(&build_action_row(
        ActionKey::ApplyGsconnectProfile,
        &gsconnect_profile_button,
    ));
    group.add(&build_action_row(
        ActionKey::ApplyDesktopIconsProfile,
        &desktop_icons_profile_button,
    ));
    group.add(&build_action_row(
        ActionKey::ApplyDockProfile,
        &dock_profile_button,
    ));
    group.add(&build_action_row(ActionKey::Reboot, &reboot_button));
    group.add(&build_action_row(ActionKey::OpenCamera, &open_camera_button));
    group.add(&build_action_row(ActionKey::OpenSoundApp, &open_sound_button));

    QuickActionsSection {
        group,
        install_main_button,
        install_button,
        install_sound_button,
        repair_button,
        enable_camera_module_button,
        force_driver_button,
        restore_camera_button,
        enable_browser_camera_button,
        enable_speakers_button,
        repair_nvidia_button,
        balanced_profile_button,
        clipboard_profile_button,
        gsconnect_profile_button,
        desktop_icons_profile_button,
        dock_profile_button,
        reboot_button,
        open_camera_button,
        open_sound_button,
    }
}

pub(super) fn build_future_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(tr("Módulos futuros"))
        .description(tr("Estrutura reservada para outros fluxos do Galaxy Book no Fedora."))
        .build();
    for (title, subtitle) in [
        (
            tr_mark("Fingerprint"),
            tr_mark("Planejado para uma etapa futura."),
        ),
        (tr_mark("Sistema"), tr_mark("Planejado para uma etapa futura.")),
    ] {
        let row = adw::ActionRow::builder()
            .title(tr(title))
            .subtitle(tr(subtitle))
            .build();
        row.set_activatable(false);
        group.add(&row);
    }

    group
}
