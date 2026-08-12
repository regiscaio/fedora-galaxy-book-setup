use std::sync::mpsc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use galaxybook_setup::{SetupSnapshot, collect_snapshot, package_update_names, tr};

use crate::actions::SETUP_UPDATE_PACKAGES;

use crate::diagnostics::suggested_actions;
use crate::ui::DiagnosticKey;
use crate::ui::SetupWindow;

fn begin_refresh(action_running: bool, refresh_running: &mut bool) -> bool {
    if action_running || *refresh_running {
        return false;
    }
    *refresh_running = true;
    true
}

fn finish_refresh(refresh_running: &mut bool) {
    *refresh_running = false;
}

fn refresh_workers_complete(snapshot_complete: bool, updates_complete: bool) -> bool {
    snapshot_complete && updates_complete
}

impl SetupWindow {
    pub(crate) fn refresh(&self) {
        if !begin_refresh(
            *self.action_running.borrow(),
            &mut self.refresh_running.borrow_mut(),
        ) {
            return;
        }
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.set_diagnostic_navigation_sensitive(false);
        self.prepare_update_refresh();
        self.recommendation_title_row
            .set_subtitle(&tr("Atualizando diagnóstico…"));
        self.recommendation_body_row.set_subtitle(
            &tr("Aguarde enquanto o setup verifica pacotes, driver, akmods, câmera, áudio, fingerprint, GPU, plataforma e integrações do desktop."),
        );

        let (snapshot_sender, snapshot_receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let snapshot = collect_snapshot();
            let _ = snapshot_sender.send(snapshot);
        });
        let (updates_sender, updates_receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = updates_sender.send(package_update_names(SETUP_UPDATE_PACKAGES));
        });

        let this = self.clone();
        let mut snapshot_result = None;
        let mut updates_result = None;
        glib::timeout_add_local(Duration::from_millis(75), move || {
            if snapshot_result.is_none() {
                match snapshot_receiver.try_recv() {
                    Ok(snapshot) => snapshot_result = Some(Ok(snapshot)),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => snapshot_result = Some(Err(())),
                }
            }
            if updates_result.is_none() {
                match updates_receiver.try_recv() {
                    Ok(result) => updates_result = Some(result),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        updates_result = Some(Err(tr("Falha ao consultar atualizações.")))
                    }
                }
            }
            if !refresh_workers_complete(snapshot_result.is_some(), updates_result.is_some()) {
                return glib::ControlFlow::Continue;
            }

            finish_refresh(&mut this.refresh_running.borrow_mut());
            let snapshot_result = snapshot_result.take().expect("resultado do diagnóstico");
            let updates_result = updates_result.take().expect("resultado de atualizações");
            match snapshot_result {
                Ok(snapshot) => this.apply_snapshot(snapshot),
                Err(()) => {
                    this.refresh_button.set_sensitive(true);
                    this.set_action_buttons_sensitive(true);
                    this.set_diagnostic_navigation_sensitive(true);
                    this.toast_overlay
                        .add_toast(adw::Toast::new(&tr("Falha ao atualizar o diagnóstico.")));
                }
            }
            this.apply_update_refresh(updates_result);
            glib::ControlFlow::Break
        });
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
        let secure_boot_actions = suggested_actions(&snapshot, DiagnosticKey::SecureBootKey);
        let platform_actions = suggested_actions(&snapshot, DiagnosticKey::PlatformProfile);
        let clipboard_actions = suggested_actions(&snapshot, DiagnosticKey::Clipboard);
        let gsconnect_actions = suggested_actions(&snapshot, DiagnosticKey::Gsconnect);
        let desktop_icons_actions = suggested_actions(&snapshot, DiagnosticKey::DesktopIcons);
        let dock_actions = suggested_actions(&snapshot, DiagnosticKey::Dock);

        self.device_row.set_subtitle(&snapshot.system.notebook);
        self.fedora_row.set_subtitle(&snapshot.system.fedora);
        self.kernel_row.set_subtitle(&snapshot.system.kernel);
        self.secure_boot_row
            .set_subtitle(&snapshot.system.secure_boot);

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
        self.fingerprint_login_row
            .apply(&snapshot.fingerprint_login);
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
            .set_sensitive(snapshot.camera_app_installed && self.operation_is_idle());
        self.open_sound_button
            .set_sensitive(snapshot.sound_app_installed && self.operation_is_idle());
        self.refresh_button.set_sensitive(self.operation_is_idle());

        *self.snapshot.borrow_mut() = Some(snapshot);
        self.set_action_buttons_sensitive(self.operation_is_idle());
        self.set_diagnostic_navigation_sensitive(self.operation_is_idle());
    }

    fn set_diagnostic_navigation_sensitive(&self, sensitive: bool) {
        for row in [
            &self.packages_row,
            &self.akmods_row,
            &self.module_row,
            &self.libcamera_row,
            &self.browser_camera_row,
            &self.boot_row,
            &self.speakers_row,
            &self.sound_app_row,
            &self.fingerprint_reader_row,
            &self.fingerprint_login_row,
            &self.gpu_row,
            &self.secure_boot_key_row,
            &self.platform_profile_row,
            &self.clipboard_row,
            &self.gsconnect_row,
            &self.desktop_icons_row,
            &self.dock_row,
        ] {
            row.row.set_sensitive(sensitive);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{begin_refresh, finish_refresh, refresh_workers_complete};

    #[test]
    fn refresh_state_is_exclusive_and_cleared_on_every_completion_path() {
        let mut refresh_running = false;
        assert!(begin_refresh(false, &mut refresh_running));
        assert!(refresh_running);
        assert!(!begin_refresh(false, &mut refresh_running));
        finish_refresh(&mut refresh_running);
        assert!(!refresh_running);

        assert!(!begin_refresh(true, &mut refresh_running));
        assert!(!refresh_running);
        assert!(begin_refresh(false, &mut refresh_running));
        finish_refresh(&mut refresh_running);
        assert!(!refresh_running);
    }

    #[test]
    fn refresh_waits_for_snapshot_and_update_workers() {
        assert!(!refresh_workers_complete(false, false));
        assert!(!refresh_workers_complete(true, false));
        assert!(!refresh_workers_complete(false, true));
        assert!(refresh_workers_complete(true, true));
    }
}
