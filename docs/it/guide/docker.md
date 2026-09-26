# Docker

L'immagine ufficiale è `ghcr.io/garzuu/otterroute` (pubblica, per `linux/amd64` e `linux/arm64`). Contiene il nodo, il pannello e la [guida offline](./install) sotto `/docs/`.

## Tag

| Tag | Cosa segue |
|---|---|
| `0.1.0` | Esattamente quella versione: non cambia mai. |
| `0.1` | L'ultima correzione della serie 0.1 (`0.1.1`, `0.1.2`…). **Consigliato per la produzione.** |
| `latest` | L'ultima versione stabile, anche con cambi importanti: comodo per provare, rischioso in produzione. |

Le versioni di prova hanno un tag con suffisso (`0.1.0-rc3`) e non aggiornano `0.1` né `latest`.

## Avvio

```sh
docker run -d --name otterroute --restart unless-stopped \
  -p 80:80 -p 443:443 -p 127.0.0.1:9090:9090 \
  --sysctl net.ipv4.ip_unprivileged_port_start=0 \
  -v otterroute-data:/data \
  ghcr.io/garzuu/otterroute:0.1
```

- `-p 127.0.0.1:9090:9090` tiene il **pannello solo in locale**: non pubblicare mai la 9090 su tutte le interfacce.
- `--sysctl …=0` serve perché il processo non gira come root e deve usare le porte 80 e 443.
- Il volume `/data` contiene **tutto** ciò che conta (utenti, chiavi, certificati, configurazione, cache): non perderlo.

Con Compose (`compose.yaml` nel repository):

```yaml
services:
  gateway:
    image: ghcr.io/garzuu/otterroute:0.1
    restart: unless-stopped
    ports: ["80:80", "443:443", "127.0.0.1:9090:9090"]
    sysctls:
      net.ipv4.ip_unprivileged_port_start: 0
    volumes:
      - gateway-data:/data
volumes:
  gateway-data:
```

## Aggiornare

Il container non si aggiorna dall'interno. Il pannello ti avvisa della versione nuova ([Aggiornamenti](./upgrades-backup)); poi:

```sh
# 1. backup del volume (vedi Aggiornamenti e backup)
# 2. scarica la versione nuova
docker pull ghcr.io/garzuu/otterroute:0.1
# 3. ricrea il container con lo stesso volume
docker stop otterroute && docker rm otterroute
docker run -d --name otterroute ...   # lo stesso comando di prima
```

Con Compose: `docker compose pull && docker compose up -d`. Dati e configurazione restano nel volume; le migrazioni avvengono da sole al primo avvio; le sessioni di accesso si perdono. C'è un fermo di pochi secondi.

## Aggiornare in automatico (Watchtower)

Se vuoi che i container si aggiornino da soli puoi usare [Watchtower](https://containrrr.dev/watchtower/): controlla il registro e ricrea il container quando c'è un'immagine nuova per il tag che usi.

```yaml
services:
  gateway:
    image: ghcr.io/garzuu/otterroute:0.1        # tag di serie, non latest
    labels:
      - com.centurylinklabs.watchtower.enable=true
  watchtower:
    image: containrrr/watchtower
    command: --label-enable --interval 86400   # una volta al giorno
    volumes: ["/var/run/docker.sock:/var/run/docker.sock"]
```

::: warning Cosa perdi con l'aggiornamento automatico
Watchtower non fa il backup del volume né sa tornare indietro se la versione nuova ha un problema. Usa il tag di serie (`:0.1`), non `latest`, e fai backup periodici del volume. Dà a un servizio l'accesso al socket di Docker: valuta se ti va bene.
:::

## Tornare indietro

Riavvia con il tag della versione precedente (`ghcr.io/garzuu/otterroute:0.1.0`). Se la versione nuova ha già migrato i dati, ripristina anche il backup del volume fatto prima dell'aggiornamento.

## Verificare la versione

```sh
docker exec otterroute otterroute --version
```

oppure nel pannello, in **Impostazioni → Aggiornamenti**.
