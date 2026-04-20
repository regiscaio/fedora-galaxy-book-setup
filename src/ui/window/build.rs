use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::APP_NAME;

use crate::actions::{ActionKey, build_action_row};
use crate::ui::{
    InfoRow, StatusRow, build_navigation_row, build_scrolled_navigation_page,
    install_css, new_action_button,
};

use super::SetupWindow;

impl SetupWindow {
    pub(crate) fn new(app: &adw::Application) -> Self {
        install_css();

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(980)
            .default_height(760)
            .title(APP_NAME)
            .build();

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_hexpand(true);
        toast_overlay.set_vexpand(true);

        let header_title = adw::WindowTitle::new(APP_NAME, "");
        let back_button = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Voltar")
            .visible(false)
            .build();
        back_button.add_css_class("flat");

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&header_title));
        header.pack_start(&back_button);

        let refresh_button = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Atualizar diagnóstico")
            .build();
        header.pack_end(&refresh_button);

        let menu = gio::Menu::new();
        menu.append(Some("Sobre"), Some("app.about"));
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .build();
        header.pack_end(&menu_button);

        let navigation_view = adw::NavigationView::new();
        navigation_view.set_animate_transitions(true);
        navigation_view.set_pop_on_escape(true);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&navigation_view));
        toast_overlay.set_child(Some(&toolbar));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&toast_overlay);
        window.set_content(Some(&root));

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

        let sections_page = adw::PreferencesPage::builder()
            .name("sections")
            .title(APP_NAME)
            .build();
        sections_page.add(&system_group);
        let sections_group = adw::PreferencesGroup::builder()
            .title("Áreas do assistente")
            .description("Acesse as áreas operacionais do auxiliar de instalação e diagnóstico.")
            .build();
        sections_group.add(&build_navigation_row(
            "Diagnósticos",
            "Checklist geral da câmera, do áudio, da GPU e das integrações do desktop.",
            {
                let navigation_view = navigation_view.clone();
                move || navigation_view.push_by_tag("flow")
            },
        ));
        sections_group.add(&build_navigation_row(
            "Ações rápidas",
            "Execute instalação, reparo e reinício direto da interface.",
            {
                let navigation_view = navigation_view.clone();
                move || navigation_view.push_by_tag("actions")
            },
        ));
        sections_group.add(&build_navigation_row(
            "Módulos futuros",
            "Fingerprint e outras frentes planejadas.",
            {
                let navigation_view = navigation_view.clone();
                move || navigation_view.push_by_tag("future")
            },
        ));
        sections_page.add(&sections_group);

        let root_page =
            build_scrolled_navigation_page(&sections_page, APP_NAME, "home");

        let flow_page_content = adw::PreferencesPage::builder()
            .name("flow")
            .title("Diagnósticos")
            .build();
        flow_page_content.add(&diagnostics_group);
        flow_page_content.add(&camera_group);
        flow_page_content.add(&audio_group);
        flow_page_content.add(&gpu_group);
        flow_page_content.add(&integrations_group);
        let flow_page =
            build_scrolled_navigation_page(&flow_page_content, "Diagnósticos", "flow");

        let actions_page_content = adw::PreferencesPage::builder()
            .name("actions")
            .title("Ações rápidas")
            .build();
        actions_page_content.add(&actions_group);
        let actions_page = build_scrolled_navigation_page(
            &actions_page_content,
            "Ações rápidas",
            "actions",
        );

        let suggested_summary_group = adw::PreferencesGroup::builder()
            .title("Diagnóstico selecionado")
            .description("Leitura do item selecionado e ações rápidas relacionadas ao problema ou à validação atual.")
            .build();
        let suggested_title_row = InfoRow::new("Item");
        let suggested_status_row = InfoRow::new("Status");
        let suggested_detail_row = InfoRow::new("Leitura");
        suggested_summary_group.add(&suggested_title_row.row);
        suggested_summary_group.add(&suggested_status_row.row);
        suggested_summary_group.add(&suggested_detail_row.row);

        let suggested_actions_group = adw::PreferencesGroup::builder()
            .title("Ações sugeridas")
            .description("Ações rápidas filtradas para o diagnóstico selecionado.")
            .build();

        let suggested_page_content = adw::PreferencesPage::builder()
            .name("suggested-actions")
            .title("Ações sugeridas")
            .build();
        suggested_page_content.add(&suggested_summary_group);
        suggested_page_content.add(&suggested_actions_group);
        let suggested_page = build_scrolled_navigation_page(
            &suggested_page_content,
            "Ações sugeridas",
            "suggested-actions",
        );

        let future_page_content = adw::PreferencesPage::builder()
            .name("future")
            .title("Módulos futuros")
            .build();
        future_page_content.add(&future_group);
        let future_page = build_scrolled_navigation_page(
            &future_page_content,
            "Módulos futuros",
            "future",
        );

        navigation_view.add(&root_page);
        navigation_view.add(&flow_page);
        navigation_view.add(&actions_page);
        navigation_view.add(&suggested_page);
        navigation_view.add(&future_page);
        navigation_view.replace_with_tags(&["home"]);

        back_button.connect_clicked({
            let navigation_view = navigation_view.clone();
            move |_| {
                navigation_view.pop();
            }
        });

        navigation_view.connect_visible_page_notify({
            let header_title = header_title.clone();
            let back_button = back_button.clone();
            move |navigation_view| {
                let Some(page) = navigation_view.visible_page() else {
                    header_title.set_title(APP_NAME);
                    back_button.set_visible(false);
                    return;
                };

                header_title.set_title(page.title().as_str());
                back_button
                    .set_visible(navigation_view.previous_page(&page).is_some());
            }
        });

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
            suggested_title_row,
            suggested_status_row,
            suggested_detail_row,
            suggested_actions_group,
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
