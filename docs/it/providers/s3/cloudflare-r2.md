# Cloudflare R2

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://ACCOUNT_ID.r2.cloudflarestorage.com` |
| Regione | `auto` |
| Indirizzamento | `path` |

## Chiavi di accesso

Dashboard → **R2 → Manage API tokens** → crea un token S3 con permesso **Object Read only**, limitato al bucket. Ottieni *Access Key ID* e *Secret Access Key* (il segreto si vede una volta sola).

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- La documentazione indica `auto` come regione; `us-east-1` e il valore vuoto vengono trattati come `auto`.
- Per i bucket con giurisdizione (UE, FedRAMP) l'endpoint ha un formato diverso: controlla la pagina del bucket.
- GetObject e HeadObject supportano le richieste con `Range`.
- Non sono supportati `x-amz-request-payer` e `x-amz-expected-bucket-owner`: il gateway non li usa.

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [R2: compatibilità API S3](https://developers.cloudflare.com/r2/api/s3/api/)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
