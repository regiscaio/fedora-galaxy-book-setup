use libadwaita as adw;
use libadwaita::prelude::*;

use crate::actions::{ActionKey, dedupe_action_keys};
use crate::diagnostics::{diagnostic_item, suggested_actions};
use crate::ui::{DiagnosticKey, SetupWindow};

use galaxybook_setup::SetupSnapshot;

impl SetupWindow {
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

    pub(crate) fn apply_suggested_actions(
        &self,
        snapshot: &SetupSnapshot,
        key: DiagnosticKey,
    ) {
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
