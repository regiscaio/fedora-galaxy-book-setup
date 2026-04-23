use gtk::prelude::*;

use crate::actions::ActionKey;
use crate::ui::SetupWindow;

impl SetupWindow {
    pub(crate) fn bind_events(&self) {
        let this = self.clone();
        self.install_main_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::InstallMainSupport);
        });

        let this = self.clone();
        self.refresh_button.connect_clicked(move |_| {
            this.refresh();
        });

        let this = self.clone();
        self.install_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::InstallCamera);
        });

        let this = self.clone();
        self.install_sound_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::InstallSoundApp);
        });

        let this = self.clone();
        self.repair_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::RepairDriver);
        });

        let this = self.clone();
        self.enable_camera_module_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::EnableCameraModule);
        });

        let this = self.clone();
        self.force_driver_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::ForceDriverPriority);
        });

        let this = self.clone();
        self.restore_camera_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::RestoreIntelIpu6);
        });

        let this = self.clone();
        self.enable_browser_camera_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::EnableBrowserCamera);
        });

        let this = self.clone();
        self.enable_speakers_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::EnableSpeakers);
        });

        let this = self.clone();
        self.prepare_secure_boot_key_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::PrepareSecureBootKey);
        });

        let this = self.clone();
        self.repair_fingerprint_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::RepairFingerprintStack);
        });

        let this = self.clone();
        self.enable_fingerprint_auth_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::EnableFingerprintAuth);
        });

        let this = self.clone();
        self.open_fingerprint_settings_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::OpenFingerprintSettings);
        });

        let this = self.clone();
        self.repair_nvidia_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::RepairNvidia);
        });

        let this = self.clone();
        self.balanced_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::SetBalancedProfile);
        });

        let this = self.clone();
        self.clipboard_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::ApplyClipboardProfile);
        });

        let this = self.clone();
        self.gsconnect_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::ApplyGsconnectProfile);
        });

        let this = self.clone();
        self.desktop_icons_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::ApplyDesktopIconsProfile);
        });

        let this = self.clone();
        self.dock_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::ApplyDockProfile);
        });

        let this = self.clone();
        self.reboot_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::Reboot);
        });

        let this = self.clone();
        self.open_camera_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::OpenCamera);
        });

        let this = self.clone();
        self.open_sound_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::OpenSoundApp);
        });
    }

    pub(crate) fn set_action_buttons_sensitive(&self, sensitive: bool) {
        let busy = *self.action_running.borrow();
        let allowed = sensitive && !busy;
        self.install_main_button.set_sensitive(allowed);
        self.install_button.set_sensitive(allowed);
        self.install_sound_button.set_sensitive(allowed);
        self.repair_button.set_sensitive(allowed);
        self.enable_camera_module_button.set_sensitive(allowed);
        self.force_driver_button.set_sensitive(allowed);
        self.restore_camera_button.set_sensitive(allowed);
        self.enable_browser_camera_button.set_sensitive(allowed);
        self.enable_speakers_button.set_sensitive(allowed);
        self.prepare_secure_boot_key_button.set_sensitive(allowed);
        self.repair_fingerprint_button.set_sensitive(allowed);
        self.enable_fingerprint_auth_button.set_sensitive(allowed);
        self.open_fingerprint_settings_button.set_sensitive(allowed);
        self.repair_nvidia_button.set_sensitive(allowed);
        self.balanced_profile_button.set_sensitive(allowed);
        self.clipboard_profile_button.set_sensitive(allowed);
        self.gsconnect_profile_button.set_sensitive(allowed);
        self.desktop_icons_profile_button.set_sensitive(allowed);
        self.dock_profile_button.set_sensitive(allowed);
        self.reboot_button.set_sensitive(allowed);
        let open_allowed = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.camera_app_installed)
            .unwrap_or(false);
        self.open_camera_button.set_sensitive(allowed && open_allowed);
        let open_sound_allowed = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.sound_app_installed)
            .unwrap_or(false);
        self.open_sound_button
            .set_sensitive(allowed && open_sound_allowed);
    }
}
