use std::rc::Rc;

use galaxybook_setup::{APP_ID, APP_NAME, Health};
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

pub(crate) fn build_scrolled_navigation_page(
    page: &adw::PreferencesPage,
    title: &str,
    tag: &str,
) -> adw::NavigationPage {
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(0)
        .child(page)
        .build();

    adw::NavigationPage::builder()
        .title(title)
        .tag(tag)
        .child(&scroller)
        .can_pop(true)
        .build()
}

pub(crate) fn build_button_row(title: &str, subtitle: &str, button: &gtk::Button) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).subtitle(subtitle).build();
    let suffix_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    suffix_box.set_halign(gtk::Align::End);
    suffix_box.set_valign(gtk::Align::Center);
    suffix_box.set_vexpand(false);
    suffix_box.set_height_request(40);
    suffix_box.append(button);

    let top_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top_spacer.set_vexpand(true);
    let bottom_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bottom_spacer.set_vexpand(true);

    let suffix_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    suffix_column.set_halign(gtk::Align::End);
    suffix_column.set_hexpand(false);
    suffix_column.set_vexpand(true);
    suffix_column.append(&top_spacer);
    suffix_column.append(&suffix_box);
    suffix_column.append(&bottom_spacer);

    row.add_suffix(&suffix_column);
    row.set_activatable_widget(Some(button));
    row
}

pub(crate) fn new_action_button(tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::End)
        .build();
    button.add_css_class("flat");
    button.add_css_class("quick-action-button");
    button.set_width_request(40);
    button.set_height_request(40);
    button.set_vexpand(false);
    button
}

pub(crate) fn build_suffix_action_row<F>(
    title: &str,
    subtitle: &str,
    icon_name: &str,
    tooltip: &str,
    on_activate: F,
) -> adw::ActionRow
where
    F: Fn() + 'static,
{
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.set_subtitle_selectable(true);

    let button = gtk::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("flat");

    let callback = Rc::new(on_activate);
    {
        let callback = callback.clone();
        button.connect_clicked(move |_| {
            callback();
        });
    }

    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    row.set_activatable(true);
    row
}

pub(crate) fn build_navigation_row<F>(title: &str, subtitle: &str, on_activate: F) -> adw::ActionRow
where
    F: Fn() + 'static,
{
    build_suffix_action_row(title, subtitle, "go-next-symbolic", "Abrir seção", on_activate)
}

pub(crate) fn build_about_summary_row(app_name: &str, description: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let app_icon = gtk::Image::from_icon_name(APP_ID);
    app_icon.set_pixel_size(48);
    app_icon.set_valign(gtk::Align::Start);

    let text_column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text_column.set_hexpand(true);
    text_column.set_valign(gtk::Align::Center);

    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    title_row.set_halign(gtk::Align::Start);

    let title_label = gtk::Label::new(None);
    title_label.set_markup(&format!(
        "<span size='large' weight='600'>{}</span>",
        glib::markup_escape_text(app_name)
    ));
    title_label.set_xalign(0.0);

    let version_label = gtk::Label::new(None);
    version_label.set_markup(&format!(
        "<span alpha='55%' size='small'>Versão {}</span>",
        glib::markup_escape_text(env!("CARGO_PKG_VERSION"))
    ));
    version_label.set_xalign(0.0);

    let description_label = gtk::Label::new(None);
    description_label.set_markup(&format!(
        "<span alpha='55%' size='small'>{}</span>",
        glib::markup_escape_text(description)
    ));
    description_label.set_xalign(0.0);
    description_label.set_wrap(true);

    title_row.append(&title_label);
    title_row.append(&version_label);
    text_column.append(&title_row);
    text_column.append(&description_label);

    content.append(&app_icon);
    content.append(&text_column);
    row.set_child(Some(&content));
    row
}

pub(crate) fn build_about_details_subpage() -> adw::NavigationPage {
    let page = adw::PreferencesPage::builder()
        .name("details")
        .title("Detalhes")
        .build();

    let app_group = adw::PreferencesGroup::builder()
        .title("Aplicativo")
        .description("Identificação pública e técnica do Galaxy Book Setup.")
        .build();

    for (title, subtitle) in [
        ("Nome", APP_NAME.to_string()),
        ("Versão", env!("CARGO_PKG_VERSION").to_string()),
        ("App ID", APP_ID.to_string()),
        ("Desktop ID", format!("{APP_ID}.desktop")),
    ] {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .build();
        row.set_activatable(false);
        row.set_subtitle_selectable(true);
        app_group.add(&row);
    }

    let scope_group = adw::PreferencesGroup::builder()
        .title("Escopo atual")
        .description("Resumo do que esta primeira entrega do assistente cobre hoje.")
        .build();
    for (title, subtitle) in [
        (
            "Objetivo",
            "Auxiliar de instalação e diagnóstico para notebooks Galaxy Book no Fedora."
                .to_string(),
        ),
        (
            "Módulo disponível",
            "Fluxos de instalação, reparo e checklist da câmera interna, bridge V4L2 para navegador, suporte inicial aos alto-falantes MAX98390, estabilidade básica da NVIDIA, perfil de uso balanceado e integrações do desktop."
                .to_string(),
        ),
        (
            "Próximos módulos",
            "Fingerprint e outros fluxos de integração do notebook."
                .to_string(),
        ),
    ] {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .build();
        row.set_activatable(false);
        row.set_subtitle_selectable(true);
        scope_group.add(&row);
    }

    page.add(&app_group);
    page.add(&scope_group);

    build_scrolled_navigation_page(&page, "Detalhes", "details")
}

pub(crate) fn apply_status_class(widget: &impl IsA<gtk::Widget>, health: Health) {
    let widget = widget.as_ref();
    for class_name in [
        "status-pill-good",
        "status-pill-warning",
        "status-pill-error",
        "status-pill-unknown",
    ] {
        widget.remove_css_class(class_name);
    }
    widget.add_css_class(health.css_class());
}

pub(crate) fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        .status-pill {
            border-radius: 9999px;
            padding: 4px 10px;
            font-size: 0.85rem;
            font-weight: 700;
        }

        .status-icon {
            -gtk-icon-size: 18px;
            border-radius: 9999px;
            padding: 6px;
        }

        .status-pill-good {
            color: #78e08f;
            background-color: rgba(120, 224, 143, 0.14);
        }

        .status-pill-warning {
            color: #ffd166;
            background-color: rgba(255, 209, 102, 0.14);
        }

        .status-pill-error {
            color: #ff8fa3;
            background-color: rgba(255, 143, 163, 0.14);
        }

        .status-pill-unknown {
            color: #c6c7d0;
            background-color: rgba(198, 199, 208, 0.12);
        }

        .quick-action-button {
            min-width: 40px;
            min-height: 40px;
            padding: 0;
            border-radius: 9999px;
            background: transparent;
            box-shadow: none;
            color: @accent_fg_color;
        }

        .quick-action-button:hover {
            background: rgba(255, 255, 255, 0.08);
        }

        .quick-action-button:active {
            background: rgba(255, 255, 255, 0.14);
        }

        .quick-action-button:disabled {
            color: alpha(currentColor, 0.45);
        }
        ",
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
