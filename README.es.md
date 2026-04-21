<p align="center">
  <img src="assets/galaxybook-setup.svg" alt="Ícono de Galaxy Book Setup" width="112">
</p>

<h1 align="center">Galaxy Book Setup</h1>

<p align="center">
  <a href="README.md">🇧🇷 Português</a> 
  <a href="README.en.md">🇺🇸 English</a> 
  <a href="README.es.md">🇪🇸 Español</a> 
  <a href="README.it.md">🇮🇹 Italiano</a>
</p>

## Instalación rápida

Para instalar la aplicación desde el repositorio público de DNF:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Con el repositorio configurado, la propia app ya puede instalar el conjunto
principal mediante la acción rápida `Instalar soporte principal`, trayendo la
app de cámara, el driver `OV02C10` y el soporte `MAX98390` para los altavoces.
También puede ofrecer la instalación de `Galaxy Book Sound`, que se encarga
del ecualizador, los perfiles y el modo Atmos compatible.

`Galaxy Book Setup` es un asistente de instalación y diagnóstico para portátiles
Samsung Galaxy Book en Fedora. Su objetivo es organizar flujos que normalmente
terminan repartidos entre terminal, logs, paquetes RPM y validaciones manuales.

El foco actual es la **cámara interna** del Galaxy Book4 Ultra, pero el
proyecto ya cubre también los **altavoces internos con MAX98390**, además de
GPU, perfil de plataforma e integraciones generales del escritorio. El módulo
de fingerprint sigue planificado, pero todavía no forma parte de esta versión.

## Alcance

Esta app no sustituye:

- el driver del kernel;
- la app final de cámara;
- herramientas de bajo nivel como `akmods`, `modinfo` o `journalctl`.

Su papel es actuar como un **asistente de instalación y validación**, mostrando
el estado actual de la máquina y organizando los siguientes pasos.

En el flujo de audio, eso significa separar bien las responsabilidades:
`Galaxy Book Setup` valida la ruta de los altavoces internos, organiza la
instalación y abre `Galaxy Book Sound`, mientras que la ecualización, los
perfiles y `Atmos compatible` quedan en la propia app de sonido.

## Relación con los otros repositorios

Este proyecto trabaja junto con:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-max98390>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>
- <https://github.com/regiscaio/fedora-galaxy-book-sound>

Responsabilidades:

- `fedora-galaxy-book-ov02c10`: módulo `ov02c10` empaquetado para Fedora;
- `fedora-galaxy-book-max98390`: soporte empaquetado de los altavoces internos mediante MAX98390;
- `fedora-galaxy-book-camera`: app de uso diario de la cámara;
- `fedora-galaxy-book-sound`: app de ecualizador, perfiles y Atmos compatible con backend propio en PipeWire;
- `fedora-galaxy-book-setup`: asistente de instalación, diagnóstico y flujo.

## Capacidades actuales

La versión actual ya organiza la interfaz en áreas claras:

- `Sistema`: resumen del portátil, Fedora, kernel y Secure Boot;
- `Diagnósticos`: checklist global del estado de cámara, bridge para navegador, audio, `Galaxy Book Sound`, GPU e integraciones del escritorio, incluido el perfil de la dock de GNOME usado en este portátil;
- `Acciones rápidas`: instalación, reparación y prioridad del driver, activación de cámara para navegador, activación de altavoces internos, instalación y apertura de `Galaxy Book Sound`, flujo NVIDIA, perfil equilibrado, reaplicación del perfil de la dock, reinicio y apertura de la app de cámara o de sonido;
- `Módulos futuros`: espacio reservado para fingerprint y otros flujos.

Dentro de `Diagnósticos`, cada fila abre una subsección de **acciones sugeridas**.
Eso permite saltar a las correcciones y validaciones más relevantes para el ítem
seleccionado sin perder la página global de acciones rápidas.

La checklist también cubre ahora el estado de `Dash to Dock`, validando si la
dock inferior auto-ocultable mantiene el perfil usado en este notebook.

La checklist cubre hoy:

- paquetes principales de la cámara;
- generación del driver en el arranque vía `akmods`;
- origen del módulo `ov02c10` activo;
- detección directa con `libcamera` usada por `Galaxy Book Camera`;
- bridge V4L2 para navegadores y aplicaciones de comunicación;
- errores conocidos del arranque;
- ruta MAX98390 para los altavoces internos, incluso cuando el paquete está
  instalado pero el kernel actual todavía no expone `snd-hda-scodec-max98390`
  mediante `modinfo`;
- presencia de `Galaxy Book Sound`;
- estado del driver NVIDIA y la observación de que `nvidia-smi` es opcional;
- estado del perfil de plataforma, con `balanced` como valor recomendado;
- estado de `Dash to Dock`, incluyendo la validación del perfil usado en este
  portátil;
- extensiones de GNOME como historial del portapapeles, GSConnect e iconos en
  el escritorio.

Las acciones rápidas no se limitan a copiar comandos: ejecutan los flujos
principales directamente desde la interfaz, solicitando privilegios
administrativos cuando hace falta.

Las acciones rápidas actuales incluyen:

- instalar el soporte principal del portátil desde el propio setup, trayendo la
  app de cámara, el driver `OV02C10` y el soporte `MAX98390`;
- instalar `Galaxy Book Sound` para aplicar ecualización y Atmos compatible en
  la sesión actual mediante PipeWire;
- reinstalar o reparar el soporte NVIDIA;
- reaplicar el perfil de `Dash to Dock` usado en este portátil, reactivando la
  extensión y restaurando el comportamiento esperado de la dock inferior
  auto-ocultable cuando la configuración del escritorio se desvía;
- abrir `Galaxy Book Camera`;
- abrir `Galaxy Book Sound`.

## Instalación para usuarios

### Vía el repositorio público de DNF

La ruta recomendada es:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Después, dentro de la app:

1. abre `Acciones rápidas`;
2. ejecuta `Instalar soporte principal`;
3. usa acciones específicas si cámara, audio, NVIDIA o la dock siguen
   necesitando ajuste.

### Vía RPM local

El proyecto también puede empaquetarse localmente:

```bash
make rpm
```

Después, el RPM puede instalarse con:

```bash
sudo dnf install /ruta/a/galaxybook-setup-*.rpm
```

## Build

Dependencias de build en Fedora:

```bash
sudo dnf install cargo rust pkgconf-pkg-config gtk4-devel libadwaita-devel
```

Si el host no tiene todo el toolchain, el `Makefile` usa un contenedor rootless
con `podman`.

Comandos principales:

```bash
make build
make test
make dist
make srpm
make rpm
```

## Empaquetado

Archivos relevantes:

- spec RPM: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- metadatos AppStream: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

El RPM usa `Recommends` para los paquetes más importantes del flujo:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

## Roadmap

Módulos planeados para próximas etapas:

- fingerprint;
- chequeos más amplios de compatibilidad de Galaxy Book con Fedora;
- nuevos flujos guiados para integraciones de GNOME y periféricos del portátil.

## Licencia

Este proyecto se distribuye bajo **GPL-3.0-only**. Consulta [LICENSE](LICENSE).
