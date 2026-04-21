use galaxybook_setup::tr_mark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionKey {
    InstallMainSupport,
    InstallCamera,
    InstallSoundApp,
    RepairDriver,
    EnableCameraModule,
    ForceDriverPriority,
    RestoreIntelIpu6,
    EnableBrowserCamera,
    EnableSpeakers,
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
                "Instala Galaxy Book Câmera, driver OV02C10 e suporte MAX98390 para começar pelo setup sem depender de instalação manual dos outros pacotes.",
            ),
        },
        ActionKey::InstallCamera => ActionMetadata {
            title: tr_mark("Instalar suporte da câmera"),
            subtitle: tr_mark(
                "Instala o driver corrigido e o aplicativo Galaxy Book Câmera usando privilégios administrativos.",
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
                "Reconstrói o módulo com akmods para o kernel atual e atualiza a árvore de módulos.",
            ),
        },
        ActionKey::EnableCameraModule => ActionMetadata {
            title: tr_mark("Habilitar driver da câmera"),
            subtitle: tr_mark(
                "Garante o carregamento do ov02c10 no boot, ajusta o softdep do IPU6 e carrega o módulo agora no kernel.",
            ),
        },
        ActionKey::ForceDriverPriority => ActionMetadata {
            title: tr_mark("Ajustar prioridade do driver"),
            subtitle: tr_mark(
                "Compila o módulo corrigido, assina quando o Secure Boot estiver ativo e o instala em /lib/modules/.../updates sem compressão incompatível.",
            ),
        },
        ActionKey::RestoreIntelIpu6 => ActionMetadata {
            title: tr_mark("Restaurar stack Intel IPU6"),
            subtitle: tr_mark(
                "Remove o override manual em /updates, reinstala o stack Intel empacotado e volta ao caminho que já funcionava com a câmera do sistema.",
            ),
        },
        ActionKey::EnableBrowserCamera => ActionMetadata {
            title: tr_mark("Ativar câmera para navegador"),
            subtitle: tr_mark(
                "Expõe a câmera interna como webcam V4L2 para Meet, Discord, Teams e outros apps WebRTC usando o bridge do sistema e oculta os nós crus do IPU6 no PipeWire.",
            ),
        },
        ActionKey::EnableSpeakers => ActionMetadata {
            title: tr_mark("Ativar alto-falantes internos"),
            subtitle: tr_mark(
                "Instala o suporte MAX98390, reconstrói os módulos dos amplificadores, instala manualmente o driver no kernel atual quando necessário e habilita o serviço de I2C usado pelos alto-falantes internos.",
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
                "Aplica mudanças pendentes do driver e reinicia a sessão inteira do notebook.",
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

pub(crate) fn dedupe_action_keys(actions: &[ActionKey]) -> Vec<ActionKey> {
    let mut deduped = Vec::with_capacity(actions.len());
    for action in actions {
        if !deduped.contains(action) {
            deduped.push(*action);
        }
    }
    deduped
}
