# Aggiornamenti e backup

## Cosa c'è da salvare

Tutto ciò che conta sta nella **cartella di stato** (`OTR_STATE_DIR`, `/data/state` nell'immagine Docker):

| Elemento | Note |
|---|---|
| `users.json` | Utenti, ruoli, 2FA. Permessi `0600`. |
| `panel.json` | Domini, bucket, instradamenti, impostazioni. |
| `secrets/` | Le chiavi dei bucket. Permessi `0600`. |
| `last-good.yaml` | L'ultima configurazione valida (il nodo la usa se il file indicato non è disponibile). |
| `node-id` | L'identità del nodo. |
| `certs/` | Certificati HTTPS e relative chiavi (0600). Si possono anche riemettere, ma non quelli caricati a mano. |
| `notify.json`, `notify-state.json`, `notify-log.json` | Impostazioni delle [notifiche](./notifications), problemi già notificati, ultimi invii. |
| `metrics.json`, `audit.jsonl` | Statistiche e registro attività. |

Il file `config.yaml` (`OTR_CONFIG`) è **generato** dal pannello a partire da `panel.json`: si può rigenerare, ma tenerne una copia non costa nulla.

La **cache** (`OTR_CACHE_DIR`) invece non va salvata: si ricostruisce da sola.

## Fare un backup

I file sono scritti in modo atomico, quindi si può copiare la cartella anche con il nodo acceso. Per una copia coerente al 100% fermalo un istante.

```sh
tar czf otterroute-stato-$(date +%F).tgz -C /var/lib/otterroute state config.yaml
chmod 600 otterroute-stato-*.tgz
```

Il backup contiene chiavi e hash: conservalo cifrato e con permessi ristretti.

## Backup dal pannello

**Impostazioni → Backup e ripristino** produce un unico file `.otrbak` con utenti (hash delle password), chiavi dei bucket, certificati, domini, instradamenti, notifiche e identità del nodo; non c'è la cache. Il file è **cifrato** (AES-256-GCM, chiave derivata dalla tua frase con Argon2id): scegli una frase di almeno 12 caratteri e conservala **separata** dal file. Senza la frase non si recupera nulla. Serve lo scope `users:manage` (di default solo l'Amministratore).

Per ripristinare scegli il file, inserisci la frase e premi *Controlla il file*: il nodo mostra versione e data del backup senza cambiare nulla. Poi *Ripristina ora*:

- lo stato attuale viene copiato in `state/backups/pre-restore/` (una sola copia: il ripristino successivo la sostituisce);
- il nodo rifiuta backup creati da una **versione più recente** (aggiorna prima il nodo) o con formato sconosciuto; quelli di versioni precedenti sono accettati;
- il nodo si **riavvia** (stesso processo) e le sessioni si perdono: accedi con le credenziali del backup.

Il ripristino sostituisce tutto, compresa l'identità del nodo: serve anche per **spostare** il nodo su un altro server. Dopo lo spostamento aggiorna il DNS e, se il nodo ha un nuovo indirizzo, ricontrolla i domini.

## Ripristinare a mano

1. Ferma il nodo.
2. Ripristina la cartella di stato (e `config.yaml`) con gli stessi permessi.
3. Riavvia. Il nodo riparte con utenti, domini, bucket e instradamenti; le sessioni sono perse e i visitatori ricostruiscono la cache.

## Aggiornare

Il pannello ti avvisa quando c'è una versione nuova e ti mostra i passi per il tuo tipo di installazione. Con un'installazione **a binario o a servizio** il nodo può anche **aggiornarsi da solo** (vedi sotto); con **Docker** l'immagine si sostituisce da fuori.

### Sapere se c'è una versione nuova

Il nodo controlla **una volta al giorno** le release pubbliche su GitHub (una richiesta, senza inviare nulla del nodo). Se ne trova una più recente compare un avviso nella campanella e una scheda in **Impostazioni → Aggiornamenti**, con le note e i comandi. Da lì puoi anche **Controllare subito**, includere le versioni di prova (pre-release) o spegnere il controllo. Con `OTR_UPDATE_CHECK=off` il nodo non contatta mai GitHub (ambienti chiusi). Dopo un aggiornamento, per 24 ore un avviso ti dice da quale versione arrivi.

### Passi comuni

1. Fai un backup della cartella di stato.
2. Sostituisci il programma con la versione nuova (secondo il tipo di installazione qui sotto) e riavvia.
3. Accedi di nuovo: le sessioni non sopravvivono al riavvio.

### Docker

Non si sostituisce l'immagine dall'interno: si scarica la nuova e si ricrea il container **con lo stesso volume**. → [Docker](./docker)

```sh
docker pull ghcr.io/garzuu/otterroute:0.1
docker stop otterroute && docker rm otterroute
# rilancia lo STESSO comando "docker run" di prima (stesse porte, stesso volume /data)
```

