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

Per installare l'app dal repository DNF pubblico:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Con il repository configurato, l'app può già installare il supporto principale
tramite l'azione rapida `Installa supporto principale`, richiamando l'app della
fotocamera, il driver `OV02C10` e il supporto `MAX98390` per gli altoparlanti.
Può anche proporre l'installazione di `Galaxy Book Sound`, che gestisce
equalizzatore, profili e modalità Atmos compatibile.

`Galaxy Book Setup` è un assistente di installazione e diagnostica per notebook
Samsung Galaxy Book su Fedora. Il suo obiettivo è organizzare flussi che di
solito finiscono sparsi tra terminale, log, pacchetti RPM e verifiche manuali.

Il focus attuale è la **fotocamera interna** del Galaxy Book4 Ultra, ma il
progetto copre già anche gli **altoparlanti interni con MAX98390**, oltre a
GPU, profilo di piattaforma e integrazioni generali del desktop. Il modulo per
il fingerprint rimane pianificato, ma non è ancora incluso in questa versione.

## Ambito

Questa app non sostituisce:

- il driver del kernel;
- l'app finale della fotocamera;
- strumenti di basso livello come `akmods`, `modinfo` o `journalctl`.

Il suo ruolo è quello di un **assistente di installazione e validazione**,
mostrando lo stato corrente della macchina e organizzando i prossimi passi.

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
- `fedora-galaxy-book-max98390`: supporto pacchettizzato agli altoparlanti interni via MAX98390;
- `fedora-galaxy-book-camera`: app fotocamera per l'uso quotidiano;
- `fedora-galaxy-book-sound`: app per equalizzatore, profili e Atmos compatibile con backend PipeWire proprio;
- `fedora-galaxy-book-setup`: assistente di installazione, diagnostica e flusso.

## Capacità attuali

La versione attuale organizza già l'interfaccia in aree chiare:

- `Sistema`: riepilogo di notebook, Fedora, kernel e Secure Boot;
- `Diagnostica`: checklist globale sullo stato di fotocamera, bridge per browser, audio, `Galaxy Book Sound`, GPU e integrazioni desktop, incluso il profilo della dock di GNOME usato su questo notebook;
- `Azioni rapide`: installazione, riparazione e priorità del driver, attivazione della fotocamera per browser, attivazione degli altoparlanti interni, installazione e apertura di `Galaxy Book Sound`, flusso NVIDIA, profilo bilanciato, riapplicazione del profilo della dock, riavvio e apertura dell'app fotocamera o audio;
- `Moduli futuri`: spazio riservato per fingerprint e altri flussi.

Dentro `Diagnostica`, ogni riga apre anche una sottosezione di **azioni
suggerite**, utile per saltare alle correzioni più rilevanti per l'elemento
selezionato senza perdere la pagina generale delle azioni rapide.

La checklist copre ora anche lo stato di `Dash to Dock`, verificando se la
dock inferiore auto-nascosta mantiene il profilo usato su questo notebook.

La checklist copre oggi:

- pacchetti principali della fotocamera;
- generazione del driver all'avvio tramite `akmods`;
- origine del modulo `ov02c10` attivo;
- rilevamento diretto via `libcamera` usato da `Galaxy Book Camera`;
- bridge V4L2 per browser e app di comunicazione;
- errori noti di avvio;
- percorso MAX98390 per gli altoparlanti interni, incluso il caso in cui il
  pacchetto è installato ma il kernel corrente non espone ancora
  `snd-hda-scodec-max98390` tramite `modinfo`;
- presenza di `Galaxy Book Sound`;
- stato del driver NVIDIA e il fatto che `nvidia-smi` è opzionale;
- stato del profilo di piattaforma, con `balanced` come valore consigliato;
- stato di `Dash to Dock`, inclusa la verifica del profilo usato su questo
  notebook;
- estensioni GNOME come cronologia degli appunti, GSConnect e icone sul desktop.

Le azioni rapide non si limitano a copiare comandi: eseguono i flussi
principali direttamente dall'interfaccia, richiedendo privilegi
amministrativi quando necessario.

Le azioni rapide attuali includono:

- installare il supporto principale del notebook direttamente dal setup,
  portando con sé l'app fotocamera, il driver `OV02C10` e il supporto
  `MAX98390`;
- installare `Galaxy Book Sound` per applicare equalizzazione e Atmos
  compatibile nella sessione corrente tramite PipeWire;
- reinstallare o riparare il supporto NVIDIA;
- riapplicare il profilo `Dash to Dock` usato su questo notebook, riattivando
  l'estensione e ripristinando il comportamento previsto della dock inferiore
  auto-nascosta quando la configurazione del desktop si altera;
- aprire `Galaxy Book Camera`;
- aprire `Galaxy Book Sound`.

## Installazione per utenti

### Tramite il repository DNF pubblico

Il percorso consigliato è:

```bash
sudo dnf config-manager addrepo --from-repofile=https://packages.caioregis.com/fedora/caioregis.repo
sudo dnf install galaxybook-setup
```

Dopo di che, dall'interno dell'app:

1. apri `Azioni rapide`;
2. esegui `Installa supporto principale`;
3. usa le azioni specifiche se fotocamera, audio, NVIDIA o la dock richiedono
   ancora interventi.

### Tramite RPM locali

Il progetto può essere pacchettizzato localmente:

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

Se l'host non ha il toolchain completo, il `Makefile` usa un container rootless
con `podman`.

Comandi principali:

```bash
make build
make test
make dist
make srpm
make rpm
```

## Packaging

File rilevanti:

- spec RPM: [`packaging/fedora/galaxybook-setup.spec`](packaging/fedora/galaxybook-setup.spec)
- launcher: [`data/com.caioregis.GalaxyBookSetup.desktop`](data/com.caioregis.GalaxyBookSetup.desktop)
- metadati AppStream: [`data/com.caioregis.GalaxyBookSetup.metainfo.xml`](data/com.caioregis.GalaxyBookSetup.metainfo.xml)

L'RPM usa `Recommends` per i pacchetti più importanti del flusso:

- `akmod-galaxybook-ov02c10`
- `akmod-galaxybook-max98390`
- `galaxybook-camera`

## Roadmap

Moduli pianificati per le prossime fasi:

- fingerprint;
- controlli più ampi di compatibilità Galaxy Book su Fedora;
- nuovi flussi guidati per integrazioni GNOME e periferiche del notebook.

## Licenza

Questo progetto è distribuito con licenza **GPL-3.0-only**. Vedi [LICENSE](LICENSE).
