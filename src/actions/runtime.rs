use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{
    CAMERA_APP_DESKTOP_ID, REBOOT_COMMAND, RESTORE_INTEL_CAMERA_COMMAND, tr, trf,
};

use crate::actions::ActionKey;
use crate::system::{
    execute_privileged_shell_command, execute_user_shell_command,
};
use crate::ui::SetupWindow;

#[derive(Clone)]
struct CommandResult {
    title: String,
    success_message: String,
    output: String,
    success: bool,
    refresh_after: bool,
}

#[derive(Clone, Copy)]
enum CommandMode {
    User,
    Privileged,
}

impl SetupWindow {
    pub(super) fn invoke_action(&self, key: ActionKey) {
        match key {
            ActionKey::InstallMainSupport => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.install_main_support_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    &tr("Instalar suporte principal"),
                    command,
                    &tr("Pacotes principais instalados. Atualize o diagnóstico e use as ações específicas se câmera ou alto-falantes ainda precisarem de ajuste."),
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
                    &tr("Instalar suporte da câmera"),
                    command,
                    &tr("Instalação concluída. Reinicie o sistema para carregar o driver."),
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
                    &tr("Reparar o driver"),
                    command,
                    &tr("Reparo concluído. Reinicie o sistema para aplicar o módulo atualizado."),
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
                    &tr("Habilitar driver da câmera"),
                    command,
                    &tr("Carregamento do ov02c10 ajustado. Se a câmera ainda não aparecer, reinicie o sistema para validar o boot completo."),
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
                    &tr("Ajustar prioridade do driver"),
                    command,
                    &tr("Ajuste concluído. Se o módulo ainda estiver em uso, reinicie o sistema antes de validar a câmera."),
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
                    &tr("Restaurar stack Intel IPU6"),
                    command,
                    &tr("Restauração concluída. Se a câmera continuar ausente no libcamera, reinicie o sistema antes de validar novamente."),
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
                    &tr("Ativar câmera para navegador"),
                    command,
                    &tr("Bridge V4L2 ativado. Se os nós crus do IPU6 ainda aparecerem na sessão atual, faça logout/login antes de validar Meet, Discord e outros apps."),
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
                    &tr("Ativar alto-falantes internos"),
                    command,
                    &tr("Fluxo dos alto-falantes concluído. Se os módulos MAX98390 já aparecerem no kernel, teste a saída Speaker imediatamente. Reinicie só se o sistema continuar preso ao estado anterior."),
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
                    &tr("Reparar suporte NVIDIA"),
                    command,
                    &tr("Fluxo NVIDIA concluído. Reinicie o sistema se os módulos ainda não aparecerem carregados."),
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
                    &tr("Definir perfil balanceado"),
                    command,
                    &tr("Perfil balanced aplicado com sucesso."),
                    true,
                );
            }
            ActionKey::ApplyClipboardProfile => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.apply_clipboard_profile_command.clone())
                    .unwrap_or_default();
                self.run_user_command(
                    &tr("Ativar histórico da área de transferência"),
                    command,
                    &tr("Perfil do histórico da área de transferência aplicado com sucesso."),
                    true,
                );
            }
            ActionKey::ApplyGsconnectProfile => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.apply_gsconnect_profile_command.clone())
                    .unwrap_or_default();
                self.run_user_command(
                    &tr("Ativar GSConnect"),
                    command,
                    &tr("Perfil do GSConnect aplicado com sucesso."),
                    true,
                );
            }
            ActionKey::ApplyDesktopIconsProfile => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| {
                        snapshot.apply_desktop_icons_profile_command.clone()
                    })
                    .unwrap_or_default();
                self.run_user_command(
                    &tr("Ativar ícones na área de trabalho"),
                    command,
                    &tr("Perfil dos ícones da área de trabalho aplicado com sucesso."),
                    true,
                );
            }
            ActionKey::ApplyDockProfile => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.apply_dock_profile_command.clone())
                    .unwrap_or_default();
                self.run_user_command(
                    &tr("Aplicar perfil da dock"),
                    command,
                    &tr("Perfil da dock aplicado com sucesso."),
                    true,
                );
            }
            ActionKey::Reboot => {
                self.run_privileged_command(
                    &tr("Reiniciar o sistema"),
                    REBOOT_COMMAND.into(),
                    &tr("Reinicialização solicitada."),
                    false,
                );
            }
            ActionKey::OpenCamera => {
                if let Some(app) = gio::DesktopAppInfo::new(CAMERA_APP_DESKTOP_ID) {
                    if let Err(error) =
                        app.launch(&[], None::<&gio::AppLaunchContext>)
                    {
                        self.toast_overlay.add_toast(adw::Toast::new(&trf(
                            "Falha ao abrir o app da câmera: {error}",
                            &[("error", error.to_string())],
                        )));
                    }
                } else {
                    self.toast_overlay.add_toast(adw::Toast::new(
                        &tr("O Galaxy Book Câmera não foi encontrado no sistema."),
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
        self.run_command(
            title,
            command,
            success_message,
            refresh_after,
            CommandMode::Privileged,
        );
    }

    fn run_user_command(
        &self,
        title: &str,
        command: String,
        success_message: &str,
        refresh_after: bool,
    ) {
        self.run_command(
            title,
            command,
            success_message,
            refresh_after,
            CommandMode::User,
        );
    }

    fn run_command(
        &self,
        title: &str,
        command: String,
        success_message: &str,
        refresh_after: bool,
        mode: CommandMode,
    ) {
        if command.trim().is_empty() || *self.action_running.borrow() {
            return;
        }

        *self.action_running.borrow_mut() = true;
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.toast_overlay
            .add_toast(adw::Toast::new(&trf(
                "Executando: {title}…",
                &[("title", title.to_string())],
            )));

        let title_owned = title.to_string();
        let success_message_owned = success_message.to_string();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let command_result = match mode {
                CommandMode::User => execute_user_shell_command(&command),
                CommandMode::Privileged => execute_privileged_shell_command(&command),
            };
            let result = match command_result {
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
                        &tr("Falha ao acompanhar a ação solicitada."),
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
        let window_title = adw::WindowTitle::new(title, &tr("Saída da ação"));
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
        let fallback_output = tr("A ação falhou, mas não retornou saída textual.");
        let output_text = if output.trim().is_empty() {
            fallback_output.as_str()
        } else {
            output
        };
        text_view.buffer().set_text(output_text);

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
