use std::cell::RefCell;
use std::rc::Rc;

use libadwaita as adw;

use crate::ui::install_css;

use super::SetupWindow;
use super::groups::{
    build_diagnostics_sections, build_quick_actions_section, build_system_section,
};
use super::pages::{
    build_actions_page, build_flow_page, build_sections_page, build_suggested_page,
};
use super::shell::build_window_shell;

impl SetupWindow {
    pub(crate) fn new(app: &adw::Application) -> Self {
        install_css();

        let shell = build_window_shell(app);
        let window = shell.window;
        let toast_overlay = shell.toast_overlay;
        let navigation_view = shell.navigation_view;
        let refresh_button = shell.refresh_button;

        let system = build_system_section();
        let diagnostics = build_diagnostics_sections();
        let quick_actions = build_quick_actions_section();

        let root_page = build_sections_page(&navigation_view, &system.group);
        let flow_page = build_flow_page(
            &diagnostics.diagnostics_group,
            &diagnostics.camera_group,
            &diagnostics.audio_group,
            &diagnostics.fingerprint_group,
            &diagnostics.gpu_group,
            &diagnostics.integrations_group,
        );
        let actions_page = build_actions_page(&quick_actions.group);
        let suggested_page = build_suggested_page();

        navigation_view.add(&root_page);
        navigation_view.add(&flow_page);
        navigation_view.add(&actions_page);
        navigation_view.add(&suggested_page.page);
        navigation_view.replace_with_tags(&["home"]);

        let snapshot = Rc::new(RefCell::new(None));
        let action_running = Rc::new(RefCell::new(false));
        let selected_diagnostic = Rc::new(RefCell::new(None));
        let suggested_action_rows = Rc::new(RefCell::new(Vec::new()));
        let notification_counts = Rc::new(RefCell::new(None));

        let instance = Self {
            app: app.clone(),
            window,
            navigation_view,
            toast_overlay,
            refresh_button,
            recommendation_title_row: diagnostics.recommendation_title_row,
            recommendation_body_row: diagnostics.recommendation_body_row,
            device_row: system.device_row,
            fedora_row: system.fedora_row,
            kernel_row: system.kernel_row,
            secure_boot_row: system.secure_boot_row,
            packages_row: diagnostics.packages_row,
            akmods_row: diagnostics.akmods_row,
            module_row: diagnostics.module_row,
            libcamera_row: diagnostics.libcamera_row,
            browser_camera_row: diagnostics.browser_camera_row,
            boot_row: diagnostics.boot_row,
            speakers_row: diagnostics.speakers_row,
            sound_app_row: diagnostics.sound_app_row,
            fingerprint_reader_row: diagnostics.fingerprint_reader_row,
            fingerprint_login_row: diagnostics.fingerprint_login_row,
            gpu_row: diagnostics.gpu_row,
            platform_profile_row: diagnostics.platform_profile_row,
            clipboard_row: diagnostics.clipboard_row,
            gsconnect_row: diagnostics.gsconnect_row,
            desktop_icons_row: diagnostics.desktop_icons_row,
            dock_row: diagnostics.dock_row,
            suggested_title_row: suggested_page.title_row,
            suggested_status_row: suggested_page.status_row,
            suggested_detail_row: suggested_page.detail_row,
            suggested_actions_group: suggested_page.actions_group,
            suggested_action_rows,
            install_main_button: quick_actions.install_main_button,
            install_button: quick_actions.install_button,
            install_sound_button: quick_actions.install_sound_button,
            repair_button: quick_actions.repair_button,
            enable_camera_module_button: quick_actions.enable_camera_module_button,
            force_driver_button: quick_actions.force_driver_button,
            restore_camera_button: quick_actions.restore_camera_button,
            enable_browser_camera_button: quick_actions.enable_browser_camera_button,
            enable_speakers_button: quick_actions.enable_speakers_button,
            repair_fingerprint_button: quick_actions.repair_fingerprint_button,
            enable_fingerprint_auth_button: quick_actions.enable_fingerprint_auth_button,
            open_fingerprint_settings_button: quick_actions.open_fingerprint_settings_button,
            repair_nvidia_button: quick_actions.repair_nvidia_button,
            balanced_profile_button: quick_actions.balanced_profile_button,
            clipboard_profile_button: quick_actions.clipboard_profile_button,
            gsconnect_profile_button: quick_actions.gsconnect_profile_button,
            desktop_icons_profile_button: quick_actions.desktop_icons_profile_button,
            dock_profile_button: quick_actions.dock_profile_button,
            reboot_button: quick_actions.reboot_button,
            open_camera_button: quick_actions.open_camera_button,
            open_sound_button: quick_actions.open_sound_button,
            snapshot,
            action_running,
            selected_diagnostic,
            notification_counts,
        };

        instance.install_actions(app);
        instance.bind_events();
        instance.bind_diagnostic_navigation();
        instance
    }
}