### Aggiornamento automatico (binario e servizio)

Se il nodo gira da un eseguibile scaricato dalla release (non da Docker né dai sorgenti) e l'utente del servizio può **scrivere nella cartella dell'eseguibile**, in **Impostazioni → Aggiornamenti** compare **Aggiorna ora**. Da terminale: `otterroute --self-update` (poi riavvia il servizio) e `otterroute --check-update` per sapere solo se c'è una versione nuova.

Cosa fa, nell'ordine (se un passo fallisce l'installazione resta com'era):

1. **Scarica** il pacchetto della tua piattaforma, solo da GitHub, con un limite di 100 MiB.
2. **Verifica** lo **SHA-256** e la **firma Ed25519**. La chiave pubblica è dentro il binario; la privata sta nei segreti del repository e firma ogni release. Un pacchetto senza firma valida **si rifiuta**: l'aggiornamento manuale resta sempre possibile.
3. **Estrae** solo l'eseguibile, `ui/` e `docs/` (niente percorsi con `..`, niente collegamenti simbolici) e **prova** il nuovo eseguibile: dichiara la versione attesa e, avviato con stato e porte temporanei (`--self-check`), risponde a `/healthz`.
4. **Salva un backup** dello stato (utenti, configurazione, chiavi, certificati) in `state/backups/<versione>/`, tenendo gli ultimi 3.
5. **Sostituisce** l'eseguibile, il pannello e la guida, conservando i vecchi come `otterroute.prev`, `ui.prev`, `docs.prev`.
6. **Riavvia** se stesso con lo stesso PID e gli stessi argomenti: un servizio non vede alcun riavvio. Le sessioni di accesso si perdono.

**Se la versione nuova non parte bene**: dopo l'aggiornamento il nodo conta gli avvii; se per **3 avvii di fila non arriva la conferma** (60 secondi di funzionamento regolare) ripristina da solo i file precedenti e riparte con la versione vecchia. Il motivo compare nella campanella. Serve che qualcosa **riavvii il processo** quando si ferma (systemd con `Restart=always` o `on-failure`). Se il nuovo eseguibile si rifiuta di partire del tutto, la prova del passo 3 lo avrebbe già scoperto prima di sostituirlo.

**Automatico, se vuoi** (spento di default): la casella *Applica da solo le versioni di correzione* installa le versioni della stessa serie (0.1.x) nella finestra oraria che scegli (di default 03:00–05:00, ora del nodo). Le versioni minori e maggiori restano manuali, non parte durante l'emissione di un certificato e non installa mai una pre-release.

::: warning Quello che non fa
- Un cambio di **schema dei dati** non si annulla da solo: se la versione nuova ha già migrato i dati, per tornare indietro ripristina anche il backup in `state/backups/`.
- Non c'è aggiornamento senza interruzione: il nodo è fermo pochi secondi.
- La firma protegge quanto il segreto che la produce: se la chiave di firma venisse compromessa servirebbe una release manuale con una chiave nuova.
:::

Il pacchetto può essere installato anche da un fork o da un mirror interno: imposta `OTR_UPDATE_API` e la chiave pubblica con cui firmi (`OTR_UPDATE_KEY`).

### Binario o servizio (systemd, launchd), a mano

Scarica dalla [pagina delle release](https://github.com/garzuu/OtterRoute/releases) il pacchetto per la tua piattaforma, verificane il checksum e sostituisci l'eseguibile (e le cartelle `ui/` e `docs/` accanto a lui):

```sh
sha256sum -c otterroute-vX.Y.Z-linux-x86_64.tar.gz.sha256
tar xzf otterroute-vX.Y.Z-linux-x86_64.tar.gz
sudo systemctl stop otterroute
sudo install -m 0755 otterroute-vX.Y.Z-linux-x86_64/otterroute /usr/local/bin/otterroute
sudo systemctl start otterroute
```

Tieni il vecchio eseguibile (`otterroute.prev`) finché non hai visto che la versione nuova parte bene.

### Dai sorgenti

```sh
git pull && cargo build --release -p otterroute && (cd web && npm ci && npm run build)
```

**Migrazioni automatiche:** se aggiorni da una versione con un solo amministratore (`admin.json`), al primo avvio l'utente viene migrato in `users.json` con lo stesso ruolo Amministratore e la stessa password; il vecchio file diventa `admin.json.migrated`.

Non c'è un percorso di ritorno automatico: se devi tornare indietro, ripristina il backup fatto prima dell'aggiornamento.

## Se il nodo non parte

- **"nessuna configurazione valida disponibile"**: il file indicato in `OTR_CONFIG` non è leggibile e manca `last-good.yaml`. Ripristina il backup.
- **Porta occupata o non permessa**: controlla `OTR_LISTEN` e i privilegi per la porta 80.
- I log (`RUST_LOG=otterroute=info`) indicano quale configurazione è attiva e perché un cambio è stato rifiutato.
