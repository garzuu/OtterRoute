# Provider

OtterRoute dipende da due servizi esterni: il **DNS**, che porta i visitatori al nodo, e lo **storage S3**, da cui il nodo legge i file. Ogni scheda spiega come configurarli per OtterRoute.

## Come leggere le schede

Sono scritte a partire dalla **documentazione ufficiale** dei provider, riassunta e con i link alle fonti e la data di verifica. Non le abbiamo provate con account reali: la colonna "Collaudo" lo dice apertamente.

## DNS

| Provider | Apex con CNAME? | Collaudo |
|---|---|---|
| [Cloudflare](./dns/cloudflare) | Sì (CNAME flattening) | Documentazione |
| [AWS Route 53](./dns/route53) | No; alias solo verso risorse AWS | Documentazione |
| [Google Cloud DNS](./dns/google-cloud-dns) | No | Documentazione |
| [Azure DNS](./dns/azure-dns) | No; alias solo verso risorse Azure | Documentazione |
| [OVHcloud](./dns/ovhcloud) | Non documentato nella fonte | Documentazione |
| [Aruba](./dns/aruba) | Non documentato nelle fonti | Documentazione |

Per un IP fisso funziona ovunque un semplice record `A`/`AAAA`.

## Storage S3

| Provider | Endpoint | Regione | Indirizzamento | Collaudo |
|---|---|---|---|---|
| [AWS S3](./s3/aws-s3) | `s3.REGIONE.amazonaws.com` | del bucket | virtual (path ancora supportato) | Documentazione |
| [Cloudflare R2](./s3/cloudflare-r2) | `ACCOUNT.r2.cloudflarestorage.com` | `auto` | path | Documentazione |
| [Backblaze B2](./s3/backblaze-b2) | `s3.REGIONE.backblazeb2.com` | come l'endpoint | path o virtual | Documentazione |
| [Wasabi](./s3/wasabi) | `s3.REGIONE.wasabisys.com` | della regione | virtual o path | Documentazione |
| [DigitalOcean Spaces](./s3/digitalocean-spaces) | `REGIONE.digitaloceanspaces.com` | datacenter | virtual | Documentazione |
| [Hetzner](./s3/hetzner) | `SEDE.your-objectstorage.com` | la sede | virtual | Documentazione |
| [MinIO / Garage](./s3/minio-garage) | il tuo host | `us-east-1` / `garage` | path | S3 finto locale (SigV4) |

Il tuo provider non c'è? Qualunque servizio compatibile S3 con firma SigV4 funziona: cerca nella sua documentazione endpoint, regione e stile d'indirizzamento e compila il modulo del bucket.
