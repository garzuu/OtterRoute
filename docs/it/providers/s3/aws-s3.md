# AWS S3

## Parametri nel pannello

| Campo | Valore |
|---|---|
| Endpoint | `https://s3.REGIONE.amazonaws.com` (es. `https://s3.eu-west-1.amazonaws.com`) |
| Regione | quella del bucket (`eu-west-1`, `us-east-1`…) |
| Indirizzamento | `virtual` (consigliato) o `path` |

## Chiavi di accesso

Crea un utente IAM dedicato con una chiave di accesso e una policy in sola lettura:

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject"],
    "Resource": ["arn:aws:s3:::NOME-BUCKET/*"]
  }]
}
```

Il gateway legge soltanto: basta il permesso di lettura degli oggetti (`s3:GetObject`) sul bucket o sulla cartella. → [Bucket S3](/guide/buckets-s3)

## Attenzioni

- **Regione giusta**: se l'endpoint o la regione non corrispondono al bucket, AWS risponde con un reindirizzamento (301) o con un errore di firma. Usa sempre l'endpoint regionale del bucket.
- **Path-style**: AWS lo supporta ancora in tutte le regioni, ma ne è previsto il ritiro (rinviato): preferisci `virtual`.
- **Virtual-host e nomi con punti**: con HTTPS il certificato jolly copre solo i bucket senza punti nel nome.
- Se il bucket è cifrato con una chiave KMS, l'utente IAM deve poter usare anche quella chiave.

## Problemi noti

Nessuno emerso dalla documentazione; non abbiamo però potuto provarlo dal vivo.

::: info Fonti ufficiali — verificato il 2026-09-26
- [Virtual hosting dei bucket](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html)
- [Endpoint e quote di S3](https://docs.aws.amazon.com/general/latest/gr/s3.html)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
