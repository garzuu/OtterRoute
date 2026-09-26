# Installazione

OtterRoute è un unico binario (`otterroute`) più la cartella del pannello. Serve una macchina con una porta pubblica (80) raggiungibile dai domini che vuoi servire.

## Requisiti

- Linux, macOS o un container. Il gateway non ha dipendenze di sistema.
- Spazio su disco per la cache (default **10 GiB**, regolabile).
- Un bucket compatibile S3 con una chiave di **sola lettura** (vedi [Bucket S3](./buckets-s3)).
- Un dominio di cui puoi modificare il DNS (vedi [Domini e DNS](./domains-dns)). Per provare in locale bastano nomi `*.localhost`.

## Con Docker

```sh
git clone https://github.com/garzuu/OtterRoute.git && cd OtterRoute
docker build -t otterroute .
docker run -d --name otterroute \
  -p 80:80 -p 127.0.0.1:9090:9090 \
  -v otterroute-data:/data \
  otterroute
```

- Il pannello risponde su `http://127.0.0.1:9090` **solo dalla macchina**: mappalo su `127.0.0.1`, non su tutte le interfacce.
- Il volume `/data` contiene cache e stato (utenti, credenziali, configurazione): va conservato e [salvato](./upgrades-backup).
- L'immagine parte come utente non privilegiato. Nel repository, `compose.yaml` imposta `net.ipv4.ip_unprivileged_port_start=0` per permettergli di usare la porta 80; con `docker run` lo stesso si ottiene con `--sysctl net.ipv4.ip_unprivileged_port_start=0`.

## Da uno script (Linux e macOS)

Lo script scarica la release, ne verifica **checksum e firma** e la installa in `/usr/local` (su Linux, da root, crea anche l'utente e il servizio systemd, con permessi che permettono l'[aggiornamento automatico](./upgrades-backup)):

```sh
curl -fsSL https://github.com/garzuu/OtterRoute/releases/latest/download/install.sh | sudo sh
# oppure una versione precisa:  sudo sh install.sh --version 0.1.0
```

Rilanciarlo aggiorna un'installazione esistente (tiene il vecchio eseguibile come `otterroute.prev`). Dopo: `sudo systemctl enable --now otterroute`.

## Dai sorgenti

```sh
cargo build --release -p otterroute        # gateway  → target/release/otterroute
cd web && npm ci && npm run build          # pannello → web/dist
OTR_UI_DIR=web/dist ./target/release/otterroute
```

Al primo avvio, senza nessuna configurazione, il nodo parte vuoto e ascolta sulla **80** (traffico pubblico) e su `127.0.0.1:9090` (pannello). Su Linux la porta 80 richiede privilegi: dai al binario la capability `cap_net_bind_service` (`setcap 'cap_net_bind_service=+ep' otterroute`) oppure usa un'altra porta con `OTR_LISTEN` e un proxy davanti.

## Come servizio (systemd)

```ini
# /etc/systemd/system/otterroute.service
[Unit]
Description=OtterRoute
After=network-online.target

[Service]
User=otterroute
ExecStart=/usr/local/bin/otterroute
Environment=OTR_UI_DIR=/usr/local/share/otterroute/ui
Environment=OTR_STATE_DIR=/var/lib/otterroute/state
Environment=OTR_CACHE_DIR=/var/lib/otterroute/cache
Environment=OTR_CONFIG=/var/lib/otterroute/config.yaml
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## Porte standard

| Porta | Uso | Si cambia con |
|---|---|---|
| **80** | HTTP pubblico (i domini arrivano qui) | `OTR_LISTEN` per l'ascolto; **Impostazioni → HTTP** per la porta usata nei controlli |
| **443** | HTTPS (con [certificati automatici](./https)) | `OTR_HTTPS_LISTEN` per l'ascolto; Impostazioni → Porte per la porta usata nei redirect |
| **9090** | Pannello e API, solo localhost | `OTR_ADMIN_LISTEN` |

Tutte le opzioni sono nel [riferimento delle variabili d'ambiente](/reference/environment).

## Verifica che sia acceso

```sh
curl -s http://127.0.0.1:9090/healthz      # {"status":"ok"}
```

Poi apri `http://127.0.0.1:9090/` nel browser: è il [primo avvio](./first-run).
