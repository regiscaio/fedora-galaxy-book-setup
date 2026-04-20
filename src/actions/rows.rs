use gtk::prelude::*;
use libadwaita as adw;

use crate::actions::{ActionKey, ActionMetadata, action_metadata};
use crate::ui::{SetupWindow, build_button_row, new_action_button};

impl SetupWindow {
    pub(crate) fn build_suggested_action_row(
        &self,
        key: ActionKey,
    ) -> adw::ActionRow {
        let metadata = action_metadata(key);
        let button = new_action_button(metadata.title);
        button.set_sensitive(!*self.action_running.borrow());

        let this = self.clone();
        button.connect_clicked(move |_| {
            this.invoke_action(key);
        });

        build_button_row(metadata.title, metadata.subtitle, &button)
    }
}

pub(crate) fn build_action_row(
    key: ActionKey,
    button: &gtk::Button,
) -> adw::ActionRow {
    let ActionMetadata { title, subtitle } = action_metadata(key);
    build_button_row(title, subtitle, button)
}
