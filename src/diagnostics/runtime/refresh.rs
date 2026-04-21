use std::sync::mpsc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use galaxybook_setup::{SetupSnapshot, collect_snapshot, tr};

use crate::ui::SetupWindow;

impl SetupWindow {
    pub(crate) fn refresh(&self) {
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.recommendation_title_row
            .set_subtitle(&tr("Atualizando diagnóstico…"));
        self.recommendation_body_row.set_subtitle(
            &tr("Aguarde enquanto o setup verifica pacotes, driver, akmods, câmera, GPU, plataforma e integrações do desktop."),
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
                        &tr("Falha ao atualizar o diagnóstico."),
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
        self.dock_row.apply(&snapshot.dock_extension);

        self.recommendation_title_row
            .set_subtitle(&tr(&snapshot.recommendation_title));
        self.recommendation_body_row
            .set_subtitle(&tr(&snapshot.recommendation_body));

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
}
