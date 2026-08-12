use galaxybook_setup::tr_mark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionKey {
    InstallMainSupport,
    InstallCamera,
    InstallSoundApp,
    RepairDriver,
    EnableCameraModule,
    ForceDriverPriority,
    EnableBrowserCamera,
    EnableSpeakers,
    PrepareSecureBootKey,
    RepairFingerprintStack,
    EnableFingerprintAuth,
    OpenFingerprintSettings,
    RepairNvidia,
    SetBalancedProfile,
    ApplyClipboardProfile,
    ApplyGsconnectProfile,
    ApplyDesktopIconsProfile,
    ApplyDockProfile,
    Reboot,
    OpenCamera,
    OpenSoundApp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActionMetadata {
    pub(crate) title: &'static str,
    pub(crate) subtitle: &'static str,
}

pub(crate) fn action_metadata(key: ActionKey) -> ActionMetadata {
    match key {
        ActionKey::InstallMainSupport => ActionMetadata {
            title: tr_mark("Instalar suporte principal"),
            subtitle: tr_mark(
                "Instala os pacotes e só conclui depois de validar OV02C10 e MAX98390 para todos os kernels instalados.",
            ),
        },
        ActionKey::InstallCamera => ActionMetadata {
            title: tr_mark("Instalar suporte da câmera"),
            subtitle: tr_mark(
                "Instala o app, gera o OV02C10 por kernel e valida RPM, depmod e initramfs antes de concluir.",
            ),
        },
        ActionKey::InstallSoundApp => ActionMetadata {
            title: tr_mark("Instalar Galaxy Book Sound"),
            subtitle: tr_mark(
                "Instala o painel de som do Galaxy Book para equalizador, perfis e Atmos compatível via PipeWire.",
            ),
        },
        ActionKey::RepairDriver => ActionMetadata {
            title: tr_mark("Reparar o driver"),
            subtitle: tr_mark(
                "Atualiza e valida o OV02C10 empacotado em todos os kernels instalados, sem trocar ao vivo um módulo ocupado.",
            ),
        },
        ActionKey::EnableCameraModule => ActionMetadata {
            title: tr_mark("Habilitar driver da câmera"),
            subtitle: tr_mark(
                "Reconcilia o OV02C10 por kernel e finaliza cada initramfs; um módulo já ocupado só muda após reboot normal.",
            ),
        },
        ActionKey::ForceDriverPriority => ActionMetadata {
            title: tr_mark("Ajustar prioridade do driver"),
            subtitle: tr_mark(
                "Migra apenas legados reconhecidos, exige payload com dono RPM e valida a cópia correta dentro de cada initramfs.",
            ),
        },
        ActionKey::EnableBrowserCamera => ActionMetadata {
            title: tr_mark("Ativar câmera para navegador"),
            subtitle: tr_mark(
                "Expõe a câmera interna como webcam V4L2 para apps WebRTC, desativa os nós crus do IPU6 só no PipeWire e preserva o acesso direto do libcamera; configurações administrativas conflitantes são preservadas e interrompem a ação.",
            ),
        },
        ActionKey::EnableSpeakers => ActionMetadata {
            title: tr_mark("Ativar alto-falantes internos"),
            subtitle: tr_mark(
                "Gera os dois módulos MAX98390 empacotados para todos os kernels, valida o dono RPM e exige os quatro vínculos I2C.",
            ),
        },
        ActionKey::PrepareSecureBootKey => ActionMetadata {
            title: tr_mark("Preparar chave do Secure Boot"),
            subtitle: tr_mark(
                "Gera a chave local do akmods quando necessário, cria o pedido de importação no MOK e deixa o reboot pronto para concluir o Enroll MOK no boot.",
            ),
        },
        ActionKey::RepairFingerprintStack => ActionMetadata {
            title: tr_mark("Reinstalar stack de fingerprint"),
            subtitle: tr_mark(
                "Reinstala fprintd e libfprint e reinicia o daemon para recuperar o leitor quando o sensor aparece, mas o cadastro ou a autenticação falham.",
            ),
        },
        ActionKey::EnableFingerprintAuth => ActionMetadata {
            title: tr_mark("Ativar login por digital"),
            subtitle: tr_mark(
                "Habilita with-fingerprint no authselect, aplica as mudanças do perfil PAM atual e devolve o estado final do authselect.",
            ),
        },
        ActionKey::OpenFingerprintSettings => ActionMetadata {
            title: tr_mark("Abrir cadastro de digitais"),
            subtitle: tr_mark(
                "Abre as configurações de usuários do GNOME para cadastrar, revisar ou refazer as digitais do usuário atual.",
            ),
        },
        ActionKey::RepairNvidia => ActionMetadata {
            title: tr_mark("Reparar suporte NVIDIA"),
            subtitle: tr_mark(
                "Instala ou reconstrói o akmod-nvidia para o kernel atual. O nvidia-smi permanece opcional.",
            ),
        },
        ActionKey::SetBalancedProfile => ActionMetadata {
            title: tr_mark("Definir perfil balanceado"),
            subtitle: tr_mark(
                "Aplica o perfil balanced da plataforma para uso geral, equilibrando ventoinha, temperatura e desempenho.",
            ),
        },
        ActionKey::ApplyClipboardProfile => ActionMetadata {
            title: tr_mark("Ativar histórico da área de transferência"),
            subtitle: tr_mark(
                "Instala e habilita a extensão de clipboard usada como perfil recomendado neste notebook.",
            ),
        },
        ActionKey::ApplyGsconnectProfile => ActionMetadata {
            title: tr_mark("Ativar GSConnect"),
            subtitle: tr_mark(
                "Instala o GSConnect, habilita a extensão e reaplica o perfil usado neste notebook para integração com o celular.",
            ),
        },
        ActionKey::ApplyDesktopIconsProfile => ActionMetadata {
            title: tr_mark("Ativar ícones na área de trabalho"),
            subtitle: tr_mark(
                "Instala o Desktop Icons NG, habilita a extensão e reaplica o perfil usado neste notebook.",
            ),
        },
        ActionKey::ApplyDockProfile => ActionMetadata {
            title: tr_mark("Aplicar perfil da dock"),
            subtitle: tr_mark(
                "Ativa o Dash to Dock e reaplica a dock inferior auto-ocultável, com clique ciclando janelas, prévia, lixeira e unidades montadas visíveis.",
            ),
        },
        ActionKey::Reboot => ActionMetadata {
            title: tr_mark("Reiniciar o sistema"),
            subtitle: tr_mark(
                "Reinicia normalmente, respeitando inhibitors do sistema e builds de módulos ainda em andamento.",
            ),
        },
        ActionKey::OpenCamera => ActionMetadata {
            title: tr_mark("Abrir Galaxy Book Câmera"),
            subtitle: tr_mark(
                "Abre o aplicativo final da câmera quando ele estiver instalado no sistema.",
            ),
        },
        ActionKey::OpenSoundApp => ActionMetadata {
            title: tr_mark("Abrir Galaxy Book Sound"),
            subtitle: tr_mark(
                "Abre o painel de som com equalizador, perfis e Atmos compatível quando ele estiver instalado no sistema.",
            ),
        },
    }
}

pub(crate) fn action_icon_name(key: ActionKey) -> &'static str {
    match key {
        ActionKey::InstallMainSupport => "system-software-install-symbolic",
        ActionKey::InstallCamera => "camera-web-symbolic",
        ActionKey::InstallSoundApp => "audio-speakers-symbolic",
        ActionKey::RepairDriver => "view-refresh-symbolic",
        ActionKey::EnableCameraModule => "camera-web-symbolic",
        ActionKey::ForceDriverPriority => "preferences-system-symbolic",
        ActionKey::EnableBrowserCamera => "web-browser-symbolic",
        ActionKey::EnableSpeakers => "audio-speakers-symbolic",
        ActionKey::PrepareSecureBootKey => "dialog-password-symbolic",
        ActionKey::RepairFingerprintStack => "view-refresh-symbolic",
        ActionKey::EnableFingerprintAuth => "system-lock-screen-symbolic",
        ActionKey::OpenFingerprintSettings => "preferences-system-symbolic",
        ActionKey::RepairNvidia => "video-display-symbolic",
        ActionKey::SetBalancedProfile => "power-profile-balanced-symbolic",
        ActionKey::ApplyClipboardProfile => "edit-paste-symbolic",
        ActionKey::ApplyGsconnectProfile => "phone-symbolic",
        ActionKey::ApplyDesktopIconsProfile => "user-desktop-symbolic",
        ActionKey::ApplyDockProfile => "view-grid-symbolic",
        ActionKey::Reboot => "system-reboot-symbolic",
        ActionKey::OpenCamera => "camera-web-symbolic",
        ActionKey::OpenSoundApp => "audio-speakers-symbolic",
    }
}

pub(crate) fn dedupe_action_keys(actions: &[ActionKey]) -> Vec<ActionKey> {
    let mut deduped = Vec::with_capacity(actions.len());
    for action in actions {
        if !deduped.contains(action) {
            deduped.push(*action);
        }
    }
    deduped
}
