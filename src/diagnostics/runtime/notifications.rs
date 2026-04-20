use std::collections::HashMap;

use gtk::gio;
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;

use galaxybook_setup::{APP_ID, SetupSnapshot};

use crate::diagnostics::{
    DiagnosticAlertCounts, diagnostic_alert_counts,
    diagnostic_notification_body, diagnostic_notification_title,
};
use crate::ui::SetupWindow;

impl SetupWindow {
    pub(crate) fn update_diagnostic_notifications(&self, snapshot: &SetupSnapshot) {
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
}
