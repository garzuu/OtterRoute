# Backblaze B2

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://s3.REGIONE.backblazeb2.com` (es. `s3.us-west-004.backblazeb2.com`) |
| Regione | quella dell'endpoint (`us-west-004`, `us-east-005`…) |
| Indirizzamento | `path` o `virtual` (entrambi supportati) |

## Chiavi di accesso

Da **Application Keys** crea una chiave di sola lettura, limitata al bucket. *keyID* è l'access key, *applicationKey* è il segreto (visibile una volta sola). Servono chiavi con accesso S3.

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- L'endpoint si legge nei dettagli del bucket: dipende da dove è stato creato.
- B2 richiede firme v4 (le v2 non sono supportate); la regione entra nella firma, quindi deve coincidere con quella dell'endpoint.

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [API S3-compatibile di Backblaze B2](https://www.backblaze.com/docs/cloud-storage-s3-compatible-api)
- [Chiamare l'API S3](https://www.backblaze.com/docs/cloud-storage-call-the-s3-compatible-api)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
