# Hetzner Object Storage

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://SEDE.your-objectstorage.com` (`fsn1`, `nbg1`, `hel1`) |
| Regione | il codice della sede (es. `fsn1`) |
| Indirizzamento | `virtual` |

## Chiavi di accesso

Nella console, dentro al progetto, genera le credenziali S3. Il segreto **non è più recuperabile** dopo la creazione: salvalo subito.

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- Gli endpoint sono legati alla **sede**, le credenziali al **progetto**: ogni combinazione sede/progetto ha il suo bucket e le sue chiavi.
- Gli esempi ufficiali disattivano il path-style (`path: off`): usa `virtual`.

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [Usare gli strumenti S3 con Hetzner](https://docs.hetzner.com/storage/object-storage/getting-started/using-s3-api-tools)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
