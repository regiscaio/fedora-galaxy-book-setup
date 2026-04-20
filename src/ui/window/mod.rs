mod build;
mod pages;
mod shell;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use libadwaita as adw;

use galaxybook_setup::SetupSnapshot;

use crate::diagnostics::DiagnosticAlertCounts;
use crate::ui::{InfoRow, StatusRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticKey {
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

#[derive(Clone)]
pub(crate) struct SetupWindow {
    pub(crate) app: adw::Application,
    pub(crate) window: adw::ApplicationWindow,
    pub(crate) navigation_view: adw::NavigationView,
    pub(crate) toast_overlay: adw::ToastOverlay,
    pub(crate) refresh_button: gtk::Button,
    pub(crate) recommendation_title_row: InfoRow,
    pub(crate) recommendation_body_row: InfoRow,
    pub(crate) device_row: InfoRow,
    pub(crate) fedora_row: InfoRow,
    pub(crate) kernel_row: InfoRow,
    pub(crate) secure_boot_row: InfoRow,
    pub(crate) packages_row: StatusRow,
    pub(crate) akmods_row: StatusRow,
    pub(crate) module_row: StatusRow,
    pub(crate) libcamera_row: StatusRow,
    pub(crate) browser_camera_row: StatusRow,
    pub(crate) boot_row: StatusRow,
    pub(crate) speakers_row: StatusRow,
    pub(crate) gpu_row: StatusRow,
    pub(crate) platform_profile_row: StatusRow,
    pub(crate) clipboard_row: StatusRow,
    pub(crate) gsconnect_row: StatusRow,
    pub(crate) desktop_icons_row: StatusRow,
    pub(crate) suggested_title_row: InfoRow,
    pub(crate) suggested_status_row: InfoRow,
    pub(crate) suggested_detail_row: InfoRow,
    pub(crate) suggested_actions_group: adw::PreferencesGroup,
    pub(crate) suggested_action_rows: Rc<RefCell<Vec<gtk::Widget>>>,
    pub(crate) install_main_button: gtk::Button,
    pub(crate) install_button: gtk::Button,
    pub(crate) repair_button: gtk::Button,
    pub(crate) enable_camera_module_button: gtk::Button,
    pub(crate) force_driver_button: gtk::Button,
    pub(crate) restore_camera_button: gtk::Button,
    pub(crate) enable_browser_camera_button: gtk::Button,
    pub(crate) enable_speakers_button: gtk::Button,
    pub(crate) repair_nvidia_button: gtk::Button,
    pub(crate) balanced_profile_button: gtk::Button,
    pub(crate) reboot_button: gtk::Button,
    pub(crate) open_camera_button: gtk::Button,
    pub(crate) snapshot: Rc<RefCell<Option<SetupSnapshot>>>,
    pub(crate) action_running: Rc<RefCell<bool>>,
    pub(crate) selected_diagnostic: Rc<RefCell<Option<DiagnosticKey>>>,
    pub(crate) notification_counts: Rc<RefCell<Option<DiagnosticAlertCounts>>>,
}

impl SetupWindow {
    pub(crate) fn present(&self) {
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

    fn bind_diagnostic_navigation(&self) {
        self.connect_diagnostic_row(&self.packages_row, DiagnosticKey::Packages);
        self.connect_diagnostic_row(&self.akmods_row, DiagnosticKey::Akmods);
        self.connect_diagnostic_row(&self.module_row, DiagnosticKey::Module);
        self.connect_diagnostic_row(&self.libcamera_row, DiagnosticKey::Libcamera);
        self.connect_diagnostic_row(
            &self.browser_camera_row,
            DiagnosticKey::BrowserCamera,
        );
        self.connect_diagnostic_row(&self.boot_row, DiagnosticKey::Boot);
        self.connect_diagnostic_row(&self.speakers_row, DiagnosticKey::Speakers);
        self.connect_diagnostic_row(&self.gpu_row, DiagnosticKey::Gpu);
        self.connect_diagnostic_row(
            &self.platform_profile_row,
            DiagnosticKey::PlatformProfile,
        );
        self.connect_diagnostic_row(&self.clipboard_row, DiagnosticKey::Clipboard);
        self.connect_diagnostic_row(&self.gsconnect_row, DiagnosticKey::Gsconnect);
        self.connect_diagnostic_row(
            &self.desktop_icons_row,
            DiagnosticKey::DesktopIcons,
        );
    }

    fn connect_diagnostic_row(&self, row: &StatusRow, key: DiagnosticKey) {
        let this = self.clone();
        row.connect_suggested_actions(move || {
            this.present_suggested_actions(key);
        });
    }
}
