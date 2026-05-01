<p align="center">
  <img src="assets/galaxybook-setup.svg" alt="Icona di Galaxy Book Setup" width="112">
</p>

<h1 align="center">Galaxy Book Setup</h1>

<p align="center">
  <a href="README.md">🇧🇷 Português</a> 
  <a href="README.en.md">🇺🇸 English</a> 
  <a href="README.es.md">🇪🇸 Español</a> 
  <a href="README.it.md">🇮🇹 Italiano</a>
</p>

## Installazione rapida

Per installare il setup dal repository DNF pubblico:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Con il repository configurato, il setup stesso riesce già a installare il
set principale del notebook tramite l'azione rapida `Installa supporto
principale`, portando con sé l'app fotocamera, il driver `OV02C10` e il
supporto `MAX98390` per gli altoparlanti. Può anche offrire l'installazione di
`Galaxy Book Sound`, che resta responsabile di equalizzatore, profili e Atmos
compatibile.

`Galaxy Book Setup` è un assistente di installazione e diagnostica per
notebook Samsung Galaxy Book su Fedora. La proposta dell'app è organizzare
flussi di configurazione che normalmente finiscono dispersi tra terminale,
log, pacchetti RPM e validazioni manuali.

Il focus iniziale è la **fotocamera interna** del Galaxy Book4 Ultra, ma il
progetto accompagna già anche il flusso degli **altoparlanti interni con
MAX98390**, oltre a GPU, fingerprint, profilo di piattaforma e integrazioni
generali del sistema.

## Interfaccia attuale

### Schermata iniziale

![Galaxy Book Setup — schermata iniziale](img/app-setup-galaxy-1.png)

### Diagnostica

![Galaxy Book Setup — diagnostica](img/app-setup-galaxy-2.png)

### Audio interno

![Galaxy Book Setup — audio interno](img/app-setup-galaxy-3.png)

### Finestra modale `Informazioni`

![Galaxy Book Setup — Informazioni](img/app-setup-galaxy-4.png)

## Ambito

Questa app non sostituisce:

- il driver del kernel;
- l'app finale della fotocamera;
- strumenti di basso livello come `akmods`, `modinfo` o `journalctl`.

Il suo ruolo è funzionare come un **assistente di installazione e validazione**,
mostrando lo stato attuale della macchina e organizzando i prossimi passi.

Nel flusso audio, questo significa separare bene le responsabilità: `Galaxy
Book Setup` valida il percorso degli altoparlanti interni, organizza
l'installazione e apre `Galaxy Book Sound`, mentre equalizzazione, profili e
`Atmos compatibile` restano nell'app audio.

## Relazione con gli altri repository

Questo progetto lavora insieme a:

- <https://github.com/regiscaio/fedora-galaxy-book-ov02c10>
- <https://github.com/regiscaio/fedora-galaxy-book-max98390>
- <https://github.com/regiscaio/fedora-galaxy-book-camera>
- <https://github.com/regiscaio/fedora-galaxy-book-sound>

Responsabilità:

- `fedora-galaxy-book-ov02c10`: modulo `ov02c10` pacchettizzato per Fedora;
- `fedora-galaxy-book-max98390`: supporto pacchettizzato agli altoparlanti
  interni via MAX98390;
- `fedora-galaxy-book-camera`: app di uso quotidiano della fotocamera;
- `fedora-galaxy-book-sound`: app di equalizzatore, profili e Atmos
  compatibile con backend PipeWire proprio;
- `fedora-galaxy-book-setup`: assistente di installazione, diagnostica e flusso.

## Capacità attuali

La versione attuale dell'app organizza già l'interfaccia in aree ben definite:

- `Sistema`: riepilogo di notebook, Fedora, kernel e Secure Boot;
- `Diagnostica`: checklist generale con lo stato di fotocamera, bridge per
  browser, audio, `Galaxy Book Sound`, lettore di impronte, GPU, chiave MOK
  di `akmods` e integrazioni desktop, inclusa la dock GNOME usata su questo
  notebook;
- `Azioni rapide`: installazione, riparazione e regolazione di priorità del
  driver; attivazione della webcam per browser; attivazione degli altoparlanti
  interni; preparazione della chiave di `Secure Boot` per `MOK`; installazione
  e apertura di `Galaxy Book Sound`; riparazione dello stack fingerprint;
  attivazione del login con impronta; apertura della registrazione delle
  impronte; flusso NVIDIA; profilo bilanciato; riapplicazione del profilo
  della dock; riavvio e apertura dell'app fotocamera.

Dentro `Diagnostica`, ogni riga porta a una sottosezione di **azioni
suggerite**. Questo permette di aprire correzioni e validazioni più rilevanti
per l'elemento selezionato senza perdere la pagina generale di `Azioni rapide`.

L'app espone anche un riepilogo di avvisi ed errori tramite notifiche desktop.
In dock ed estensioni che supportano il contatore sul launcher, l'icona può
mostrare il numero totale di elementi marcati come `Attenzione` o `Errore`
nei diagnostici.

La checklist copre oggi:

- pacchetti principali della fotocamera;
- generazione del driver al boot tramite `akmods`;
- origine del modulo `ov02c10` attivo;
- rilevamento della fotocamera nel percorso diretto di `libcamera` usato da
  `Galaxy Book Câmera`;
- bridge V4L2 per browser e applicazioni di comunicazione;
- errori noti di boot;
- percorso MAX98390 degli altoparlanti interni, incluso il caso in cui il
  pacchetto è installato ma il kernel attuale non espone ancora
  `snd-hda-scodec-max98390` via `modinfo`;
- presenza di `Galaxy Book Sound`;
- presenza del lettore di impronte integrato;
- preparazione del login con impronta tramite `fprintd` e `authselect`;
- stato del driver NVIDIA e l'osservazione che `nvidia-smi` è opzionale;
- stato di preparazione della chiave pubblica di `akmods` in `MOK` quando
  `Secure Boot` è attivo;
- profilo d'uso della piattaforma, con `balanced` in evidenza come
  raccomandazione;
- stato di `Dash to Dock`, con verifica del profilo della dock usato su questo
  notebook;
- estensioni GNOME come cronologia degli appunti, GSConnect e icone sul
  desktop.

Le azioni rapide non si limitano a copiare comandi: eseguono i flussi
principali direttamente dall'interfaccia, usando privilegi amministrativi
quando necessario.

Oggi, le azioni disponibili includono:

- installare il supporto principale del notebook direttamente dal setup,
  portando con sé l'app fotocamera, il driver `OV02C10` e il supporto
  `MAX98390`;
- installare il set principale della fotocamera;
- ricostruire il driver con `akmods`;
- abilitare il caricamento di `ov02c10` al boot e caricare subito il modulo;
- forzare la priorità del driver corretto in `updates/`, con firma per Secure
  Boot quando necessario, senza compressione incompatibile e con un messaggio
  esplicito quando il kernel attuale ha già provato ad avviare la fotocamera
  troppo presto;
- ripristinare lo stack Intel IPU6 pacchettizzato quando il percorso diretto di
  `Galaxy Book Câmera` smette di vedere il sensore;
- attivare la fotocamera per browser tramite `icamerasrc`, `v4l2-relayd` e
  `v4l2loopback`, preservando l'accesso diretto di `libcamera`;
- attivare il supporto agli altoparlanti interni via `MAX98390`, con
  ricostruzione dei moduli, fallback manuale di installazione sul kernel
  attuale e servizio I2C al boot;
- preparare la chiave di `Secure Boot` per `akmods`, generando la chiave
  locale, creando la richiesta di importazione in `MOK` e lasciando il
  riavvio pronto per `Enroll MOK` all'avvio;
- installare `Galaxy Book Sound` per applicare equalizzazione e Atmos
  compatibile nella sessione via PipeWire;
- reinstallare lo stack fingerprint con `fprintd` e `libfprint`;
- abilitare `with-fingerprint` in `authselect`;
- aprire direttamente la registrazione delle impronte nelle impostazioni
  utente;
- installare o riparare il supporto NVIDIA;
- applicare il profilo di piattaforma `balanced`;
- riapplicare il profilo della dock GNOME usato su questo notebook,
  riattivando `Dash to Dock` e ripristinando il comportamento atteso della
  dock inferiore auto-nascosta;
- riavviare il sistema;
- aprire `Galaxy Book Câmera`;
- aprire `Galaxy Book Sound`.

## Fotocamera dopo aggiornamenti del kernel

Dopo un aggiornamento del kernel, il boot può provare a caricare `ov02c10` prima
che `akmods` finisca di generare il modulo corretto per quel kernel. In questo
stato, il log registra:

```text
external clock 26000000 is not supported
probe with driver ov02c10 failed with error -22
```

Anche se `modinfo -n ov02c10` poi punta a `updates/` dopo la fine di `akmods`, il
grafo IPU6 di quel boot può già essere stato creato senza il sensore, quindi
`cam -l` non elenca la fotocamera interna.

La diagnostica ora tratta questo caso come errore del percorso diretto della
fotocamera e suggerisce `Regolare la priorità del driver` seguito dal riavvio.
L'azione ricostruisce e dà priorità al modulo corretto per il kernel attuale; il
riavvio ricrea il grafo multimediale con il driver corretto disponibile fin
dall'inizio del boot.

## Secure Boot e MOK

Se un'azione rapida fallisce con qualcosa del tipo:

```text
modprobe: ERROR: could not insert 'ov02c10': Key was rejected by service
modprobe: ERROR: could not insert 'snd_hda_scodec_max98390': Key was rejected by service
```

il problema non è la compilazione del modulo in sé. Questo errore significa
che il kernel continua a girare con `Secure Boot` attivo, ma la chiave usata
per firmare il modulo non è stata ancora accettata in `MOK`.

Il percorso atteso è:

```bash
mokutil --test-key /etc/pki/akmods/certs/public_key.der
sudo mokutil --import /etc/pki/akmods/certs/public_key.der
```

Se `mokutil --test-key` dice che la chiave `is already enrolled`, trattalo come
MOK già iscritto. In alcune versioni di Fedora, quel controllo può comunque
restituire uno stato shell diverso da zero anche in questo caso.

Lo stesso `Galaxy Book Setup` ora espone l'azione rapida
`Preparare la chiave di Secure Boot`, che:

- genera la chiave locale di `akmods` con `kmodgenca` quando serve;
- chiede una password temporanea di `MOK` nell'interfaccia;
- crea la richiesta di importazione tramite `mokutil`;
- aggiorna la diagnostica per mostrare se la chiave è pronta, in attesa di
  riavvio o se richiede ancora attenzione.

Dopo di questo:

1. riavvia il notebook;
2. entra in `Enroll MOK` nella schermata blu di boot;
3. conferma la password definita durante `mokutil --import`;
4. torna in Fedora ed esegui di nuovo l'azione rapida.

Il flusso di priorità di `ov02c10` e il flusso di attivazione di `MAX98390`
ora eseguono questo controllo prima di provare a caricare il modulo, così
l'errore non appare più come un fallimento opaco o un falso successo.

## Installazione per utenti

### Tramite repository DNF pubblico

Il percorso consigliato per gli utenti finali è:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Dopo questo, dall'interno della stessa app:

1. apri `Azioni rapide`;
2. esegui `Installa supporto principale`;
3. usa le azioni specifiche se fotocamera, audio, NVIDIA o la dock hanno
   ancora bisogno di regolazioni.

### Tramite RPM locali

Il progetto può anche essere pacchettizzato localmente:

```bash
make rpm
```

Poi l'RPM può essere installato con:

```bash
sudo dnf install /percorso/verso/galaxybook-setup-*.rpm
```

## Build

Dipendenze di build su Fedora:

```bash
sudo dnf install cargo rust pkgconf-pkg-config gtk4-devel libadwaita-devel
```

Se l'host non ha il toolchain completo, il `Makefile` usa un container
rootless con `podman`.

Comandi principali:

```bash
make build
make test
make dist
make srpm
make rpm
```

Per installare il launcher locale di sviluppo:

```bash
make install-local
```

## Packaging

File rilevanti:

- spec RPM: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- metadati AppStream: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

L'RPM usa `Recommends` per indicare i pacchetti più importanti del flusso:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

Questo permette che l'app venga installata anche prima del setup completo della
fotocamera, cosa desiderabile per un assistente di installazione.

## Roadmap

Prossime evoluzioni previste:

- controlli generali di compatibilità del Galaxy Book con Fedora;
- più flussi assistiti per integrazioni dell'ambiente GNOME e periferiche del
  notebook;
- approfondire le letture fingerprint con focus sulla validazione post-sospensione
  e sugli scenari di sensore occupato.

## Licenza

Questo progetto è distribuito sotto la licenza **GPL-3.0-only**. Consulta il
file [LICENSE](LICENSE).
