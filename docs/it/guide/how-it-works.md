# Come funziona

OtterRoute è un **gateway di sola lettura** davanti a uno o più bucket compatibili S3. Riceve le richieste HTTP dei visitatori, capisce da quale bucket e da quale cartella servire il file e lo restituisce, tenendone una copia su disco per le richieste successive.

## Il percorso di una richiesta

<!--@include: @/diagrams/request-flow.it.svg-->

1. **Instradamento.** L'host della richiesta (`img.azienda.it`) e l'inizio del percorso (`/foto/`) scelgono un *instradamento*. Se più regole coincidono vince il prefisso più lungo, confrontato per segmenti interi: `/docs/` non intercetta `/docsx/`.
2. **Cache.** Se esiste una copia fresca su disco la risposta parte subito. Le copie scadute si rivalidano con lo storage (`304`) e, se lo storage non risponde, si possono servire comunque per un po'.
3. **Storage.** Altrimenti OtterRoute chiede il file al bucket con una richiesta firmata (AWS Signature v4, scritta da zero) usando le credenziali di *quel* bucket. Molti visitatori insieme provocano **una sola** richiesta allo storage.
4. **Risposta.** Il file viene inviato in streaming mentre si salva in cache, senza caricarlo in memoria. L'header `X-Cache` dice cosa è successo: `HIT`, `MISS`, `STALE`, `REVALIDATED`, `BYPASS`.

## Le parti del sistema

<!--@include: @/diagrams/architecture.it.svg-->

| Parte | Dove | Cosa fa |
|---|---|---|
| **Gateway pubblico** | porta **80** (HTTP) | Serve i file ai visitatori. Con i certificati automatici serve anche **HTTPS** sulla 443: vedi [HTTPS automatico](./https) (oppure [HTTPS e proxy](./https-proxy)). |
| **Pannello e API** | porta **9090**, solo `localhost` | Domini, bucket, instradamenti, utenti, statistiche. Non va mai esposto. |
| **Cache** | cartella su disco | Copie dei file, con limite di spazio (LRU). |
| **Stato** | cartella su disco | Utenti, configurazione generata, credenziali, statistiche, registro attività. |

Il pannello non è un accessorio: **genera** la configurazione del gateway (`config.yaml`) dai domini, bucket e instradamenti che inserisci, la valida e la applica subito, senza riavvii. Se la nuova configurazione non è valida resta attiva la precedente.

## Cosa fa e cosa non fa

- ✅ Serve file da bucket privati su più domini, con cache, `ETag`, `Range`, `If-None-Match`, `If-Modified-Since`.
- ✅ Verifica che i domini arrivino davvero al nodo e ricontrolla nel tempo.
- ✅ Mostra statistiche e le espone a Prometheus.
- ❌ **Solo lettura**: accetta `GET` e `HEAD`, nessuna scrittura verso lo storage.
- ✅ **HTTPS integrato** con certificati gratuiti che si rinnovano da soli (sfida HTTP-01), oppure un proxy davanti (Caddy, nginx, Traefik, Cloudflare).
- ❌ **Niente elenco delle cartelle**: `/cartella/` risponde `404`, non c'è `index.html` automatico.
- ❌ La **query string non arriva mai allo storage** e non entra nella chiave di cache, salvo i parametri che una politica di cache ammette.

::: tip Prossimo passo
Se vuoi provarlo, vai a [Installazione](./install) e poi a [Primo avvio](./first-run). Se hai già un nodo acceso e ti manca il DNS o lo storage, apri la [scheda del tuo provider](/providers/).
:::
