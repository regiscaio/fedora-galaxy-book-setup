use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{APP_NAME, tr, trf};

use crate::ui::{
    build_about_details_subpage, build_about_summary_row,
    build_scrolled_navigation_page, build_suffix_action_row,
};
use crate::ui::SetupWindow;

impl SetupWindow {
    pub(crate) fn present_about_dialog(&self) {
        let dialog = adw::Dialog::builder()
            .title(tr("Sobre"))
            .content_width(520)
            .content_height(620)
            .build();
        let navigation_view = adw::NavigationView::new();
        navigation_view.set_animate_transitions(true);
        navigation_view.set_pop_on_escape(true);

        let header_title = adw::WindowTitle::new(&tr("Sobre"), "");
        let back_button = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text(tr("Voltar"))
            .visible(false)
            .build();
        back_button.add_css_class("flat");

        let header_bar = adw::HeaderBar::new();
        header_bar.set_title_widget(Some(&header_title));
        header_bar.pack_start(&back_button);

        let summary_group = adw::PreferencesGroup::new();
        summary_group.add(&build_about_summary_row(
            APP_NAME,
            &tr("Auxiliar de instalação e diagnóstico para Galaxy Book no Fedora."),
        ));

        let author_row = adw::ActionRow::builder()
            .title("Caio Régis")
            .subtitle("@regiscaio")
            .build();
        author_row.set_activatable(false);
        summary_group.add(&author_row);

        let links_group = adw::PreferencesGroup::builder().title(tr("Projeto")).build();
        links_group.add(&self.build_uri_row(&tr("Página da web"), "https://caioregis.com"));
        links_group.add(&self.build_uri_row(
            &tr("Repositório do projeto"),
            "https://github.com/regiscaio/fedora-galaxy-book-setup",
        ));
        links_group.add(&self.build_uri_row(
            &tr("Relatar problema"),
            "https://github.com/regiscaio/fedora-galaxy-book-setup/issues",
        ));
        links_group.add(&build_suffix_action_row(
            &tr("Detalhes"),
            &tr("Versão, identificação do app e escopo atual do assistente, incluindo o fluxo do Galaxy Book Sound."),
            "go-next-symbolic",
            &tr("Abrir detalhes"),
            {
                let navigation_view = navigation_view.clone();
                move || {
                    navigation_view.push_by_tag("details");
                }
            },
        ));

        let about_page_content = adw::PreferencesPage::builder()
            .name("about")
            .title(tr("Sobre"))
            .build();
        about_page_content.add(&summary_group);
        about_page_content.add(&links_group);

        let about_page =
            build_scrolled_navigation_page(&about_page_content, &tr("Sobre"), "about");
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
                    header_title.set_title(&tr("Sobre"));
                    back_button.set_visible(false);
                    return;
                };

                header_title.set_title(page.title().as_str());
                back_button
                    .set_visible(navigation_view.previous_page(&page).is_some());
            }
        });

        dialog.present(Some(&self.window));
    }

    fn build_uri_row(&self, title: &str, uri: &'static str) -> adw::ActionRow {
        let window = self.window.clone();
        let toast_overlay = self.toast_overlay.clone();
        let tooltip = tr("Abrir link");
        build_suffix_action_row(
            title,
            uri,
            "adw-external-link-symbolic",
            &tooltip,
            move || {
                let launcher = gtk::UriLauncher::new(uri);
                let toast_overlay = toast_overlay.clone();
                launcher.launch(
                    Some(&window),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let Err(error) = result {
                            toast_overlay.add_toast(adw::Toast::new(&trf(
                                "Falha ao abrir o link: {error}",
                                &[("error", error.to_string())],
                            )));
                        }
                    },
                );
            },
        )
    }
}
