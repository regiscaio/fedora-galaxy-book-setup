use galaxybook_setup::tr_mark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionKey {
    InstallMainSupport,
    InstallCamera,
    RepairDriver,
    EnableCameraModule,
    ForceDriverPriority,
    RestoreIntelIpu6,
    EnableBrowserCamera,
    EnableSpeakers,
    RepairNvidia,
    SetBalancedProfile,
    Reboot,
    OpenCamera,
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
