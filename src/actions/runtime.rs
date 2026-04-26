use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{
    AKMODS_PUBLIC_KEY_PATH, CAMERA_APP_DESKTOP_ID,
    OPEN_FINGERPRINT_SETTINGS_COMMAND, REBOOT_COMMAND,
    RESTORE_INTEL_CAMERA_COMMAND, SOUND_APP_DESKTOP_ID, package_update_names, tr, trf,
};

use crate::actions::ActionKey;
use crate::system::{
    execute_privileged_shell_command, execute_privileged_shell_command_with_input,
    execute_user_shell_command,
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

const SETUP_UPDATE_PACKAGES: &[&str] = &[
    "galaxybook-setup",
    "galaxybook-camera",
    "galaxybook-ov02c10-kmod-common",
    "akmod-galaxybook-ov02c10",
    "galaxybook-sound",
    "galaxybook-max98390-kmod-common",
    "akmod-galaxybook-max98390",
];

fn update_button_tooltip(packages: &[String]) -> String {
    trf(
        "Baixar e instalar atualizações: {packages}",
        &[("packages", packages.join(", "))],
    )
}

fn upgrade_installed_packages_command(packages: &[&str]) -> String {
    format!(
        r#"set -euo pipefail
packages=({packages})
installed=()
for package in "${{packages[@]}}"; do
  if rpm -q "$package" >/dev/null 2>&1; then
    installed+=("$package")
  fi
done
if [ "${{#installed[@]}}" -eq 0 ]; then
  echo "Nenhum pacote do Galaxy Book está instalado para atualizar."
  exit 0
fi
dnf upgrade -y "${{installed[@]}}"
"#,
        packages = packages.join(" ")
    )
}

fn output_mentions_secure_boot_key_rejection(output: &str) -> bool {
    output.contains("Key was rejected by service")
        || output.contains("key was rejected by service")
}

fn format_command_output(output: &str) -> String {
    let fallback_output = tr("A ação falhou, mas não retornou saída textual.");
    let output_text = if output.trim().is_empty() {
        fallback_output
    } else {
        output.to_string()
    };

    if !output_mentions_secure_boot_key_rejection(&output_text) {
        return output_text;
    }

    let remediation = if std::path::Path::new(AKMODS_PUBLIC_KEY_PATH).is_file() {
        trf(
            "O kernel rejeitou o módulo porque o Secure Boot continua ativo, mas a chave usada pelo akmods não foi aceita pelo MOK.\n\nVerifique com:\n  mokutil --test-key {path}\n\nSe a saída disser que a chave já está inscrita (\"already enrolled\"), o MOK atual já conhece essa chave e o problema precisa ser investigado por outro ponto. Se a chave ainda não estiver inscrita, execute:\n  sudo mokutil --import {path}\n\nDepois reinicie, entre em \"Enroll MOK\" na tela azul do boot, confirme a senha definida no import e só então repita a ação.",
            &[("path", AKMODS_PUBLIC_KEY_PATH.to_string())],
        )
    } else {
        tr(
            "O kernel rejeitou o módulo porque o Secure Boot continua ativo, mas o sistema não encontrou a chave pública do akmods em /etc/pki/akmods/certs/public_key.der. Gere ou reinstale a chave do akmods antes de repetir a ação.",
        )
    };

    format!("{remediation}\n\n---\n\n{output_text}")
}

fn secure_boot_password_error(
    password: &str,
    confirmation: &str,
) -> Option<String> {
    if password.trim().is_empty() || confirmation.trim().is_empty() {
        return Some(tr("Defina e confirme a senha temporária do MOK para continuar."));
    }

    if password != confirmation {
        return Some(tr("As duas senhas do MOK precisam ser idênticas."));
    }

    None
}

fn update_secure_boot_password_state(
    password_entry: &gtk::PasswordEntry,
    confirmation_entry: &gtk::PasswordEntry,
    error_label: &gtk::Label,
    confirm_button: &gtk::Button,
) {
    let password = password_entry.text().to_string();
    let confirmation = confirmation_entry.text().to_string();

    if password.is_empty() && confirmation.is_empty() {
        error_label.set_visible(false);
        confirm_button.set_sensitive(false);
        return;
    }

    if let Some(error) = secure_boot_password_error(&password, &confirmation) {
        error_label.set_label(&error);
        error_label.set_visible(true);
        confirm_button.set_sensitive(false);
        return;
    }

    error_label.set_visible(false);
    confirm_button.set_sensitive(true);
}

impl SetupWindow {
    pub(crate) fn refresh_updates(&self) {
        self.update_button.set_visible(false);
        self.update_button.set_sensitive(false);

        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(package_update_names(SETUP_UPDATE_PACKAGES));
        });

        let this = self.clone();
        glib::timeout_add_local(Duration::from_millis(150), move || match receiver.try_recv() {
            Ok(Ok(packages)) => {
                let has_updates = !packages.is_empty();
                let allowed = has_updates && !*this.action_running.borrow();
                this.update_button.set_visible(has_updates);
                this.update_button.set_sensitive(allowed);
                if has_updates {
                    this.update_button
                        .set_tooltip_text(Some(&update_button_tooltip(&packages)));
                }
                glib::ControlFlow::Break
            }
            Ok(Err(_error)) => {
                this.update_button.set_visible(false);
                this.update_button.set_sensitive(false);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                this.update_button.set_visible(false);
                this.update_button.set_sensitive(false);
                glib::ControlFlow::Break
            }
        });
    }

    pub(crate) fn install_updates(&self) {
        if !self.update_button.is_visible() || !self.update_button.is_sensitive() {
            return;
        }

        self.run_privileged_command(
            &tr("Atualizar pacotes"),
            upgrade_installed_packages_command(SETUP_UPDATE_PACKAGES),
            &tr("Atualizações instaladas. Reinicie os apps atualizados antes de continuar."),
            true,
        );
    }

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
            ActionKey::InstallSoundApp => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.install_sound_app_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    &tr("Instalar Galaxy Book Sound"),
                    command,
                    &tr("Galaxy Book Sound instalado. Abra o painel para ajustar equalizador, perfis e Atmos compatível."),
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
            ActionKey::PrepareSecureBootKey => {
                self.present_secure_boot_password_dialog();
            }
            ActionKey::RepairFingerprintStack => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| snapshot.repair_fingerprint_command.clone())
                    .unwrap_or_default();
                self.run_privileged_command(
                    &tr("Reinstalar stack de fingerprint"),
                    command,
                    &tr("Stack de fingerprint reinstalado. Atualize o diagnóstico e abra o cadastro de digitais se quiser testar novamente."),
                    true,
                );
            }
            ActionKey::EnableFingerprintAuth => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| {
                        snapshot.enable_fingerprint_auth_command.clone()
                    })
                    .unwrap_or_default();
                self.run_privileged_command(
                    &tr("Ativar login por digital"),
                    command,
                    &tr("Integração do authselect com fingerprint aplicada com sucesso."),
                    true,
                );
            }
            ActionKey::OpenFingerprintSettings => {
                let command = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .map(|snapshot| {
                        snapshot.open_fingerprint_settings_command.clone()
                    })
                    .unwrap_or_else(|| OPEN_FINGERPRINT_SETTINGS_COMMAND.into());
                self.run_user_command(
                    &tr("Abrir cadastro de digitais"),
                    command,
                    &tr("As configurações de usuários foram abertas para gerenciar as digitais."),
                    false,
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
            ActionKey::OpenSoundApp => {
                if let Some(app) = gio::DesktopAppInfo::new(SOUND_APP_DESKTOP_ID) {
                    if let Err(error) =
                        app.launch(&[], None::<&gio::AppLaunchContext>)
                    {
                        self.toast_overlay.add_toast(adw::Toast::new(&trf(
                            "Falha ao abrir o painel de som: {error}",
                            &[("error", error.to_string())],
                        )));
                    }
                } else {
                    self.toast_overlay.add_toast(adw::Toast::new(
                        &tr("O Galaxy Book Sound não foi encontrado no sistema."),
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
            None,
            success_message,
            refresh_after,
            CommandMode::Privileged,
        );
    }

    fn run_privileged_command_with_input(
        &self,
        title: &str,
        command: String,
        stdin_data: String,
        success_message: &str,
        refresh_after: bool,
    ) {
        self.run_command(
            title,
            command,
            Some(stdin_data),
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
            None,
            success_message,
            refresh_after,
            CommandMode::User,
        );
    }

    fn run_command(
        &self,
        title: &str,
        command: String,
        stdin_data: Option<String>,
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
                CommandMode::Privileged => match stdin_data {
                    Some(stdin) => execute_privileged_shell_command_with_input(
                        &command,
                        &stdin,
                    ),
                    None => execute_privileged_shell_command(&command),
                },
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

    fn present_secure_boot_password_dialog(&self) {
        if *self.action_running.borrow() {
            return;
        }

        let command = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.prepare_secure_boot_key_command.clone())
            .unwrap_or_default();
        if command.trim().is_empty() {
            return;
        }

        let dialog = adw::Dialog::builder()
            .title(tr("Preparar chave do Secure Boot"))
            .content_width(520)
            .build();

        let header = adw::HeaderBar::new();
        let window_title = adw::WindowTitle::new(
            &tr("Preparar chave do Secure Boot"),
            &tr("Senha temporária para o Enroll MOK"),
        );
        header.set_title_widget(Some(&window_title));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();

        let intro = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label(tr("Esta ação prepara a chave do akmods para o Secure Boot e cria o pedido de importação no MOK. A senha abaixo será pedida na tela azul do boot quando você escolher 'Enroll MOK'."))
            .build();
        content.append(&intro);

        let password_label = gtk::Label::builder()
            .label(tr("Senha temporária do MOK"))
            .xalign(0.0)
            .build();
        content.append(&password_label);

        let password_entry = gtk::PasswordEntry::builder()
            .show_peek_icon(true)
            .build();
        content.append(&password_entry);

        let confirmation_label = gtk::Label::builder()
            .label(tr("Confirmar senha"))
            .xalign(0.0)
            .build();
        content.append(&confirmation_label);

        let confirmation_entry = gtk::PasswordEntry::builder()
            .show_peek_icon(true)
            .build();
        content.append(&confirmation_entry);

        let hint = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label(tr("Depois de importar a chave, será necessário reiniciar e concluir o 'Enroll MOK' manualmente no boot."))
            .build();
        hint.add_css_class("dim-label");
        content.append(&hint);

        let error_label = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .build();
        error_label.add_css_class("error");
        content.append(&error_label);

        let actions = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk::Align::End)
            .build();
        let cancel_button = gtk::Button::with_label(&tr("Cancelar"));
        let confirm_button = gtk::Button::with_label(&tr("Preparar"));
        confirm_button.add_css_class("suggested-action");
        confirm_button.set_sensitive(false);
        actions.append(&cancel_button);
        actions.append(&confirm_button);
        content.append(&actions);

        let password_entry_for_state = password_entry.clone();
        let confirmation_entry_for_state = confirmation_entry.clone();
        let error_label_for_state = error_label.clone();
        let confirm_button_for_state = confirm_button.clone();
        password_entry.connect_changed(move |_| {
            update_secure_boot_password_state(
                &password_entry_for_state,
                &confirmation_entry_for_state,
                &error_label_for_state,
                &confirm_button_for_state,
            );
        });

        let password_entry_for_state = password_entry.clone();
        let confirmation_entry_for_state = confirmation_entry.clone();
        let error_label_for_state = error_label.clone();
        let confirm_button_for_state = confirm_button.clone();
        confirmation_entry.connect_changed(move |_| {
            update_secure_boot_password_state(
                &password_entry_for_state,
                &confirmation_entry_for_state,
                &error_label_for_state,
                &confirm_button_for_state,
            );
        });

        confirmation_entry.connect_activate({
            let confirm_button = confirm_button.clone();
            move |_| {
                if confirm_button.is_sensitive() {
                    confirm_button.emit_clicked();
                }
            }
        });

        cancel_button.connect_clicked({
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
            }
        });

        confirm_button.connect_clicked({
            let this = self.clone();
            let dialog = dialog.clone();
            let password_entry = password_entry.clone();
            let confirmation_entry = confirmation_entry.clone();
            let error_label = error_label.clone();
            move |_| {
                let password = password_entry.text().to_string();
                let confirmation = confirmation_entry.text().to_string();

                if let Some(error) =
                    secure_boot_password_error(&password, &confirmation)
                {
                    error_label.set_label(&error);
                    error_label.set_visible(true);
                    return;
                }

                dialog.close();
                let stdin_data = format!("{password}\n{confirmation}\n");
                this.run_privileged_command_with_input(
                    &tr("Preparar chave do Secure Boot"),
                    command.clone(),
                    stdin_data,
                    &tr("Fluxo do Secure Boot concluído. Se o pedido de importação foi criado agora, reinicie e conclua o Enroll MOK no boot antes de repetir a ação do driver."),
                    true,
                );
            }
        });

        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));
        dialog.present(Some(&self.window));
        password_entry.grab_focus();
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
        let output_text = format_command_output(output);
        text_view.buffer().set_text(&output_text);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_boot_rejection_output_is_detected() {
        assert!(output_mentions_secure_boot_key_rejection(
            "modprobe: ERROR: could not insert 'ov02c10': Key was rejected by service"
        ));
        assert!(!output_mentions_secure_boot_key_rejection(
            "modprobe: FATAL: Module ov02c10 not found"
        ));
    }

    #[test]
    fn secure_boot_password_validation_requires_matching_values() {
        assert_eq!(
            secure_boot_password_error("abc", "xyz"),
            Some("As duas senhas do MOK precisam ser idênticas.".into())
        );
    }

    #[test]
    fn secure_boot_password_validation_accepts_matching_values() {
        assert_eq!(secure_boot_password_error("abc", "abc"), None);
    }
}
