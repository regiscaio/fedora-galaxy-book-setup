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
            title: "Instalar suporte principal",
            subtitle: "Instala Galaxy Book Câmera, driver OV02C10 e suporte MAX98390 para começar pelo setup sem depender de instalação manual dos outros pacotes.",
        },
        ActionKey::InstallCamera => ActionMetadata {
            title: "Instalar suporte da câmera",
            subtitle: "Instala o driver corrigido e o aplicativo Galaxy Book Câmera usando privilégios administrativos.",
        },
        ActionKey::RepairDriver => ActionMetadata {
            title: "Reparar o driver",
            subtitle: "Reconstrói o módulo com akmods para o kernel atual e atualiza a árvore de módulos.",
        },
        ActionKey::EnableCameraModule => ActionMetadata {
            title: "Habilitar driver da câmera",
            subtitle: "Garante o carregamento do ov02c10 no boot, ajusta o softdep do IPU6 e carrega o módulo agora no kernel.",
        },
        ActionKey::ForceDriverPriority => ActionMetadata {
            title: "Ajustar prioridade do driver",
            subtitle: "Compila o módulo corrigido, assina quando o Secure Boot estiver ativo e o instala em /lib/modules/.../updates sem compressão incompatível.",
        },
        ActionKey::RestoreIntelIpu6 => ActionMetadata {
            title: "Restaurar stack Intel IPU6",
            subtitle: "Remove o override manual em /updates, reinstala o stack Intel empacotado e volta ao caminho que já funcionava com a câmera do sistema.",
        },
        ActionKey::EnableBrowserCamera => ActionMetadata {
            title: "Ativar câmera para navegador",
            subtitle: "Expõe a câmera interna como webcam V4L2 para Meet, Discord, Teams e outros apps WebRTC usando o bridge do sistema e oculta os nós crus do IPU6 no PipeWire.",
        },
        ActionKey::EnableSpeakers => ActionMetadata {
            title: "Ativar alto-falantes internos",
            subtitle: "Instala o suporte MAX98390, reconstrói os módulos dos amplificadores, instala manualmente o driver no kernel atual quando necessário e habilita o serviço de I2C usado pelos alto-falantes internos.",
        },
        ActionKey::RepairNvidia => ActionMetadata {
            title: "Reparar suporte NVIDIA",
            subtitle: "Instala ou reconstrói o akmod-nvidia para o kernel atual. O nvidia-smi permanece opcional.",
        },
        ActionKey::SetBalancedProfile => ActionMetadata {
            title: "Definir perfil balanceado",
            subtitle: "Aplica o perfil balanced da plataforma para uso geral, equilibrando ventoinha, temperatura e desempenho.",
        },
        ActionKey::Reboot => ActionMetadata {
            title: "Reiniciar o sistema",
            subtitle: "Aplica mudanças pendentes do driver e reinicia a sessão inteira do notebook.",
        },
        ActionKey::OpenCamera => ActionMetadata {
            title: "Abrir Galaxy Book Câmera",
            subtitle: "Abre o aplicativo final da câmera quando ele estiver instalado no sistema.",
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
