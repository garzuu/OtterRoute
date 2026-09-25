# config.yaml

Con il pannello il file è **generato**: non modificarlo a mano (il pannello lo riscrive). Se vuoi gestirlo da solo, non usare il pannello per quelle risorse. Il nodo rilegge il file ogni `OTR_RELOAD_INTERVAL`; una configurazione non valida viene rifiutata e resta attiva l'ultima buona.

Campi sconosciuti sono **errori**: un refuso non passa inosservato.

## Esempio completo

```yaml
version: 1
storages:
  - id: garage
    endpoint: http://127.0.0.1:3900
    region: garage
    addressing: path
    allow_private_endpoint: true
    credentials:
      access_key_env: GARAGE_KEY
      secret_key_env: GARAGE_SECRET
destinations:
  - id: foto
    storage: garage
    bucket: foto
    prefix: ""
    revision: 1
    cache_generation: 1
cache_policies:
  - id: standard
    ttl: 1h
    ttl_not_found: 60s
    serve_stale_on_error: 24h
routes:
  - id: cdn-foto
    host: cdn.example.com
    path_prefix: /foto
    strip_prefix: true
    destination: foto
    cache_policy: standard
```

## `version`

Numero intero; salire a ogni modifica (il pannello lo fa da solo). Compare in `otterroute_config_version`.

## `storages[]`

| Campo | Default | Descrizione |
|---|---|---|
| `id` | — | Nome univoco. |
| `endpoint` | — | URL del servizio S3. |
| `region` | `us-east-1` | Regione per la firma SigV4. |
| `addressing` | `path` | `path` (`endpoint/bucket/chiave`) o `virtual` (`bucket.endpoint/chiave`). |
| `allow_private_endpoint` | `false` | Ammette endpoint su indirizzi privati o locali. |
| `credentials` | — | Vedi sotto. |

### `credentials`

Una sola tra le due forme:

| Campo | Descrizione |
|---|---|
| `access_key_env` + `secret_key_env` | Nomi di variabili d'ambiente che contengono le chiavi. |
| `secret_file` | Percorso di un file JSON `{"access_key": "...", "secret_key": "..."}` (quello che scrive il pannello, permessi `0600`). |

## `destinations[]`

| Campo | Default | Descrizione |
|---|---|---|
| `id` | — | Nome univoco. |
| `storage` | — | `id` di uno storage. |
| `bucket` | — | Nome del bucket. |
| `prefix` | vuoto | Cartella nel bucket. |
| `revision` | `0` | Numero libero; salirlo segnala che la destinazione è cambiata. |
| `cache_generation` | `0` | Salirlo **invalida** le copie in cache di questa destinazione. |

## `cache_policies[]`

| Campo | Default | Descrizione |
|---|---|---|
| `id` | — | Nome univoco. |
| `ttl` | — | Durata di una copia fresca. |
| `ttl_not_found` | `60s` | Durata della cache negativa. |
| `query_keys` | nessuna | Parametri di query che fanno parte della chiave di cache. |
| `serve_stale_on_error` | `0` | Per quanto servire una copia scaduta se lo storage è giù. |

## `routes[]`

| Campo | Default | Descrizione |
|---|---|---|
| `id` | — | Nome univoco. |
| `host` | — | Dominio. |
| `path_prefix` | — | Prefisso del percorso (`/` per tutto il dominio). |
| `strip_prefix` | `true` | Toglie il prefisso prima di cercare il file. |
| `destination` | — | `id` di una destinazione. |
| `cache_policy` | — | `id` di una politica. |

Le durate: `30s`, `5m`, `1h`, `24h`, `7d`.
