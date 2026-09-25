# DigitalOcean Spaces

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://REGIONE.digitaloceanspaces.com` (es. `nyc3`) |
| Regione | quella del datacenter (`nyc3`, `ams3`…) |
| Indirizzamento | `virtual` (indicato dalla documentazione) |

## Chiavi di accesso

Dal pannello **API → Spaces Keys** crea una chiave; se disponibile, con accesso limitato (sola lettura) al bucket.

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- La documentazione degli SDK usa `us-east-1` come regione nei client per requisiti AWS, mentre la posizione reale è data dall'endpoint. Nel gateway la regione entra nella firma: se ricevi errori di firma, prova sia il codice del datacenter sia `us-east-1`.
- Le URL firmate non funzionano con la CDN di Spaces; il gateway non le usa.

## Problemi noti

Da verificare: la regione da usare nella firma (`nyc3` o `us-east-1`) non è chiara dalla sola documentazione; provala con **Prova la connessione** del pannello.

::: info Fonti ufficiali — verificato il 2026-09-26
- [Usare gli SDK AWS con Spaces](https://docs.digitalocean.com/products/spaces/how-to/use-aws-sdks/)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
