# MinIO e Garage (self-hosted)

## Parametri nel pannello

| Campo | MinIO | Garage |
|---|---|---|
| Endpoint | `http://host:9000` | `http://host:3900` |
| Regione | `us-east-1` | `garage` |
| Indirizzamento | `path` | `path` |
| Rete locale | spunta la casella se l'endpoint è privato | idem |

## Chiavi di accesso

**MinIO**: crea un utente con una policy di sola lettura:

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject"],
    "Resource": ["arn:aws:s3:::*/*"]
  }]
}
```

`mc admin policy create ALIAS readonly-get policy.json`, poi assegnala all'utente. **Garage**: `garage key create nome-applicazione`, poi consenti la lettura sul bucket con `garage bucket allow --read BUCKET --key nome-applicazione`.

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- Il gateway è stato provato dal vivo solo contro un S3 finto locale con firma SigV4 (test e2e): questa scheda non è stata collaudata su un'installazione reale di MinIO o Garage.
- Endpoint su `localhost` o rete privata: **spunta** *Lo storage è in rete locale*, altrimenti viene rifiutato ([Sicurezza](/guide/security)).
- Con Garage, la documentazione raccomanda `path-style`.
- Se metti un proxy con HTTPS davanti, usa l'URL del proxy come endpoint.

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [MinIO: creare una policy](https://docs.min.io/enterprise/aistor-object-store/reference/cli/admin/mc-admin-policy/mc-admin-policy-create/)
- [Garage: connettere applicazioni](https://garagehq.deuxfleurs.fr/documentation/connect/apps/)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
