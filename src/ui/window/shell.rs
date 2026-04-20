use gtk::gio;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{APP_NAME, tr};

pub(super) struct WindowShell {
    pub(super) window: adw::ApplicationWindow,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) navigation_view: adw::NavigationView,
    pub(super) refresh_button: gtk::Button,
}

pub(super) fn build_window_shell(app: &adw::Application) -> WindowShell {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .default_width(980)
        .default_height(760)
        .title(APP_NAME)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_hexpand(true);
    toast_overlay.set_vexpand(true);

    let header_title = adw::WindowTitle::new(APP_NAME, "");
    let back_button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text(tr("Voltar"))
        .visible(false)
        .build();
    back_button.add_css_class("flat");

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&header_title));
    header.pack_start(&back_button);

    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(tr("Atualizar diagnóstico"))
        .build();
    header.pack_end(&refresh_button);

    let menu = gio::Menu::new();
    menu.append(Some(&tr("Sobre")), Some("app.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();
    header.pack_end(&menu_button);

    let navigation_view = adw::NavigationView::new();
    navigation_view.set_animate_transitions(true);
    navigation_view.set_pop_on_escape(true);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&navigation_view));
    toast_overlay.set_child(Some(&toolbar));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&toast_overlay);
    window.set_content(Some(&root));

    back_button.connect_clicked({
        let navigation_view = navigation_view.clone();
        move |_| {
            navigation_view.pop();
        }
    });

    navigation_view.connect_visible_page_notify({
        let header_title = header_title.clone();
        let back_button = back_button.clone();
        move |navigation_view| {
            let Some(page) = navigation_view.visible_page() else {
                header_title.set_title(APP_NAME);
                back_button.set_visible(false);
                return;
            };

            header_title.set_title(page.title().as_str());
            back_button.set_visible(navigation_view.previous_page(&page).is_some());
        }
    });

    WindowShell {
        window,
        toast_overlay,
        navigation_view,
        refresh_button,
    }
}
