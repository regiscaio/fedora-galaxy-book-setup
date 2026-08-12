use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use galaxybook_setup::{
    AKMODS_PUBLIC_KEY_PATH, CAMERA_APP_DESKTOP_ID, OPEN_FINGERPRINT_SETTINGS_COMMAND,
    REBOOT_COMMAND, SOUND_APP_DESKTOP_ID, SetupSnapshot, tr, trf,
};

use crate::actions::ActionKey;
use crate::system::{
    execute_privileged_shell_command, execute_privileged_shell_command_with_input,
    execute_user_shell_command,
};
use crate::ui::DiagnosticKey;
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

pub(crate) const SETUP_UPDATE_PACKAGES: &[&str] = &[
    "galaxybook-setup",
    "galaxybook-camera",
    "galaxybook-ov02c10-kmod-common",
    "akmod-galaxybook-ov02c10",
    "kmod-galaxybook-ov02c10",
    "galaxybook-sound",
    "galaxybook-max98390-kmod-common",
    "akmod-galaxybook-max98390",
    "kmod-galaxybook-max98390",
];

pub(crate) fn action_is_snapshot_eligible(key: ActionKey, snapshot: &SetupSnapshot) -> bool {
    match key {
        ActionKey::Reboot => snapshot.reboot_ready,
        ActionKey::OpenCamera => snapshot.camera_app_installed,
        ActionKey::OpenSoundApp => snapshot.sound_app_installed,
        _ => true,
    }
}

pub(crate) fn suggested_action_is_snapshot_eligible(
    key: ActionKey,
    diagnostic: DiagnosticKey,
    snapshot: &SetupSnapshot,
) -> bool {
    let driver_diagnostic = matches!(
        diagnostic,
        DiagnosticKey::Akmods
            | DiagnosticKey::Module
            | DiagnosticKey::Libcamera
            | DiagnosticKey::Boot
            | DiagnosticKey::Speakers
    );
    action_is_snapshot_eligible(key, snapshot)
        && (key != ActionKey::Reboot || !driver_diagnostic || snapshot.driver_reboot_ready)
}

fn update_button_tooltip(packages: &[String]) -> String {
    trf(
        "Baixar e instalar atualizações: {packages}",
        &[("packages", packages.join(", "))],
    )
}

#[derive(Debug, PartialEq, Eq)]
enum UpdateRefreshPresentation {
    NoUpdates,
    Updates(Vec<String>),
    QueryFailed,
}

fn update_refresh_presentation(result: Result<Vec<String>, String>) -> UpdateRefreshPresentation {
    match result {
        Ok(packages) if packages.is_empty() => UpdateRefreshPresentation::NoUpdates,
        Ok(packages) => UpdateRefreshPresentation::Updates(packages),
        Err(_) => UpdateRefreshPresentation::QueryFailed,
    }
}

fn update_refresh_is_eligible(presentation: &UpdateRefreshPresentation) -> bool {
    matches!(presentation, UpdateRefreshPresentation::Updates(_))
}

pub(super) fn update_button_should_be_sensitive(
    actions_allowed: bool,
    update_query_eligible: bool,
) -> bool {
    actions_allowed && update_query_eligible
}

fn upgrade_installed_packages_command(packages: &[&str]) -> String {
    upgrade_installed_packages_command_with_helper(
        packages,
        "/usr/libexec/galaxybook-setup/reconcile-kmods",
    )
}

