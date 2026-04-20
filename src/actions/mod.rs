pub(crate) mod catalog;

use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{
    CAMERA_APP_DESKTOP_ID, REBOOT_COMMAND, RESTORE_INTEL_CAMERA_COMMAND,
};

use crate::system::execute_privileged_shell_command;
use crate::ui::{build_button_row, new_action_button};
use crate::ui::SetupWindow;

pub(crate) use self::catalog::{
    ActionKey, ActionMetadata, action_metadata, dedupe_action_keys,
};

#[derive(Clone)]
struct CommandResult {
    title: String,
    success_message: String,
    output: String,
    success: bool,
    refresh_after: bool,
}

impl SetupWindow {
    pub(super) fn bind_events(&self) {
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
        self.repair_nvidia_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::RepairNvidia);
        });

        let this = self.clone();
        self.balanced_profile_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::SetBalancedProfile);
        });

        let this = self.clone();
        self.reboot_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::Reboot);
        });

        let this = self.clone();
        self.open_camera_button.connect_clicked(move |_| {
            this.invoke_action(ActionKey::OpenCamera);
        });
    }

    pub(super) fn build_suggested_action_row(
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

    pub(super) fn set_action_buttons_sensitive(&self, sensitive: bool) {
        let busy = *self.action_running.borrow();
        let allowed = sensitive && !busy;
        self.install_main_button.set_sensitive(allowed);
        self.install_button.set_sensitive(allowed);
        self.repair_button.set_sensitive(allowed);
        self.enable_camera_module_button.set_sensitive(allowed);
        self.force_driver_button.set_sensitive(allowed);
        self.restore_camera_button.set_sensitive(allowed);
        self.enable_browser_camera_button.set_sensitive(allowed);
        self.enable_speakers_button.set_sensitive(allowed);
        self.repair_nvidia_button.set_sensitive(allowed);
        self.balanced_profile_button.set_sensitive(allowed);
        self.reboot_button.set_sensitive(allowed);
        let open_allowed = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.camera_app_installed)
            .unwrap_or(false);
        self.open_camera_button.set_sensitive(allowed && open_allowed);
    }

    fn invoke_action(&self, key: ActionKey) {
        match key {
            ActionKey::InstallMainSupport => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.install_main_support_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Instalar suporte principal",
                    command,
                    "Pacotes principais instalados. Atualize o diagnóstico e use as ações específicas se câmera ou alto-falantes ainda precisarem de ajuste.",
                    true,
                );
            }
            ActionKey::InstallCamera => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.install_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Instalar suporte da câmera",
                    command,
                    "Instalação concluída. Reinicie o sistema para carregar o driver.",
                    true,
                );
            }
            ActionKey::RepairDriver => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.repair_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Reparar o driver",
                    command,
                    "Reparo concluído. Reinicie o sistema para aplicar o módulo atualizado.",
                    true,
                );
            }
            ActionKey::EnableCameraModule => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.enable_camera_module_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Habilitar driver da câmera",
                    command,
                    "Carregamento do ov02c10 ajustado. Se a câmera ainda não aparecer, reinicie o sistema para validar o boot completo.",
                    true,
                );
            }
            ActionKey::ForceDriverPriority => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.force_camera_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Ajustar prioridade do driver",
                    command,
                    "Ajuste concluído. Se o módulo ainda estiver em uso, reinicie o sistema antes de validar a câmera.",
                    true,
                );
            }
            ActionKey::RestoreIntelIpu6 => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.restore_intel_camera_command.clone())
                    .unwrap_or_else(|| RESTORE_INTEL_CAMERA_COMMAND.into());
                self.run_privileged_command(
                    "Restaurar stack Intel IPU6",
                    command,
                    "Restauração concluída. Se a câmera continuar ausente no libcamera, reinicie o sistema antes de validar novamente.",
                    true,
                );
            }
            ActionKey::EnableBrowserCamera => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.enable_browser_camera_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Ativar câmera para navegador",
                    command,
                    "Bridge V4L2 ativado. Se os nós crus do IPU6 ainda aparecerem na sessão atual, faça logout/login antes de validar Meet, Discord e outros apps.",
                    true,
                );
            }
            ActionKey::EnableSpeakers => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.enable_speaker_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Ativar alto-falantes internos",
                    command,
                    "Fluxo dos alto-falantes concluído. Se os módulos MAX98390 já aparecerem no kernel, teste a saída Speaker imediatamente. Reinicie só se o sistema continuar preso ao estado anterior.",
                    true,
                );
            }
            ActionKey::RepairNvidia => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.repair_nvidia_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Reparar suporte NVIDIA",
                    command,
                    "Fluxo NVIDIA concluído. Reinicie o sistema se os módulos ainda não aparecerem carregados.",
                    true,
                );
            }
            ActionKey::SetBalancedProfile => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.set_balanced_profile_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    "Definir perfil balanceado",
                    command,
                    "Perfil balanced aplicado com sucesso.",
                    true,
                );
            }
            ActionKey::Reboot => {
                self.run_privileged_command(
                    "Reiniciar o sistema",
                    REBOOT_COMMAND.into(),
                    "Reinicialização solicitada.",
                    false,
                );
            }
            ActionKey::OpenCamera => {
                if let Some(app) = gio::DesktopAppInfo::new(CAMERA_APP_DESKTOP_ID) {
                    if let Err(error) =
                        app.launch(&[], None::<&gio::AppLaunchContext>)
                    {
                        self.toast_overlay.add_toast(adw::Toast::new(&format!(
                            "Falha ao abrir o app da câmera: {error}"
                        )));
                    }
                } else {
                    self.toast_overlay.add_toast(adw::Toast::new(
                        "O Galaxy Book Câmera não foi encontrado no sistema.",
                    ));
                }
            }
        }
    }

    fn run_privileged_command(
        &self,
        title: &str,
        command: String,
        success_message: &str,
        refresh_after: bool,
    ) {
        if command.trim().is_empty() || *self.action_running.borrow() {
            return;
        }

        *self.action_running.borrow_mut() = true;
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.toast_overlay
            .add_toast(adw::Toast::new(&format!("Executando: {title}…")));

        let title_owned = title.to_string();
        let success_message_owned = success_message.to_string();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match execute_privileged_shell_command(&command) {
                Ok(output) => CommandResult {
                    title: title_owned,
                    success_message: success_message_owned,
                    output: output.output,
                    success: output.success,
                    refresh_after,
                },
                Err(error) => CommandResult {
                    title: title_owned,
                    success_message: success_message_owned,
                    output: error,
                    success: false,
                    refresh_after,
                },
            };
            let _ = sender.send(result);
        });

        let this = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            match receiver.try_recv() {
                Ok(result) => {
                    *this.action_running.borrow_mut() = false;
                    this.refresh_button.set_sensitive(true);
                    this.set_action_buttons_sensitive(true);

                    if result.success {
                        this.toast_overlay
                            .add_toast(adw::Toast::new(&result.success_message));
                        if result.refresh_after {
                            this.refresh();
                        }
                    } else {
                        this.present_command_result_dialog(
                            &result.title,
                            &result.output,
                        );
                    }

                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    *this.action_running.borrow_mut() = false;
                    this.refresh_button.set_sensitive(true);
                    this.set_action_buttons_sensitive(true);
                    this.toast_overlay.add_toast(adw::Toast::new(
                        "Falha ao acompanhar a ação solicitada.",
                    ));
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn present_command_result_dialog(&self, title: &str, output: &str) {
        let dialog = adw::Dialog::builder()
            .title(title)
            .content_width(680)
            .content_height(420)
            .build();

        let header = adw::HeaderBar::new();
        let window_title = adw::WindowTitle::new(title, "Saída da ação");
        header.set_title_widget(Some(&window_title));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);

        let text_view = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(16)
            .bottom_margin(16)
            .left_margin(16)
            .right_margin(16)
            .build();
        text_view.buffer().set_text(if output.trim().is_empty() {
            "A ação falhou, mas não retornou saída textual."
        } else {
            output
        });

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&text_view)
            .build();

        toolbar.set_content(Some(&scroller));
        dialog.set_child(Some(&toolbar));
        dialog.present(Some(&self.window));
    }
}

pub(crate) fn build_action_row(
    key: ActionKey,
    button: &gtk::Button,
) -> adw::ActionRow {
    let ActionMetadata { title, subtitle } = action_metadata(key);
    build_button_row(title, subtitle, button)
}
