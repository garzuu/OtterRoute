# Metriche Prometheus

`GET http://127.0.0.1:9090/metrics`, formato testo, senza autenticazione. → [Statistiche](/guide/statistics)

| Metrica | Tipo | Etichette | Descrizione |
|---|---|---|---|
| `otterroute_requests_total` | counter | `route`, `cache`, `class` | Richieste servite. `cache`: `HIT`, `MISS`, `STALE`, `REVALIDATED`, `BYPASS`, `none`; `class`: `2xx`…`5xx`. |
| `otterroute_response_bytes_total` | counter | `route` | Byte inviati ai visitatori. |
| `otterroute_upstream_errors_total` | counter | `route`, `storage` | Errori verso lo storage. |
| `otterroute_request_duration_seconds` | histogram | `route` | Latenza fino alla risposta (`_bucket`, `_sum`, `_count`). |
| `otterroute_cache_bytes` | gauge | — | Spazio usato dalla cache. |
| `otterroute_cache_max_bytes` | gauge | — | Limite della cache. |
| `otterroute_cache_entries` | gauge | — | Copie in cache. |
| `otterroute_cache_inflight` | gauge | — | Scaricamenti dallo storage in corso. |
| `otterroute_config_version` | gauge | — | Versione della configurazione attiva. |

Le richieste che non corrispondono a nessuna regola hanno `route="-"`.