fn upgrade_installed_packages_command_with_helper(
    packages: &[&str],
    reconcile_helper: &str,
) -> String {
    format!(
        r#"set -euo pipefail
	packages=({packages})
	installed=()
	rpm_inventory=""
	load_rpm_inventory() {{
	  local error_file inventory_file status
	  error_file="$(mktemp /var/tmp/galaxybook-setup-update-rpm.XXXXXX)"
	  inventory_file="$(mktemp /var/tmp/galaxybook-setup-update-rpm.XXXXXX)"
	  if LC_ALL=C rpm -qa --qf '%{{NAME}}\n' >"$inventory_file" 2>"$error_file"; then
	    status=0
	  else
	    status=$?
	  fi
	  if (( status != 0 )) \
	    || [[ -s "$error_file" || ! -s "$inventory_file" ]] \
	    || ! tail -c 1 -- "$inventory_file" | grep -qx ''; then
	    echo "O RPM não conseguiu produzir um inventário conclusivo dos pacotes instalados. A atualização foi interrompida sem assumir que pacotes estão ausentes." >&2
	    rm -f -- "$error_file" "$inventory_file"
	    return 1
	  fi
	  if ! LC_ALL=C awk '
	    $0 !~ /^[[:alnum:]][[:alnum:]+._-]*$/ {{ invalid = 1 }}
	    END {{ exit invalid ? 1 : 0 }}
	  ' "$inventory_file"; then
	    echo "O RPM retornou um nome de pacote malformado. A atualização foi interrompida sem usar um inventário parcial." >&2
	    rm -f -- "$error_file" "$inventory_file"
	    return 1
	  fi
	  rpm_inventory="$(<"$inventory_file")"
	  rm -f -- "$error_file" "$inventory_file"
	}}
	package_is_installed() {{
	  local expected="$1" candidate
	  while IFS= read -r candidate; do
	    [[ "$candidate" == "$expected" ]] && return 0
	  done <<< "$rpm_inventory"
	  return 1
	}}
	count_installed_family() {{
	  local count=0 package
	  for package in "$@"; do
	    if package_is_installed "$package"; then
	      count=$((count + 1))
	    fi
	  done
	  printf '%s\n' "$count"
	}}
	load_rpm_inventory
	for package in "${{packages[@]}}"; do
	  if package_is_installed "$package"; then
	    installed+=("$package")
	  fi
done
if [ "${{#installed[@]}}" -eq 0 ]; then
  echo "Nenhum pacote do Galaxy Book está instalado para atualizar."
  exit 0
fi
ov_count="$(count_installed_family galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10)"
max_count="$(count_installed_family galaxybook-max98390-kmod-common akmod-galaxybook-max98390 kmod-galaxybook-max98390)"
if (( ov_count > 0 && ov_count < 3 )); then
  echo "A família OV02C10 está parcialmente instalada. Use 'Instalar suporte da câmera' ou 'Reparar o driver' para completar os três pacotes antes de atualizar." >&2
  exit 1
fi
if (( max_count > 0 && max_count < 3 )); then
  echo "A família MAX98390 está parcialmente instalada. Use 'Ativar alto-falantes internos' para completar os três pacotes antes de atualizar." >&2
  exit 1
fi
	dnf --refresh upgrade -y "${{installed[@]}}"
	load_rpm_inventory
	ov_count="$(count_installed_family galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10)"
max_count="$(count_installed_family galaxybook-max98390-kmod-common akmod-galaxybook-max98390 kmod-galaxybook-max98390)"
if (( ov_count > 0 && ov_count < 3 )); then
  echo "A atualização deixou a família OV02C10 parcialmente instalada. Complete os três pacotes pela ação de câmera antes de reiniciar." >&2
  exit 1
fi
if (( max_count > 0 && max_count < 3 )); then
  echo "A atualização deixou a família MAX98390 parcialmente instalada. Complete os três pacotes pela ação de alto-falantes antes de reiniciar." >&2
  exit 1
fi
case "$ov_count:$max_count" in
  3:3) {reconcile_helper} all ;;
  3:0) {reconcile_helper} ov02c10 ;;
  0:3) {reconcile_helper} max98390 ;;
  0:0) echo "Atualizações concluídas; nenhuma família completa de driver estava instalada para reconciliar." ;;
esac
"#,
        packages = packages.join(" "),
        reconcile_helper = reconcile_helper,
    )
}

fn output_mentions_secure_boot_key_rejection(output: &str) -> bool {
    output.contains("Key was rejected by service") || output.contains("key was rejected by service")
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

    let remediation = trf(
        "O kernel rejeitou o módulo porque o Secure Boot não aceitou sua assinatura. O aplicativo sem privilégios pode não conseguir ler {path} porque o diretório do akmods é protegido; isso não significa que a chave esteja ausente. Use a ação 'Preparar chave do Secure Boot' para validar a chave com privilégios, criar o pedido correto no MOK quando necessário e, depois que os payloads estiverem prontos, reinicie normalmente para concluir o Enroll MOK.",
        &[("path", AKMODS_PUBLIC_KEY_PATH.to_string())],
    );

    format!("{remediation}\n\n---\n\n{output_text}")
}

