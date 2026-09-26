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
| `otterroute_build_info` | gauge | `version`, `install` | Sempre 1: versione in esecuzione e tipo di installazione (`docker`, `service`, `binary`, `source`). |
| `otterroute_image_transforms_total` | counter | `result` | [Immagini al volo](/guide/images): `ok`, `error` (file non valido, 422), `too_large` (413), `busy` (scaduta o interrotta, 503). |
| `otterroute_image_transform_seconds_sum` / `_count` | counter | — | Tempo totale e numero delle trasformazioni riuscite: la media è `sum / count`. |
| `otterroute_signed_links_denied_total` | counter | — | Richieste rifiutate con `403` per [link firmato](/guide/signed-links) assente, scaduto o non valido. |
| `otterroute_acme_issuances_total` | counter | `result` | Emissioni di [certificati automatici](/guide/https): `ok`, `error`. |
| `otterroute_certificate_not_after_timestamp_seconds` | gauge | `host` | Scadenza del certificato del dominio (secondi Unix); assente se non ce n'è uno. |
| `otterroute_certificate_serving` | gauge | `host` | 1 se il nodo serve su HTTPS un certificato non scaduto per il dominio. |
| `otterroute_update_available` | gauge | — | 1 se c'è una versione più recente di quella in uso. |
| `otterroute_update_last_check_timestamp_seconds` | gauge | — | Ultimo controllo delle nuove versioni (0 = mai). |
| `otterroute_update_checks_total` | counter | `result` | Controlli delle nuove versioni: `ok`, `error`. |
| `otterroute_updates_total` | counter | `result` | Aggiornamenti visti da questo processo: `applied` (avvio dopo un cambio di versione), `rolled_back`. |

Le richieste che non corrispondono a nessuna regola hanno `route="-"`.

## Query e allarmi utili

```text
# certificati che scadono entro 14 giorni
(otterroute_certificate_not_after_timestamp_seconds - time()) < 14 * 86400

# un dominio senza certificato in uso
otterroute_certificate_serving == 0

# durata media di una trasformazione di immagine (ultimi 5 minuti)
rate(otterroute_image_transform_seconds_sum[5m]) / rate(otterroute_image_transform_seconds_count[5m])

# trasformazioni fallite o rimandate
sum(rate(otterroute_image_transforms_total{result!="ok"}[5m]))

# il nodo non controlla le versioni da più di 3 giorni (con il controllo attivo)
time() - otterroute_update_last_check_timestamp_seconds > 3 * 86400

# c'è una versione nuova
otterroute_update_available == 1
```

