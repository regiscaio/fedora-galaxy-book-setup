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

Para instalar el setup desde el repositorio DNF público:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Con el repositorio configurado, el propio setup ya puede instalar el conjunto
principal del portátil mediante la acción rápida `Instalar soporte principal`,
trayendo la app de cámara, el driver `OV02C10` y el soporte `MAX98390` de los
altavoces. También puede ofrecer la instalación de `Galaxy Book Sound`, que se
encarga del ecualizador, los perfiles y Atmos compatible.

`Galaxy Book Setup` es un asistente de instalación y diagnóstico para
portátiles Samsung Galaxy Book en Fedora. La propuesta de la app es organizar
flujos de configuración que normalmente terminan repartidos entre terminal,
logs, paquetes RPM y validaciones manuales.

El foco inicial es la **cámara interna** del Galaxy Book4 Ultra, pero el
proyecto ya acompaña también el flujo de los **altavoces internos con
MAX98390**, además de GPU, fingerprint, perfil de plataforma e integraciones
generales del sistema.

## Interfaz actual

### Pantalla inicial

![Galaxy Book Setup — pantalla inicial](img/app-setup-galaxy-1.png)

### Diagnósticos

![Galaxy Book Setup — diagnósticos](img/app-setup-galaxy-2.png)

### Audio interno

![Galaxy Book Setup — audio interno](img/app-setup-galaxy-3.png)

### Modal `Sobre`

![Galaxy Book Setup — Sobre](img/app-setup-galaxy-4.png)

## Alcance

Esta app no sustituye:

- el driver del kernel;
- la app final de cámara;
- herramientas de bajo nivel como `akmods`, `modinfo` o `journalctl`.

Su papel es funcionar como un **asistente de instalación y validación**,
mostrando el estado actual de la máquina y organizando los próximos pasos.

En el flujo de audio, eso significa separar bien las responsabilidades:
`Galaxy Book Setup` valida la ruta de los altavoces internos, organiza la
instalación y abre `Galaxy Book Sound`, mientras que la ecualización, los
perfiles y `Atmos compatible` quedan en la app de sonido.

## Relación con los otros repositorios

Este proyecto trabaja junto con:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-max98390>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>
- <https://github.com/regiscaio/fedora-galaxy-book-sound>

Responsabilidades:

- `fedora-galaxy-book-ov02c10`: módulo `ov02c10` empaquetado para Fedora;
- `fedora-galaxy-book-max98390`: soporte empaquetado para los altavoces
  internos vía MAX98390;
- `fedora-galaxy-book-camera`: app de uso diario de la cámara;
- `fedora-galaxy-book-sound`: app de ecualizador, perfiles y Atmos compatible
  con backend propio en PipeWire;
- `fedora-galaxy-book-setup`: asistente de instalación, diagnóstico y flujo.

## Capacidades actuales

La versión actual de la app ya organiza la interfaz en áreas bien definidas:

- `Sistema`: resumen del portátil, Fedora, kernel y Secure Boot;
- `Diagnósticos`: checklist general con el estado de la cámara, del bridge para
  navegador, del audio, de `Galaxy Book Sound`, del lector de huellas, de la
  GPU, de la clave MOK de `akmods` y de las integraciones del escritorio,
  incluida la dock de GNOME usada en este portátil;
- `Acciones rápidas`: instalación, reparación y ajuste de prioridad del
  driver; activación de la webcam para navegador; activación de los altavoces
  internos; preparación de la clave de `Secure Boot` para `MOK`; instalación
  y apertura de `Galaxy Book Sound`; reparación del stack de fingerprint;
  activación del inicio de sesión por huella; apertura del registro de
  huellas; flujo NVIDIA; perfil equilibrado; reaplicación del perfil de la
  dock; reinicio y apertura de la app de cámara.

Dentro de `Diagnósticos`, cada línea lleva a una subsección de **acciones
sugeridas**. Eso permite abrir correcciones y validaciones más relevantes para
el ítem seleccionado sin perder la página general de `Acciones rápidas`.

La app también expone un resumen de alertas y errores a través de
notificaciones del escritorio. En docks y extensiones que soportan contador en
el launcher, el icono puede mostrar la cantidad total de ítems marcados como
`Atención` o `Error` en los diagnósticos.

La checklist cubre hoy:

- paquetes principales de la cámara;
- generación del driver en el arranque vía `akmods`;
- origen del módulo `ov02c10` activo;
- detección de la cámara en la ruta directa de `libcamera` usada por `Galaxy
  Book Câmera`;
- bridge V4L2 para navegadores y comunicadores;
- errores conocidos del arranque;
- ruta MAX98390 de los altavoces internos, incluso cuando el paquete está
  instalado pero el kernel actual todavía no expone `snd-hda-scodec-max98390`
  vía `modinfo`;
- presencia de `Galaxy Book Sound`;
- presencia del lector de huellas integrado;
- preparación del inicio de sesión por huella con `fprintd` y `authselect`;
- estado del driver NVIDIA y la observación de que `nvidia-smi` es opcional;
- preparación de la clave pública de `akmods` en `MOK` cuando `Secure Boot`
  está activo;
- perfil de uso de la plataforma, con destaque para `balanced`;
- estado de `Dash to Dock`, con comprobación del perfil de dock usado en este
  portátil;
