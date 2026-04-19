# Galaxy Book Setup

<p align="center">
  <img src="assets/galaxybook-setup.svg" alt="Ícone do Galaxy Book Setup" width="112">
</p>

`Galaxy Book Setup` é um auxiliar de instalação e diagnóstico para notebooks
Samsung Galaxy Book no Fedora. A proposta do app é organizar fluxos de
configuração que hoje normalmente acabam espalhados em terminal, logs, pacotes
RPM e validações manuais.

O foco inicial é a **câmera interna** do Galaxy Book4 Ultra, mas o repositório
foi estruturado para crescer para outros módulos, como áudio,
fingerprint e checagens gerais do sistema.

## Escopo

Este app não substitui:

- o driver do kernel;
- o app final de câmera;
- ferramentas de baixo nível como `akmods`, `modinfo` ou `journalctl`.

O papel dele é funcionar como um **assistente de instalação e validação**,
mostrando o estado atual da máquina e organizando os próximos passos.

## Relação com os outros repositórios

Este projeto trabalha junto com:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>

Responsabilidades:

- `fedora-galaxy-book-ov02c10`: módulo `ov02c10` empacotado para Fedora;
- `fedora-galaxy-book-camera`: app de uso diário da câmera;
- `fedora-galaxy-book-setup`: assistente de instalação, diagnóstico e fluxo.

## Recursos atuais

A versão atual do app já organiza a interface em áreas bem definidas:

- `Sistema`: resumo do notebook, Fedora, kernel e Secure Boot já visível na tela inicial;
- `Diagnósticos`: checklist geral com o estado da câmera, da webcam para navegador, da GPU e de integrações do desktop;
- `Ações rápidas`: instalação, reparo e ajuste de prioridade do driver, ativação da webcam para navegador, fluxo NVIDIA, perfil balanceado, reboot e abertura do app da câmera;
- `Próximos módulos`: espaço reservado para áudio, fingerprint e outros fluxos.

Dentro de `Diagnósticos`, cada linha de checklist também leva para uma
subseção de **ações sugeridas**. Isso permite abrir correções e validações mais
relevantes para o item selecionado sem perder a página geral de `Ações rápidas`,
que continua disponível sem filtragem.

O app também passa a expor um resumo de alertas e erros via notificações do
desktop. Em docks e extensões que suportam contador no launcher, o ícone pode
mostrar a quantidade total de itens com `Atenção` ou `Erro` nos diagnósticos.

O checklist de `Diagnósticos` cobre hoje:

- pacotes principais da câmera;
- geração do driver no boot via `akmods`;
- origem do módulo `ov02c10` ativo;
- detecção da câmera no caminho direto do `libcamera` usado pelo `Galaxy Book Câmera`;
- bridge V4L2 para navegadores e comunicadores, com foco em Meet, Discord, Teams e apps WebRTC;
- erros conhecidos do boot;
- estado do driver NVIDIA e observação de que `nvidia-smi` é opcional;
- perfil de uso da plataforma, com destaque para `balanced` como padrão recomendado de ventoinha, temperatura e desempenho;
- extensões do GNOME como histórico da área de transferência, GSConnect e ícones na área de trabalho.

As ações rápidas não apenas copiam comandos: elas executam os fluxos principais
pela própria interface, usando privilégio administrativo quando necessário.

Hoje, as ações disponíveis incluem:

- instalar o conjunto principal da câmera;
- reconstruir o driver com `akmods`;
- habilitar o carregamento do `ov02c10` no boot e carregar o módulo imediatamente;
- forçar a prioridade do driver corrigido em `updates/`, com assinatura para Secure Boot quando necessário e sem compressão incompatível do módulo;
- restaurar o stack Intel IPU6 empacotado quando o caminho direto do `Galaxy Book Câmera` deixa de enxergar o sensor, mesmo com a câmera exposta no sistema;
- ativar a câmera para navegador via `icamerasrc`, `v4l2-relayd` e `v4l2loopback`, expondo uma webcam V4L2 compatível com apps WebRTC;
- instalar ou reparar o suporte NVIDIA;
- aplicar o perfil `balanced` da plataforma;
- reiniciar o sistema;
- abrir o `Galaxy Book Câmera`.

## Instalação para usuários

O formato alvo é **RPM**, preferencialmente distribuído via **COPR**.

Enquanto isso, o projeto pode ser empacotado localmente:

```bash
make rpm
```

Depois, o RPM pode ser instalado com:

```bash
sudo dnf install /caminho/para/galaxybook-setup-*.rpm
```

## Build

Dependências de build no Fedora:

```bash
sudo dnf install cargo rust pkgconf-pkg-config gtk4-devel libadwaita-devel
```

Se o host não tiver o toolchain completo, o `Makefile` usa um container rootless
com `podman`.

Comandos principais:

```bash
make build
make test
make dist
make srpm
make rpm
```

Para instalar o launcher local de desenvolvimento:

```bash
make install-local
```

## Empacotamento

Arquivos relevantes:

- spec RPM: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- metadados AppStream: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

O RPM usa `Recommends` para apontar os pacotes mais importantes do fluxo:

- `akmod-galaxybook-ov02c10`
- `galaxybook-camera`

Isso permite que o app seja instalado mesmo antes do setup completo da câmera,
o que é desejável para um auxiliar de instalação.

## Roadmap

Módulos planejados para próximas etapas:

- áudio;
- fingerprint;
- checagens gerais de compatibilidade do Galaxy Book com Fedora;
- novos fluxos assistidos para integrações do ambiente GNOME e periféricos do notebook.
