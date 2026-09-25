---
source_commit: dd1d866
---
# Providers

OtterRoute depends on two external services: **DNS**, which brings visitors to the node, and **S3 storage**, from which the node reads files. Each sheet explains how to configure them for OtterRoute.

## How to read the sheets

They are written from the providers' **official documentation**, summarized and with links to the sources and the verification date. We have not tested them with real accounts: the "Testing" column says so openly.

## DNS

| Provider | CNAME on apex? | Testing |
|---|---|---|
| [Cloudflare](./dns/cloudflare) | Yes (CNAME flattening) | Documentation |
| [AWS Route 53](./dns/route53) | No; alias only towards AWS resources | Documentation |
| [Google Cloud DNS](./dns/google-cloud-dns) | No | Documentation |
| [Azure DNS](./dns/azure-dns) | No; alias only towards Azure resources | Documentation |
| [OVHcloud](./dns/ovhcloud) | Not documented in the source | Documentation |
| [Aruba](./dns/aruba) | Not documented in the sources | Documentation |

For a fixed IP a plain `A`/`AAAA` record works everywhere.

## S3 storage

| Provider | Endpoint | Region | Addressing | Testing |
|---|---|---|---|---|
| [AWS S3](./s3/aws-s3) | `s3.REGION.amazonaws.com` | the bucket's | virtual (path still supported) | Documentation |
| [Cloudflare R2](./s3/cloudflare-r2) | `ACCOUNT.r2.cloudflarestorage.com` | `auto` | path | Documentation |
| [Backblaze B2](./s3/backblaze-b2) | `s3.REGION.backblazeb2.com` | same as endpoint | path or virtual | Documentation |
| [Wasabi](./s3/wasabi) | `s3.REGION.wasabisys.com` | the region's | virtual or path | Documentation |
| [DigitalOcean Spaces](./s3/digitalocean-spaces) | `REGION.digitaloceanspaces.com` | datacenter | virtual | Documentation |
| [Hetzner](./s3/hetzner) | `LOCATION.your-objectstorage.com` | the location | virtual | Documentation |
| [MinIO / Garage](./s3/minio-garage) | your host | `us-east-1` / `garage` | path | Local fake S3 (SigV4) |

Your provider is not there? Any S3-compatible service with SigV4 signing works: look in its documentation for endpoint, region and addressing style and fill in the bucket form.
