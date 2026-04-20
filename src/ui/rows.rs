use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{CheckItem, Health};

use super::apply_status_class;

#[derive(Clone)]
pub(crate) struct StatusRow {
    pub(crate) row: adw::ActionRow,
    icon: gtk::Image,
    badge: gtk::Label,
    next_button: gtk::Button,
}

impl StatusRow {
    pub(crate) fn new(title: &'static str) -> Self {
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

    pub(crate) fn apply(&self, item: &CheckItem) {
        self.row.set_subtitle(&item.detail);
        self.icon.set_icon_name(Some(item.health.icon_name()));
        apply_status_class(&self.icon, item.health);
        self.badge.set_label(item.health.label());
        apply_status_class(&self.badge, item.health);
    }

    pub(crate) fn connect_suggested_actions<F>(&self, on_activate: F)
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
pub(crate) struct InfoRow {
    pub(crate) row: adw::ActionRow,
}

impl InfoRow {
    pub(crate) fn new(title: &'static str) -> Self {
        let row = adw::ActionRow::builder().title(title).build();
        row.set_subtitle("Coletando…");
        Self { row }
    }

    pub(crate) fn set_subtitle(&self, subtitle: &str) {
        self.row.set_subtitle(subtitle);
    }
}