- extensiones de GNOME como historial del portapapeles, GSConnect e iconos en
  el escritorio.

Las acciones rápidas no solo copian comandos: ejecutan los flujos principales
desde la propia interfaz, usando privilegios administrativos cuando es
necesario.

Hoy, las acciones disponibles incluyen:

- instalar el soporte principal del portátil directamente desde el setup,
  trayendo la app de cámara, el driver `OV02C10` y el soporte `MAX98390`;
- instalar el conjunto principal de la cámara;
- reconstruir el driver con `akmods`;
- habilitar la carga de `ov02c10` en el arranque y cargar el módulo
  inmediatamente;
- forzar la prioridad del driver corregido en `updates/`, con firma para
  Secure Boot cuando sea necesario y sin compresión incompatible;
- restaurar el stack Intel IPU6 empaquetado cuando la ruta directa de `Galaxy
  Book Câmera` deja de ver el sensor;
- activar la cámara para navegador vía `icamerasrc`, `v4l2-relayd` y
  `v4l2loopback`, preservando el acceso directo de `libcamera`;
- activar el soporte a los altavoces internos vía `MAX98390`, con
  reconstrucción de módulos, fallback manual de instalación en el kernel
  actual y servicio de I2C en el arranque;
- preparar la clave de `Secure Boot` para `akmods`, generando la clave local,
  creando la solicitud de importación en `MOK` y dejando el reinicio listo
  para `Enroll MOK` en el arranque;
- instalar `Galaxy Book Sound` para aplicar ecualización y Atmos compatible en
  la sesión vía PipeWire;
- reinstalar el stack de fingerprint con `fprintd` y `libfprint`;
- habilitar `with-fingerprint` en `authselect`;
- abrir directamente el registro de huellas en la configuración de usuarios;
- instalar o reparar el soporte NVIDIA;
- aplicar el perfil `balanced` de la plataforma;
- reaplicar el perfil de la dock de GNOME usado en este portátil, reactivando
  `Dash to Dock` y restaurando el comportamiento esperado de la dock inferior
  auto-ocultable;
- reiniciar el sistema;
- abrir `Galaxy Book Câmera`;
- abrir `Galaxy Book Sound`.

## Secure Boot y MOK

Si alguna acción rápida falla con algo como:

```text
modprobe: ERROR: could not insert 'ov02c10': Key was rejected by service
modprobe: ERROR: could not insert 'snd_hda_scodec_max98390': Key was rejected by service
```

el problema no es la compilación del módulo en sí. Ese error significa que el
kernel sigue con `Secure Boot` activo, pero la clave usada para firmar el
módulo todavía no fue aceptada en `MOK`.

La ruta esperada es:

```bash
mokutil --test-key /etc/pki/akmods/certs/public_key.der
sudo mokutil --import /etc/pki/akmods/certs/public_key.der
```

Si `mokutil --test-key` dice que la clave `is already enrolled`, trátalo como
MOK ya inscrito. En algunas versiones de Fedora, esa verificación puede seguir
devolviendo un código de shell distinto de cero incluso en ese caso.

El propio `Galaxy Book Setup` ahora expone la acción rápida
`Preparar clave de Secure Boot`, que:

- genera la clave local de `akmods` con `kmodgenca` cuando hace falta;
- pide una contraseña temporal de `MOK` en la interfaz;
- crea la solicitud de importación en `mokutil`;
- actualiza el diagnóstico para mostrar si la clave quedó lista, pendiente de
  reinicio o si todavía necesita atención.

Después de eso:

1. reinicia el portátil;
2. entra en `Enroll MOK` en la pantalla azul del arranque;
3. confirma la contraseña definida en `mokutil --import`;
4. vuelve a Fedora y ejecuta la acción rápida otra vez.

Las acciones rápidas de prioridad del `ov02c10` y de activación del
`MAX98390` ahora hacen esta comprobación antes de intentar cargar el módulo,
para que el error no aparezca como un fallo opaco ni como un falso éxito.

## Instalación para usuarios

### Vía repositorio DNF público

La ruta recomendada para usuarios finales es:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Después de eso, dentro de la propia app:

1. abre `Acciones rápidas`;
2. ejecuta `Instalar soporte principal`;
3. usa las acciones específicas si cámara, audio, NVIDIA o la dock todavía
   necesitan ajuste.

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

Si el host no tiene el toolchain completo, el `Makefile` usa un contenedor
rootless con `podman`.

Comandos principales:

```bash
make build
make test
make dist
make srpm
make rpm
```

Para instalar el launcher local de desarrollo:

```bash
make install-local
```

## Empaquetado

Archivos relevantes:

- spec RPM: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- metadatos AppStream: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

El RPM usa `Recommends` para señalar los paquetes más importantes del flujo:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

Eso permite que la app se instale incluso antes del setup completo de la
cámara, lo que es deseable para un asistente de instalación.

## Roadmap

Próximas evoluciones previstas:

- comprobaciones generales de compatibilidad del Galaxy Book con Fedora;
- más flujos asistidos para integraciones del entorno GNOME y periféricos del
  portátil;
- profundizar las lecturas de fingerprint con foco en validación post-suspensión
  y escenarios de sensor ocupado.

## Licencia

Este proyecto se distribuye bajo la licencia **GPL-3.0-only**. Consulta el
archivo [LICENSE](LICENSE).
