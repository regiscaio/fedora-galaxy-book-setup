mod actions;
mod diagnostics;
mod ui;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{
    APP_ID, APP_NAME, CheckItem, Health, SetupSnapshot, collect_snapshot,
    run_smoke_test,
};
use diagnostics::{
    DiagnosticAlertCounts, dedupe_action_keys, diagnostic_alert_counts,
    diagnostic_item, diagnostic_notification_body, diagnostic_notification_title,
    suggested_actions,
};
use ui::{
    apply_status_class, build_about_details_subpage, build_about_summary_row,
    build_button_row, build_navigation_row, build_scrolled_navigation_page,
    build_suffix_action_row, install_css, new_action_button,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticKey {
    Packages,
    Akmods,
    Module,
    Libcamera,
    BrowserCamera,
    Boot,
    Speakers,
    Gpu,
    PlatformProfile,
    Clipboard,
    Gsconnect,
    DesktopIcons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionKey {
    InstallMainSupport,
    InstallCamera,
    RepairDriver,
    EnableCameraModule,
    ForceDriverPriority,
    RestoreIntelIpu6,
    EnableBrowserCamera,
    EnableSpeakers,
    RepairNvidia,
    SetBalancedProfile,
    Reboot,
    OpenCamera,
}

#[derive(Clone)]
struct StatusRow {
    row: adw::ActionRow,
    icon: gtk::Image,
    badge: gtk::Label,
    next_button: gtk::Button,
}

impl StatusRow {
    fn new(title: &'static str) -> Self {
        let row = adw::ActionRow::builder().title(title).build();
        row.set_subtitle("Aguardando diagnóstico");

        let icon = gtk::Image::from_icon_name("dialog-question-symbolic");
        icon.set_valign(gtk::Align::Center);
        icon.add_css_class("status-icon");
        icon.add_css_class("status-pill-unknown");
        row.add_prefix(&icon);

        let badge = gtk::Label::new(Some(Health::Unknown.label()));
        badge.set_valign(gtk::Align::Center);
        badge.add_css_class("status-pill");
        badge.add_css_class("status-pill-unknown");
        row.add_suffix(&badge);

        let next_button = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Ver ações sugeridas")
            .valign(gtk::Align::Center)
            .build();
        next_button.add_css_class("flat");
        row.add_suffix(&next_button);

        Self {
            row,
            icon,
            badge,
            next_button,
        }
    }

    fn apply(&self, item: &CheckItem) {
        self.row.set_subtitle(&item.detail);
        self.icon.set_icon_name(Some(item.health.icon_name()));
        apply_status_class(&self.icon, item.health);
        self.badge.set_label(item.health.label());
        apply_status_class(&self.badge, item.health);
    }

    fn connect_suggested_actions<F>(&self, on_activate: F)
    where
        F: Fn() + 'static,
    {
        let callback = Rc::new(on_activate);
        {
            let callback = callback.clone();
            self.next_button.connect_clicked(move |_| {
                callback();
            });
        }
        {
            let callback = callback.clone();
            self.row.connect_activated(move |_| {
                callback();
            });
        }
        self.row.set_activatable(true);
        self.row.set_activatable_widget(Some(&self.next_button));
    }
}

#[derive(Clone)]
struct InfoRow {
    row: adw::ActionRow,
}

impl InfoRow {
    fn new(title: &'static str) -> Self {
        let row = adw::ActionRow::builder().title(title).build();
        row.set_subtitle("Coletando…");
        Self { row }
    }

    fn set_subtitle(&self, subtitle: &str) {
        self.row.set_subtitle(subtitle);
    }
}

#[derive(Clone)]
struct CommandResult {
    title: String,
    success_message: String,
    output: String,
    success: bool,
    refresh_after: bool,
}

#[derive(Clone)]
struct SetupWindow {
    app: adw::Application,
    window: adw::ApplicationWindow,
    navigation_view: adw::NavigationView,
    toast_overlay: adw::ToastOverlay,
    refresh_button: gtk::Button,
    recommendation_title_row: InfoRow,
    recommendation_body_row: InfoRow,
    device_row: InfoRow,
    fedora_row: InfoRow,
    kernel_row: InfoRow,
    secure_boot_row: InfoRow,
    packages_row: StatusRow,
    akmods_row: StatusRow,
    module_row: StatusRow,
    libcamera_row: StatusRow,
    browser_camera_row: StatusRow,
    boot_row: StatusRow,
    speakers_row: StatusRow,
    gpu_row: StatusRow,
    platform_profile_row: StatusRow,
    clipboard_row: StatusRow,
    gsconnect_row: StatusRow,
    desktop_icons_row: StatusRow,
    suggested_title_row: InfoRow,
    suggested_status_row: InfoRow,
    suggested_detail_row: InfoRow,
    suggested_actions_group: adw::PreferencesGroup,
    suggested_action_rows: Rc<RefCell<Vec<gtk::Widget>>>,
    install_main_button: gtk::Button,
    install_button: gtk::Button,
    repair_button: gtk::Button,
    enable_camera_module_button: gtk::Button,
    force_driver_button: gtk::Button,
    restore_camera_button: gtk::Button,
    enable_browser_camera_button: gtk::Button,
    enable_speakers_button: gtk::Button,
    repair_nvidia_button: gtk::Button,
    balanced_profile_button: gtk::Button,
    reboot_button: gtk::Button,
    open_camera_button: gtk::Button,
    snapshot: Rc<RefCell<Option<SetupSnapshot>>>,
    action_running: Rc<RefCell<bool>>,
    selected_diagnostic: Rc<RefCell<Option<DiagnosticKey>>>,
    notification_counts: Rc<RefCell<Option<DiagnosticAlertCounts>>>,
}

impl SetupWindow {
    fn new(app: &adw::Application) -> Self {
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
        actions_group.add(&build_button_row(
            "Instalar suporte principal",
            "Instala o conjunto principal do notebook a partir do próprio setup: Galaxy Book Câmera, driver OV02C10 e suporte MAX98390 dos alto-falantes internos.",
            &install_main_button,
        ));
        actions_group.add(&build_button_row(
            "Instalar suporte da câmera",
            "Instala o driver corrigido e o aplicativo Galaxy Book Câmera usando privilégios administrativos.",
            &install_button,
        ));
        actions_group.add(&build_button_row(
            "Reparar o driver",
            "Reconstrói o módulo com akmods para o kernel atual e atualiza a árvore de módulos.",
            &repair_button,
        ));
        actions_group.add(&build_button_row(
            "Habilitar driver da câmera",
            "Garante o carregamento do ov02c10 no boot, ajusta o softdep do IPU6 e carrega o módulo agora no kernel.",
            &enable_camera_module_button,
        ));
        actions_group.add(&build_button_row(
            "Ajustar prioridade do driver",
            "Compila o módulo corrigido, assina quando o Secure Boot estiver ativo e o instala em /lib/modules/.../updates sem compressão incompatível.",
            &force_driver_button,
        ));
        actions_group.add(&build_button_row(
            "Restaurar stack Intel IPU6",
            "Remove o override manual em /updates, reinstala o stack Intel empacotado e volta ao caminho que já funcionava com a câmera do sistema.",
            &restore_camera_button,
        ));
        actions_group.add(&build_button_row(
            "Ativar câmera para navegador",
            "Expõe a câmera interna como webcam V4L2 para Meet, Discord, Teams e outros apps WebRTC, usando icamerasrc, v4l2-relayd e v4l2loopback, além de ocultar os nós crus do IPU6.",
            &enable_browser_camera_button,
        ));
        actions_group.add(&build_button_row(
            "Ativar alto-falantes internos",
            "Instala o suporte MAX98390, reconstrói os módulos, instala manualmente o driver no kernel atual quando necessário e habilita o serviço de I2C usado pelos alto-falantes internos.",
            &enable_speakers_button,
        ));
        actions_group.add(&build_button_row(
            "Reparar suporte NVIDIA",
            "Instala ou reconstrói o akmod-nvidia para o kernel atual. O nvidia-smi permanece opcional.",
            &repair_nvidia_button,
        ));
        actions_group.add(&build_button_row(
            "Definir perfil balanceado",
            "Aplica o perfil balanced da plataforma para uso geral, equilibrando ventoinha, temperatura e desempenho.",
            &balanced_profile_button,
        ));
        actions_group.add(&build_button_row(
            "Reiniciar o sistema",
            "Aplica mudanças do driver e reinicia a sessão inteira do notebook.",
            &reboot_button,
        ));
        actions_group.add(&build_button_row(
            "Abrir Galaxy Book Câmera",
            "Abre o aplicativo final da câmera quando ele estiver instalado no sistema.",
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
            let row = adw::ActionRow::builder().title(title).subtitle(subtitle).build();
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

        let root_page = build_scrolled_navigation_page(&sections_page, APP_NAME, "home");

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
        let actions_page =
            build_scrolled_navigation_page(&actions_page_content, "Ações rápidas", "actions");

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
        let future_page =
            build_scrolled_navigation_page(&future_page_content, "Módulos futuros", "future");

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
                back_button.set_visible(navigation_view.previous_page(&page).is_some());
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

    fn present(&self) {
        self.window.present();
        self.refresh();
    }

    fn install_actions(&self, app: &adw::Application) {
        let action = gio::SimpleAction::new("about", None);
        let this = self.clone();
        action.connect_activate(move |_, _| {
            this.present_about_dialog();
        });
        app.add_action(&action);
    }

    fn present_about_dialog(&self) {
        let dialog = adw::Dialog::builder()
            .title("Sobre")
            .content_width(520)
            .content_height(620)
            .build();
        let navigation_view = adw::NavigationView::new();
        navigation_view.set_animate_transitions(true);
        navigation_view.set_pop_on_escape(true);

        let header_title = adw::WindowTitle::new("Sobre", "");
        let back_button = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Voltar")
            .visible(false)
            .build();
        back_button.add_css_class("flat");

        let header_bar = adw::HeaderBar::new();
        header_bar.set_title_widget(Some(&header_title));
        header_bar.pack_start(&back_button);

        let summary_group = adw::PreferencesGroup::new();
        summary_group.add(&build_about_summary_row(
            APP_NAME,
            "Auxiliar de instalação e diagnóstico para Galaxy Book no Fedora.",
        ));

        let author_row = adw::ActionRow::builder()
            .title("Caio Régis")
            .subtitle("@regiscaio")
            .build();
        author_row.set_activatable(false);
        summary_group.add(&author_row);

        let links_group = adw::PreferencesGroup::builder().title("Projeto").build();
        links_group.add(&self.build_uri_row("Página da web", "https://caioregis.com"));
        links_group.add(&self.build_uri_row(
            "Repositório do projeto",
            "https://github.com/regiscaio/fedora-galaxy-book-setup",
        ));
        links_group.add(&self.build_uri_row(
            "Relatar problema",
            "https://github.com/regiscaio/fedora-galaxy-book-setup/issues",
        ));
        links_group.add(&build_suffix_action_row(
            "Detalhes",
            "Versão, identificação do app e escopo atual do assistente.",
            "go-next-symbolic",
            "Abrir detalhes",
            {
                let navigation_view = navigation_view.clone();
                move || {
                    navigation_view.push_by_tag("details");
                }
            },
        ));

        let about_page_content = adw::PreferencesPage::builder()
            .name("about")
            .title("Sobre")
            .build();
        about_page_content.add(&summary_group);
        about_page_content.add(&links_group);

        let about_page = build_scrolled_navigation_page(&about_page_content, "Sobre", "about");
        let details_page = build_about_details_subpage();

        navigation_view.add(&about_page);
        navigation_view.add(&details_page);
        navigation_view.replace_with_tags(&["about"]);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&navigation_view));
        dialog.set_child(Some(&toolbar_view));

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
                    header_title.set_title("Sobre");
                    back_button.set_visible(false);
                    return;
                };

                header_title.set_title(page.title().as_str());
                back_button.set_visible(navigation_view.previous_page(&page).is_some());
            }
        });

        dialog.present(Some(&self.window));
    }

    fn build_uri_row(&self, title: &str, uri: &'static str) -> adw::ActionRow {
        let window = self.window.clone();
        let toast_overlay = self.toast_overlay.clone();
        build_suffix_action_row(title, uri, "send-to-symbolic", "Abrir link", move || {
            let launcher = gtk::UriLauncher::new(uri);
            let toast_overlay = toast_overlay.clone();
            launcher.launch(
                Some(&window),
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    if let Err(error) = result {
                        toast_overlay.add_toast(adw::Toast::new(&format!(
                            "Falha ao abrir o link: {error}"
                        )));
                    }
                },
            );
        })
    }

    fn bind_diagnostic_navigation(&self) {
        self.connect_diagnostic_row(&self.packages_row, DiagnosticKey::Packages);
        self.connect_diagnostic_row(&self.akmods_row, DiagnosticKey::Akmods);
        self.connect_diagnostic_row(&self.module_row, DiagnosticKey::Module);
        self.connect_diagnostic_row(&self.libcamera_row, DiagnosticKey::Libcamera);
        self.connect_diagnostic_row(&self.browser_camera_row, DiagnosticKey::BrowserCamera);
        self.connect_diagnostic_row(&self.boot_row, DiagnosticKey::Boot);
        self.connect_diagnostic_row(&self.speakers_row, DiagnosticKey::Speakers);
        self.connect_diagnostic_row(&self.gpu_row, DiagnosticKey::Gpu);
        self.connect_diagnostic_row(&self.platform_profile_row, DiagnosticKey::PlatformProfile);
        self.connect_diagnostic_row(&self.clipboard_row, DiagnosticKey::Clipboard);
        self.connect_diagnostic_row(&self.gsconnect_row, DiagnosticKey::Gsconnect);
        self.connect_diagnostic_row(&self.desktop_icons_row, DiagnosticKey::DesktopIcons);
    }

    fn connect_diagnostic_row(&self, row: &StatusRow, key: DiagnosticKey) {
        let this = self.clone();
        row.connect_suggested_actions(move || {
            this.present_suggested_actions(key);
        });
    }

    fn refresh(&self) {
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.recommendation_title_row
            .set_subtitle("Atualizando diagnóstico…");
        self.recommendation_body_row.set_subtitle(
            "Aguarde enquanto o setup verifica pacotes, driver, akmods, câmera, GPU, plataforma e integrações do desktop.",
        );

        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let snapshot = collect_snapshot();
            let _ = sender.send(snapshot);
        });

        let this = self.clone();
        glib::timeout_add_local(Duration::from_millis(75), move || match receiver.try_recv() {
            Ok(snapshot) => {
                this.apply_snapshot(snapshot);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                this.refresh_button.set_sensitive(true);
                this.set_action_buttons_sensitive(true);
                this.toast_overlay
                    .add_toast(adw::Toast::new("Falha ao atualizar o diagnóstico."));
                glib::ControlFlow::Break
            }
        });
    }

    fn apply_snapshot(&self, snapshot: SetupSnapshot) {
        self.device_row.set_subtitle(&snapshot.system.notebook);
        self.fedora_row.set_subtitle(&snapshot.system.fedora);
        self.kernel_row.set_subtitle(&snapshot.system.kernel);
        self.secure_boot_row.set_subtitle(&snapshot.system.secure_boot);

        self.packages_row.apply(&snapshot.packages);
        self.akmods_row.apply(&snapshot.akmods);
        self.module_row.apply(&snapshot.module);
        self.libcamera_row.apply(&snapshot.libcamera);
        self.browser_camera_row.apply(&snapshot.browser_camera);
        self.boot_row.apply(&snapshot.boot);
        self.speakers_row.apply(&snapshot.speakers);
        self.gpu_row.apply(&snapshot.gpu);
        self.platform_profile_row.apply(&snapshot.platform_profile);
        self.clipboard_row.apply(&snapshot.clipboard_extension);
        self.gsconnect_row.apply(&snapshot.gsconnect_extension);
        self.desktop_icons_row
            .apply(&snapshot.desktop_icons_extension);

        self.recommendation_title_row
            .set_subtitle(&snapshot.recommendation_title);
        self.recommendation_body_row
            .set_subtitle(&snapshot.recommendation_body);

        self.update_diagnostic_notifications(&snapshot);

        if let Some(key) = *self.selected_diagnostic.borrow() {
            self.apply_suggested_actions(&snapshot, key);
        }

        self.open_camera_button
            .set_sensitive(snapshot.camera_app_installed && !*self.action_running.borrow());
        self.refresh_button.set_sensitive(!*self.action_running.borrow());
        self.set_action_buttons_sensitive(!*self.action_running.borrow());

        *self.snapshot.borrow_mut() = Some(snapshot);
    }

    fn update_diagnostic_notifications(&self, snapshot: &SetupSnapshot) {
        let counts = diagnostic_alert_counts(snapshot);
        let previous = self.notification_counts.replace(Some(counts));

        self.update_launcher_badge(counts);

        if counts.is_clear() {
            self.app.withdraw_notification("diagnostics-summary");
            return;
        }

        if previous.is_none() || previous == Some(counts) {
            return;
        }

        let notification =
            gio::Notification::new(&diagnostic_notification_title(counts));
        notification.set_body(Some(&diagnostic_notification_body(snapshot, counts)));
        notification.set_priority(if counts.errors > 0 {
            gio::NotificationPriority::High
        } else {
            gio::NotificationPriority::Normal
        });
        notification.set_icon(&gio::ThemedIcon::new(APP_ID));

        self.app
            .send_notification(Some("diagnostics-summary"), &notification);
    }

    fn update_launcher_badge(&self, counts: DiagnosticAlertCounts) {
        let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        else {
            return;
        };

        let mut properties = HashMap::new();
        properties.insert("count".to_string(), counts.total().to_variant());
        properties.insert("count-visible".to_string(), (!counts.is_clear()).to_variant());
        properties.insert("urgent".to_string(), (counts.errors > 0).to_variant());

        let parameters = (
            format!("application://{APP_ID}.desktop"),
            properties,
        )
            .to_variant();

        let _ = connection.emit_signal(
            None::<&str>,
            "/com/canonical/Unity/LauncherEntry",
            "com.canonical.Unity.LauncherEntry",
            "Update",
            Some(&parameters),
        );
    }

    fn present_suggested_actions(&self, key: DiagnosticKey) {
        *self.selected_diagnostic.borrow_mut() = Some(key);
        if let Some(snapshot) = self.snapshot.borrow().as_ref().cloned() {
            self.apply_suggested_actions(&snapshot, key);
        } else {
            self.suggested_title_row.set_subtitle("Diagnóstico indisponível");
            self.suggested_status_row.set_subtitle("Aguardando leitura");
            self.suggested_detail_row
                .set_subtitle("Atualize o diagnóstico antes de abrir as ações sugeridas.");
            self.reset_suggested_actions(&[]);
        }
        self.navigation_view.push_by_tag("suggested-actions");
    }

    fn apply_suggested_actions(&self, snapshot: &SetupSnapshot, key: DiagnosticKey) {
        let item = diagnostic_item(snapshot, key);
        self.suggested_title_row.set_subtitle(item.title);
        self.suggested_status_row.set_subtitle(item.health.label());
        self.suggested_detail_row.set_subtitle(&item.detail);
        let actions = suggested_actions(snapshot, key);
        self.reset_suggested_actions(&actions);
    }

    fn reset_suggested_actions(&self, actions: &[ActionKey]) {
        {
            let mut rows = self.suggested_action_rows.borrow_mut();
            for widget in rows.drain(..) {
                self.suggested_actions_group.remove(&widget);
            }
        }

        let deduped_actions = dedupe_action_keys(actions);

        if deduped_actions.is_empty() {
            let row = adw::ActionRow::builder()
                .title("Sem automação disponível")
                .subtitle("Este diagnóstico ainda não tem uma ação rápida dedicada no setup. O painel geral de ações continua disponível sem filtro.")
                .build();
            row.set_activatable(false);
            self.suggested_actions_group.add(&row);
            self.suggested_action_rows.borrow_mut().push(row.clone().upcast());
            return;
        }

        for action in deduped_actions {
            let row = self.build_suggested_action_row(action);
            self.suggested_actions_group.add(&row);
            self.suggested_action_rows.borrow_mut().push(row.clone().upcast());
        }
    }

}

fn main() -> glib::ExitCode {
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
    use crate::diagnostics::diagnostic_counts_summary;
    use super::{
        ActionKey, DiagnosticAlertCounts, DiagnosticKey, diagnostic_alert_counts,
        suggested_actions,
    };
    use galaxybook_setup::{
        CheckItem, Health, SetupSnapshot, SystemSummary,
    };

    fn item(title: &'static str, health: Health) -> CheckItem {
        CheckItem {
            title,
            detail: String::new(),
            health,
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
