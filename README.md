<p align="center">
  <img src="docs/assets/logo.png" alt="OtterRoute" width="360">
</p>

# OtterRoute

OtterRoute è un gateway self-hosted che pubblica file da più bucket S3
compatibili attraverso più domini, con cache su disco. Colleghi uno storage,
associ un dominio e ottieni un URL funzionante, senza toccare la
configurazione di un proxy.

> **Stato: prototipo tecnico.** Questa fase serve a validare il motore:
> due domini che servono due bucket privati con credenziali diverse, con cache
> isolata. Pannello, controller, HTTPS automatico e Helm arrivano nelle fasi
> successive (vedi [Roadmap](#roadmap)).

## Cosa fa oggi

- **Instradamento** per host e prefisso di percorso; vince il prefisso più
  lungo, confrontato per segmenti interi (`/docs/` non intercetta `/docsx/`).
- **Più storage S3** con credenziali separate, firma SigV4 scritta da zero,
  indirizzamento `path` o `virtual`.
- **Sola lettura:** `GET` e `HEAD`. La query string non viene mai inoltrata
  allo storage e gli header `x-amz-*` non arrivano al client.
- **Cache su disco** con limite di spazio (LRU), cache negativa breve per i
  file mancanti, una sola richiesta allo storage per oggetto anche con molti
  client contemporanei, streaming senza caricare il file in RAM.
- **Svuotamento senza cancellazioni:** aumentando `cache_generation` di una
  destinazione cambiano le chiavi di cache; le copie vecchie diventano
  irraggiungibili e vengono eliminate dall'LRU.
- **HTTP:** `ETag`, `If-None-Match`, `If-Modified-Since`, `Range`, `If-Range`,
  rivalidazione con lo storage (304) delle copie scadute.
- **Errori:** un `403 AccessDenied` dello storage (file mancante senza permesso
  di elenco) diventa `404`; credenziali sbagliate diventano `502` e non vanno
  mai in cache; con lo storage giù si possono servire copie scadute
  (`serve_stale_on_error`).
- **Configurazione** ricaricata a caldo quando il file cambia e `version`
  aumenta; se non è valida resta attiva la precedente. L'ultima valida viene
  salvata e usata all'avvio se quella indicata non è disponibile.
- **Endpoint interni** (IP privati, `localhost`) consentiti solo con
  `allow_private_endpoint: true`, controllati anche sugli indirizzi risolti
  dal DNS: il gateway non diventa un proxy verso la rete interna.

## Avvio rapido

```sh
docker compose up -d --build     # due MinIO con credenziali diverse + gateway
./scripts/seed.sh                 # bucket privati con file di prova

curl -H 'Host: img.localhost'   localhost:8080/barca.jpg
curl -H 'Host: media.localhost' localhost:8080/docs/listino.pdf
curl localhost:9090/status        # regole attive e stato della cache

./scripts/e2e.sh                  # test end-to-end
```

Senza Docker, con due finti S3 che verificano davvero la firma SigV4
(solo Python 3 e Perl, nessuna dipendenza):

```sh
./scripts/local-e2e.sh            # compila, avvia tutto e lancia i test end-to-end
```

## Configurazione

Vedi [`examples/config.yaml`](examples/config.yaml). Le entità sono:

| Entità | Contenuto |
|---|---|
| `storages` | endpoint, regione, indirizzamento, credenziali (per ora da variabili d'ambiente) |
| `destinations` | storage, bucket, cartella, `revision`, `cache_generation` |
| `cache_policies` | `ttl`, `ttl_not_found`, `query_keys` ammesse, `serve_stale_on_error` |
| `routes` | `host`, `path_prefix`, `strip_prefix`, destinazione, politica |

Esempio: con la regola `media.azienda.it` + `/docs/` → destinazione
`documenti` (bucket `documenti`, cartella `pubblici/`), la richiesta
`media.azienda.it/docs/listino.pdf` legge `documenti/pubblici/listino.pdf`.

Regole di validazione (bloccanti): coppia host + prefisso unica, riferimenti
esistenti, niente `.`/`..`/`//` nei prefissi, niente porta o wildcard
nell'host, credenziali presenti.

### Opzioni del nodo

| Variabile | Default | |
|---|---|---|
| `OTR_CONFIG` | `config.yaml` | configurazione pubblicata |
| `OTR_LISTEN` | `0.0.0.0:8080` | traffico pubblico |
| `OTR_ADMIN_LISTEN` | `127.0.0.1:9090` | `/healthz`, `/status` |
| `OTR_CACHE_DIR` | `./data/cache` | |
| `OTR_CACHE_MAX_BYTES` | 10 GiB | |
| `OTR_CACHE_MAX_OBJECT_BYTES` | 1 GiB | oggetti più grandi: niente cache |
| `OTR_STATE_DIR` | `./data/state` | ultima configurazione valida |
| `OTR_RELOAD_INTERVAL` | `2s` | |

La risposta indica cosa è successo con `X-Cache`: `HIT`, `MISS`, `STALE`,
`REVALIDATED`, `BYPASS`.

## Limiti noti del prototipo

- `HEAD` e `Range` senza copia in cache passano allo storage senza riempire
  la cache: il primo accesso a un video a pezzi non lo mette in cache.
- Mentre un oggetto viene scaricato, le altre richieste aspettano fino a 5
  secondi, poi vanno allo storage senza cache.
- Niente HTTPS integrato: per ora va messo davanti un proxy.
- Niente `index.html` per le cartelle (scelta dell'MVP).

## Roadmap

| Fase | Risultato verificabile |
|---|---|
| **Prototipo tecnico** | Due domini servono due bucket privati differenti, con cache isolata |
| MVP singolo nodo | HTTPS automatico, controller, pannello, diagnostica, backup e ripristino |
| Versione cluster | Helm, configurazioni sincronizzate, stato di applicazione per replica |
| Estensioni | Link firmati, ottimizzazione immagini, accessi separati per clienti |

## Licenza

MIT oppure Apache-2.0, a scelta.
