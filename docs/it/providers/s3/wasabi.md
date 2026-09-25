# Wasabi

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://s3.REGIONE.wasabisys.com` (us-east-1: `s3.wasabisys.com`) |
| Regione | il codice della regione (`eu-central-1`, `eu-south-1`…) |
| Indirizzamento | `virtual` o `path` |

## Chiavi di accesso

Console Wasabi → **Access Keys** → crea una chiave (meglio per un utente con policy di sola lettura sul bucket).

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- L'endpoint dipende dalla regione del bucket: se sbagli, ricevi errori di firma o reindirizzamenti. Regioni elencate nella pagina ufficiale (Americhe, EMEA con alias come `s3.it-1.wasabisys.com`, APAC).

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [URL dei servizi Wasabi per regione](https://docs.wasabi.com/docs/what-are-the-service-urls-for-wasabis-different-storage-regions)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
