use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{APP_NAME, tr};

use crate::ui::{
    InfoRow, build_navigation_row, build_scrolled_navigation_page,
};

pub(super) struct SuggestedPage {
    pub(super) page: adw::NavigationPage,
    pub(super) title_row: InfoRow,
    pub(super) status_row: InfoRow,
    pub(super) detail_row: InfoRow,
    pub(super) actions_group: adw::PreferencesGroup,
}

pub(super) fn build_sections_page(
    navigation_view: &adw::NavigationView,
    system_group: &adw::PreferencesGroup,
) -> adw::NavigationPage {
    let sections_page = adw::PreferencesPage::builder()
        .name("sections")
        .title(APP_NAME)
        .build();
    sections_page.add(system_group);

    let sections_group = adw::PreferencesGroup::builder()
        .title(tr("Áreas do assistente"))
        .description(tr("Acesse as áreas operacionais do auxiliar de instalação e diagnóstico."))
        .build();
    sections_group.add(&build_navigation_row(
        &tr("Diagnósticos"),
        &tr("Checklist geral da câmera, do áudio, da biometria, da GPU e das integrações do desktop."),
        {
            let navigation_view = navigation_view.clone();
            move || navigation_view.push_by_tag("flow")
        },
    ));
    sections_group.add(&build_navigation_row(
        &tr("Ações rápidas"),
        &tr("Execute instalação, reparo, cadastro de digitais, abertura dos apps finais e reinício direto da interface."),
        {
            let navigation_view = navigation_view.clone();
            move || navigation_view.push_by_tag("actions")
        },
    ));
    sections_page.add(&sections_group);

    build_scrolled_navigation_page(&sections_page, APP_NAME, "home")
}

pub(super) fn build_flow_page(
    diagnostics_group: &adw::PreferencesGroup,
    camera_group: &adw::PreferencesGroup,
    audio_group: &adw::PreferencesGroup,
    fingerprint_group: &adw::PreferencesGroup,
    gpu_group: &adw::PreferencesGroup,
    integrations_group: &adw::PreferencesGroup,
) -> adw::NavigationPage {
    let flow_page_content = adw::PreferencesPage::builder()
        .name("flow")
        .title(tr("Diagnósticos"))
        .build();
    flow_page_content.add(diagnostics_group);
    flow_page_content.add(camera_group);
    flow_page_content.add(audio_group);
    flow_page_content.add(fingerprint_group);
    flow_page_content.add(gpu_group);
    flow_page_content.add(integrations_group);

    build_scrolled_navigation_page(&flow_page_content, &tr("Diagnósticos"), "flow")
}

pub(super) fn build_actions_page(
    actions_group: &adw::PreferencesGroup,
) -> adw::NavigationPage {
    let actions_page_content = adw::PreferencesPage::builder()
        .name("actions")
        .title(tr("Ações rápidas"))
        .build();
    actions_page_content.add(actions_group);

    build_scrolled_navigation_page(
        &actions_page_content,
        &tr("Ações rápidas"),
        "actions",
    )
}

pub(super) fn build_suggested_page() -> SuggestedPage {
    let suggested_summary_group = adw::PreferencesGroup::builder()
        .title(tr("Diagnóstico selecionado"))
        .description(tr("Leitura do item selecionado e ações rápidas relacionadas ao problema ou à validação atual."))
        .build();
    let title_row = InfoRow::new("Item");
    let status_row = InfoRow::new("Status");
    let detail_row = InfoRow::new("Leitura");
    suggested_summary_group.add(&title_row.row);
    suggested_summary_group.add(&status_row.row);
    suggested_summary_group.add(&detail_row.row);

    let actions_group = adw::PreferencesGroup::builder()
        .title(tr("Ações sugeridas"))
        .description(tr("Ações rápidas filtradas para o diagnóstico selecionado."))
        .build();

    let page_content = adw::PreferencesPage::builder()
        .name("suggested-actions")
        .title(tr("Ações sugeridas"))
        .build();
    page_content.add(&suggested_summary_group);
    page_content.add(&actions_group);

    SuggestedPage {
        page: build_scrolled_navigation_page(
            &page_content,
            &tr("Ações sugeridas"),
            "suggested-actions",
        ),
        title_row,
        status_row,
        detail_row,
        actions_group,
    }
}
