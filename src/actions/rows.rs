use gtk::prelude::*;
use libadwaita as adw;

use crate::actions::{ActionKey, ActionMetadata, action_icon_name, action_metadata};
use crate::ui::DiagnosticKey;
use crate::ui::{SetupWindow, build_button_row, new_action_button};
use galaxybook_setup::tr;

impl SetupWindow {
    pub(crate) fn build_suggested_action_row(
        &self,
        key: ActionKey,
        diagnostic: DiagnosticKey,
    ) -> Option<(adw::ActionRow, gtk::Button)> {
        if !self.operation_is_idle() {
            return None;
        }
        let metadata = action_metadata(key);
        let button = new_action_button(&tr(metadata.title), action_icon_name(key));
        button.set_sensitive(true);

        let this = self.clone();
        button.connect_clicked(move |_| {
            this.invoke_suggested_action(key, diagnostic);
        });

        let row = build_button_row(&tr(metadata.title), &tr(metadata.subtitle), &button);
        Some((row, button))
    }
}

pub(crate) fn build_action_row(key: ActionKey, button: &gtk::Button) -> adw::ActionRow {
    let ActionMetadata { title, subtitle } = action_metadata(key);
    build_button_row(&tr(title), &tr(subtitle), button)
}
