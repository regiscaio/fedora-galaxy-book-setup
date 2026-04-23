use std::sync::mpsc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use galaxybook_setup::{SetupSnapshot, collect_snapshot, tr};

use crate::diagnostics::suggested_actions;
use crate::ui::DiagnosticKey;
use crate::ui::SetupWindow;

impl SetupWindow {
    pub(crate) fn refresh(&self) {
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.recommendation_title_row
            .set_subtitle(&tr("Atualizando diagnóstico…"));
        self.recommendation_body_row.set_subtitle(
            &tr("Aguarde enquanto o setup verifica pacotes, driver, akmods, câmera, áudio, fingerprint, GPU, plataforma e integrações do desktop."),
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
        let packages_actions = suggested_actions(&snapshot, DiagnosticKey::Packages);
        let akmods_actions = suggested_actions(&snapshot, DiagnosticKey::Akmods);
        let module_actions = suggested_actions(&snapshot, DiagnosticKey::Module);
        let libcamera_actions = suggested_actions(&snapshot, DiagnosticKey::Libcamera);
        let browser_actions = suggested_actions(&snapshot, DiagnosticKey::BrowserCamera);
        let boot_actions = suggested_actions(&snapshot, DiagnosticKey::Boot);
        let speakers_actions = suggested_actions(&snapshot, DiagnosticKey::Speakers);
        let sound_app_actions = suggested_actions(&snapshot, DiagnosticKey::SoundApp);
        let fingerprint_reader_actions =
            suggested_actions(&snapshot, DiagnosticKey::FingerprintReader);
        let fingerprint_login_actions =
            suggested_actions(&snapshot, DiagnosticKey::FingerprintLogin);
        let gpu_actions = suggested_actions(&snapshot, DiagnosticKey::Gpu);
        let secure_boot_actions =
            suggested_actions(&snapshot, DiagnosticKey::SecureBootKey);
        let platform_actions =
            suggested_actions(&snapshot, DiagnosticKey::PlatformProfile);
        let clipboard_actions = suggested_actions(&snapshot, DiagnosticKey::Clipboard);
        let gsconnect_actions = suggested_actions(&snapshot, DiagnosticKey::Gsconnect);
        let desktop_icons_actions =
            suggested_actions(&snapshot, DiagnosticKey::DesktopIcons);
        let dock_actions = suggested_actions(&snapshot, DiagnosticKey::Dock);

        self.device_row.set_subtitle(&snapshot.system.notebook);
        self.fedora_row.set_subtitle(&snapshot.system.fedora);
        self.kernel_row.set_subtitle(&snapshot.system.kernel);
        self.secure_boot_row.set_subtitle(&snapshot.system.secure_boot);

        self.packages_row.apply(&snapshot.packages);
        self.packages_row
            .set_suggested_actions_available(!packages_actions.is_empty());
        self.akmods_row.apply(&snapshot.akmods);
        self.akmods_row
            .set_suggested_actions_available(!akmods_actions.is_empty());
        self.module_row.apply(&snapshot.module);
        self.module_row
            .set_suggested_actions_available(!module_actions.is_empty());
        self.libcamera_row.apply(&snapshot.libcamera);
        self.libcamera_row
            .set_suggested_actions_available(!libcamera_actions.is_empty());
        self.browser_camera_row.apply(&snapshot.browser_camera);
        self.browser_camera_row
            .set_suggested_actions_available(!browser_actions.is_empty());
        self.boot_row.apply(&snapshot.boot);
        self.boot_row
            .set_suggested_actions_available(!boot_actions.is_empty());
        self.speakers_row.apply(&snapshot.speakers);
        self.speakers_row
            .set_suggested_actions_available(!speakers_actions.is_empty());
        self.sound_app_row.apply(&snapshot.sound_app);
        self.sound_app_row
            .set_suggested_actions_available(!sound_app_actions.is_empty());
        self.fingerprint_reader_row
            .apply(&snapshot.fingerprint_reader);
        self.fingerprint_reader_row
            .set_suggested_actions_available(!fingerprint_reader_actions.is_empty());
        self.fingerprint_login_row.apply(&snapshot.fingerprint_login);
        self.fingerprint_login_row
            .set_suggested_actions_available(!fingerprint_login_actions.is_empty());
        self.gpu_row.apply(&snapshot.gpu);
        self.gpu_row
            .set_suggested_actions_available(!gpu_actions.is_empty());
        self.secure_boot_key_row.apply(&snapshot.secure_boot_key);
        self.secure_boot_key_row
            .set_suggested_actions_available(!secure_boot_actions.is_empty());
        self.platform_profile_row.apply(&snapshot.platform_profile);
        self.platform_profile_row
            .set_suggested_actions_available(!platform_actions.is_empty());
        self.clipboard_row.apply(&snapshot.clipboard_extension);
        self.clipboard_row
            .set_suggested_actions_available(!clipboard_actions.is_empty());
        self.gsconnect_row.apply(&snapshot.gsconnect_extension);
        self.gsconnect_row
            .set_suggested_actions_available(!gsconnect_actions.is_empty());
        self.desktop_icons_row
            .apply(&snapshot.desktop_icons_extension);
        self.desktop_icons_row
            .set_suggested_actions_available(!desktop_icons_actions.is_empty());
        self.dock_row.apply(&snapshot.dock_extension);
        self.dock_row
            .set_suggested_actions_available(!dock_actions.is_empty());

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
        self.open_sound_button
            .set_sensitive(snapshot.sound_app_installed && !*self.action_running.borrow());
        self.refresh_button.set_sensitive(!*self.action_running.borrow());
        self.set_action_buttons_sensitive(!*self.action_running.borrow());

        *self.snapshot.borrow_mut() = Some(snapshot);
    }
}