fn secure_boot_password_error(password: &str, confirmation: &str) -> Option<String> {
    if password.trim().is_empty() || confirmation.trim().is_empty() {
        return Some(tr(
            "Defina e confirme a senha temporária do MOK para continuar.",
        ));
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
    pub(crate) fn prepare_update_refresh(&self) {
        self.update_query_eligible.set(false);
        self.update_button.set_visible(false);
        self.update_button.set_sensitive(false);
    }

    pub(crate) fn apply_update_refresh(&self, result: Result<Vec<String>, String>) {
        let presentation = update_refresh_presentation(result);
        self.update_query_eligible
            .set(update_refresh_is_eligible(&presentation));

        match presentation {
            UpdateRefreshPresentation::NoUpdates => {
                self.update_button.set_visible(false);
                self.update_button.set_sensitive(false);
                self.update_button.set_tooltip_text(None);
            }
            UpdateRefreshPresentation::Updates(packages) => {
                self.update_button.set_visible(true);
                self.update_button
                    .set_sensitive(update_button_should_be_sensitive(
                        self.operation_is_idle(),
                        self.update_query_eligible.get(),
                    ));
                self.update_button
                    .set_tooltip_text(Some(&update_button_tooltip(&packages)));
            }
            UpdateRefreshPresentation::QueryFailed => {
                let message = tr(
                    "Falha ao consultar atualizações. Verifique o RPM, a conexão e os repositórios; depois atualize o diagnóstico.",
                );
                self.update_button.set_visible(true);
                self.update_button.set_sensitive(false);
                self.update_button.set_tooltip_text(Some(&message));
                self.toast_overlay.add_toast(adw::Toast::new(&message));
            }
        }
    }

    pub(crate) fn install_updates(&self) {
        if !self.operation_is_idle()
            || !self.update_query_eligible.get()
            || !self.update_button.is_visible()
            || !self.update_button.is_sensitive()
        {
            return;
        }

        self.run_privileged_command(
            &tr("Atualizar pacotes"),
            upgrade_installed_packages_command(SETUP_UPDATE_PACKAGES),
            &tr("Atualização concluída. Revise a saída da ação e atualize o diagnóstico; payloads só são declarados prontos quando a reconciliação da família instalada termina sem erros."),
            true,
        );
    }

    pub(super) fn invoke_action(&self, key: ActionKey) {
        if !self.operation_is_idle()
            || !self
                .snapshot
                .borrow()
                .as_ref()
                .is_some_and(|snapshot| action_is_snapshot_eligible(key, snapshot))
        {
            return;
        }

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
                    &tr("Pacotes e payloads por kernel instalados e validados. Atualize o diagnóstico e reinicie normalmente para trocar módulos já ocupados."),
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
                    &tr("OV02C10 e initramfs validados em todos os kernels instalados. Reinicie normalmente para carregar o driver sem trocar ao vivo um módulo ocupado."),
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
                    &tr("OV02C10 validado em todos os kernels instalados. Reinicie normalmente para trocar qualquer módulo já ocupado."),
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
                    &tr("OV02C10 e initramfs validados em todos os kernels instalados. Reinicie normalmente; o módulo ocupado não foi trocado ao vivo."),
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
                    &tr("Payload, prioridade e initramfs do OV02C10 validados para todos os kernels. Reinicie normalmente antes de validar o novo grafo IPU6."),
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
                    &tr("Payloads MAX98390 por kernel e serviço I2C validados. Reinicie normalmente se o kernel atual ainda mantiver módulos antigos carregados."),
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
                    .map(|snapshot| snapshot.enable_fingerprint_auth_command.clone())
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
                    .map(|snapshot| snapshot.open_fingerprint_settings_command.clone())
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
                    .map(|snapshot| snapshot.apply_desktop_icons_profile_command.clone())
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
                    if let Err(error) = app.launch(&[], None::<&gio::AppLaunchContext>) {
                        self.toast_overlay.add_toast(adw::Toast::new(&trf(
                            "Falha ao abrir o app da câmera: {error}",
                            &[("error", error.to_string())],
                        )));
                    }
                } else {
                    self.toast_overlay.add_toast(adw::Toast::new(&tr(
                        "O Galaxy Book Câmera não foi encontrado no sistema.",
                    )));
                }
            }
            ActionKey::OpenSoundApp => {
                if let Some(app) = gio::DesktopAppInfo::new(SOUND_APP_DESKTOP_ID) {
                    if let Err(error) = app.launch(&[], None::<&gio::AppLaunchContext>) {
                        self.toast_overlay.add_toast(adw::Toast::new(&trf(
                            "Falha ao abrir o painel de som: {error}",
                            &[("error", error.to_string())],
                        )));
                    }
                } else {
                    self.toast_overlay.add_toast(adw::Toast::new(&tr(
                        "O Galaxy Book Sound não foi encontrado no sistema.",
                    )));
                }
            }
        }
    }

    pub(super) fn invoke_suggested_action(&self, key: ActionKey, diagnostic: DiagnosticKey) {
        if !self.operation_is_idle()
            || !self.snapshot.borrow().as_ref().is_some_and(|snapshot| {
                suggested_action_is_snapshot_eligible(key, diagnostic, snapshot)
            })
        {
            return;
        }
        self.invoke_action(key);
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
        if command.trim().is_empty() || !self.operation_is_idle() {
            return;
        }

        *self.action_running.borrow_mut() = true;
        self.refresh_button.set_sensitive(false);
        self.set_action_buttons_sensitive(false);
        self.toast_overlay.add_toast(adw::Toast::new(&trf(
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
                    Some(stdin) => execute_privileged_shell_command_with_input(&command, &stdin),
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

                    if result.success {
                        this.toast_overlay
                            .add_toast(adw::Toast::new(&result.success_message));
                        if result.refresh_after {
                            this.refresh();
                        } else {
                            this.refresh_button.set_sensitive(true);
                            this.set_action_buttons_sensitive(true);
                        }
                    } else {
                        this.refresh_button.set_sensitive(true);
                        this.set_action_buttons_sensitive(true);
                        this.present_command_result_dialog(&result.title, &result.output);
                    }

                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    *this.action_running.borrow_mut() = false;
                    this.refresh_button.set_sensitive(true);
                    this.set_action_buttons_sensitive(true);
                    this.toast_overlay.add_toast(adw::Toast::new(&tr(
                        "Falha ao acompanhar a ação solicitada.",
                    )));
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn present_secure_boot_password_dialog(&self) {
        if !self.operation_is_idle() {
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

        let password_entry = gtk::PasswordEntry::builder().show_peek_icon(true).build();
        content.append(&password_entry);

        let confirmation_label = gtk::Label::builder()
            .label(tr("Confirmar senha"))
            .xalign(0.0)
            .build();
        content.append(&confirmation_label);

        let confirmation_entry = gtk::PasswordEntry::builder().show_peek_icon(true).build();
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
            let password_entry = password_entry.clone();
            let confirmation_entry = confirmation_entry.clone();
            move |_| {
                password_entry.set_text("");
                confirmation_entry.set_text("");
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

                let stdin_data = format!("{password}\n{confirmation}\n");
                password_entry.set_text("");
                confirmation_entry.set_text("");
                dialog.close();
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
    use std::io::Write;

    fn run_upgrade_fixture(installed_packages: &str) -> std::process::Output {
        run_upgrade_fixture_with_post_state_and_rpm_status(installed_packages, "", 0)
    }

    fn run_upgrade_fixture_with_post_state(
        installed_packages: &str,
        post_dnf_packages: &str,
    ) -> std::process::Output {
        run_upgrade_fixture_with_post_state_and_rpm_status(installed_packages, post_dnf_packages, 0)
    }

    fn run_upgrade_fixture_with_post_state_and_rpm_status(
        installed_packages: &str,
        post_dnf_packages: &str,
        rpm_status: i32,
    ) -> std::process::Output {
        run_upgrade_fixture_with_rpm_shape(
            installed_packages,
            post_dnf_packages,
            rpm_status,
            false,
            "",
        )
    }

    fn run_upgrade_fixture_with_rpm_shape(
        installed_packages: &str,
        post_dnf_packages: &str,
        rpm_status: i32,
        raw_inventory: bool,
        rpm_stderr: &str,
    ) -> std::process::Output {
        let script = upgrade_installed_packages_command_with_helper(
            SETUP_UPDATE_PACKAGES,
            "record_reconcile",
        );
        let harness = format!(
            r#"rpm() {{
  [[ "$1" == -qa ]] || return 64
  printf '%s' "$RPM_STDERR" >&2
  if [[ "$RPM_RAW_INVENTORY" == 1 ]]; then
    printf '%s' "$INSTALLED_PACKAGES"
  else
    tr ' ' '\n' <<< "$INSTALLED_PACKAGES" | sed '/^$/d'
  fi
  return "$RPM_STATUS"
}}
dnf() {{
  printf 'DNF:%s\n' "$*"
  if [[ -n "$POST_DNF_PACKAGES" ]]; then
    INSTALLED_PACKAGES="$POST_DNF_PACKAGES"
  fi
}}
record_reconcile() {{ printf 'RECONCILE:%s\n' "$*"; }}
{script}
"#
        );
        let mut child = std::process::Command::new("bash")
            .args(["--noprofile", "--norc", "-s"])
            .env("INSTALLED_PACKAGES", installed_packages)
            .env("POST_DNF_PACKAGES", post_dnf_packages)
            .env("RPM_STATUS", rpm_status.to_string())
            .env("RPM_RAW_INVENTORY", if raw_inventory { "1" } else { "0" })
            .env("RPM_STDERR", rpm_stderr)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("não foi possível iniciar fixture bash");
        child
            .stdin
            .as_mut()
            .expect("stdin da fixture indisponível")
            .write_all(harness.as_bytes())
            .expect("não foi possível enviar fixture ao bash");
        child.wait_with_output().expect("fixture bash não terminou")
    }

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

    #[test]
    fn package_upgrade_command_has_valid_bash_syntax() {
        let script = upgrade_installed_packages_command(SETUP_UPDATE_PACKAGES);
        let mut child = std::process::Command::new("bash")
            .args(["--noprofile", "--norc", "-n"])
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("não foi possível iniciar bash");
        child
            .stdin
            .as_mut()
            .expect("stdin do bash indisponível")
            .write_all(script.as_bytes())
            .expect("não foi possível enviar o comando ao bash");
        let output = child.wait_with_output().expect("bash -n não terminou");
        assert!(
            output.status.success(),
            "sintaxe shell inválida: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn package_upgrade_without_driver_family_does_not_claim_reconciliation() {
        let output = run_upgrade_fixture("galaxybook-camera");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("nenhuma família completa"));
        assert!(!stdout.contains("RECONCILE:"));
    }

    #[test]
    fn package_upgrade_reconciles_a_complete_driver_family() {
        let output = run_upgrade_fixture(
            "galaxybook-camera galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10",
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("RECONCILE:ov02c10"));
    }

    #[test]
    fn package_upgrade_reconciles_a_driver_family_added_by_weak_dependencies() {
        let output = run_upgrade_fixture_with_post_state(
            "galaxybook-setup",
            "galaxybook-setup galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10",
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("RECONCILE:ov02c10"));
    }

    #[test]
    fn package_updater_tracks_the_versioned_camera_and_sound_applications() {
        assert!(SETUP_UPDATE_PACKAGES.contains(&"galaxybook-camera"));
        assert!(SETUP_UPDATE_PACKAGES.contains(&"galaxybook-sound"));
    }

    #[test]
    fn package_upgrade_rejects_a_partial_driver_family() {
        let output = run_upgrade_fixture("galaxybook-camera galaxybook-ov02c10-kmod-common");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("parcialmente instalada"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DNF:"));
    }

    #[test]
    fn package_upgrade_fails_closed_when_rpm_inventory_is_inconclusive() {
        let output = run_upgrade_fixture_with_post_state_and_rpm_status("galaxybook-camera", "", 1);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("inventário conclusivo"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DNF:"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("RECONCILE:"));
    }

    #[test]
    fn package_upgrade_rejects_empty_truncated_malformed_or_warning_inventory() {
        for (inventory, raw, warning) in [
            ("", false, ""),
            ("galaxybook-camera", true, ""),
            ("invalid package\n", true, ""),
            ("galaxybook-camera\n", true, "rpm warning\n"),
        ] {
            let output = run_upgrade_fixture_with_rpm_shape(inventory, "", 0, raw, warning);
            assert!(
                !output.status.success(),
                "accepted inconclusive RPM inventory {inventory:?}",
            );
            assert!(!String::from_utf8_lossy(&output.stdout).contains("DNF:"));
            assert!(!String::from_utf8_lossy(&output.stdout).contains("RECONCILE:"));
        }
    }

    #[test]
    fn update_query_failure_is_not_presented_as_zero_updates() {
        assert_eq!(
            update_refresh_presentation(Ok(Vec::new())),
            UpdateRefreshPresentation::NoUpdates
        );
        assert_eq!(
            update_refresh_presentation(Ok(vec!["galaxybook-setup".into()])),
            UpdateRefreshPresentation::Updates(vec!["galaxybook-setup".into()])
        );
        assert_eq!(
            update_refresh_presentation(Err("rpmdb indisponível".into())),
            UpdateRefreshPresentation::QueryFailed
        );
    }

    #[test]
    fn update_query_eligibility_survives_global_button_state_changes() {
        let failed = update_refresh_presentation(Err("rpmdb indisponível".into()));
        let failed_eligible = update_refresh_is_eligible(&failed);
        assert!(!update_button_should_be_sensitive(false, failed_eligible));
        assert!(!update_button_should_be_sensitive(true, failed_eligible));

        let succeeded = update_refresh_presentation(Ok(vec!["galaxybook-setup".into()]));
        let succeeded_eligible = update_refresh_is_eligible(&succeeded);
        assert!(!update_button_should_be_sensitive(
            false,
            succeeded_eligible
        ));
        assert!(update_button_should_be_sensitive(true, succeeded_eligible));
    }
}
