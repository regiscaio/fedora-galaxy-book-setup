use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{APP_ID, SetupSnapshot, collect_snapshot};

use crate::actions::{ActionKey, dedupe_action_keys};
use crate::diagnostics::{
    DiagnosticAlertCounts, diagnostic_alert_counts,
    diagnostic_item, diagnostic_notification_body, diagnostic_notification_title,
    suggested_actions,
};
use crate::{DiagnosticKey, SetupWindow};

impl SetupWindow {
    pub(crate) fn refresh(&self) {
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
        glib::timeout_add_local(
            Duration::from_millis(75),
            move || match receiver.try_recv() {
                Ok(snapshot) => {
                    this.apply_snapshot(snapshot);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    this.refresh_button.set_sensitive(true);
                    this.set_action_buttons_sensitive(true);
                    this.toast_overlay.add_toast(adw::Toast::new(
                        "Falha ao atualizar o diagnóstico.",
                    ));
                    glib::ControlFlow::Break
                }
            },
        );
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
        let Ok(connection) =
            gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        else {
            return;
        };

        let mut properties = HashMap::new();
        properties.insert("count".to_string(), counts.total().to_variant());
        properties.insert(
            "count-visible".to_string(),
            (!counts.is_clear()).to_variant(),
        );
        properties.insert("urgent".to_string(), (counts.errors > 0).to_variant());

        let parameters =
            (format!("application://{APP_ID}.desktop"), properties).to_variant();

        let _ = connection.emit_signal(
            None::<&str>,
            "/com/canonical/Unity/LauncherEntry",
            "com.canonical.Unity.LauncherEntry",
            "Update",
            Some(&parameters),
        );
    }

    pub(crate) fn present_suggested_actions(&self, key: DiagnosticKey) {
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
