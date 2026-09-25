---
source_commit: dd1d866
---
# S3 buckets

A bucket is an S3-compatible storage (AWS, Cloudflare R2, Backblaze B2, Wasabi, MinIO…) from which OtterRoute reads files. Add one from **Buckets → New bucket**.

## The fields

| Field | What to enter |
|---|---|
| **Name** | A label to recognize it. |
| **Endpoint** | Storage address only: scheme + host (+ port). **No** bucket, path, query or credentials in the URL. E.g. `https://s3.eu-central-1.amazonaws.com`. |
| **Region** | The region used to sign the request (`us-east-1` by default). It must match the bucket's; some providers use a fixed value (e.g. `auto`). |
| **Addressing** | *Path* or *Virtual host*, see below. |
| **Access key / Secret key** | A **read-only** key (see "Minimum permissions"). Keys stay on the node, in a file readable only by its owner, and are never shown again. |
| **Bucket** | The bucket name. |
| **Test file** | The path of a file that really exists in the bucket, e.g. `photos/boat.jpg`. Used to verify reading now and in later checks. |

The panel shows the *"The storage is on a local network"* checkbox only for internal addresses (see [Security](./security)).

## Path or virtual host

Two ways of writing the same URL:

| Style | Request URL | Typical of |
|---|---|---|
| **Path** | `https://ENDPOINT/BUCKET/key` | MinIO, Garage, Ceph, many self-hosted storages |
| **Virtual host** | `https://BUCKET.ENDPOINT/key` | AWS S3, Wasabi, DigitalOcean Spaces, Backblaze B2, Hetzner… |

If verification gives a signature or host-not-found error with one style, try the other. Each [provider sheet](/en/providers/#s3-storage) says which to use.

## Minimum permissions (read-only)

OtterRoute **never writes** to the storage. Create a dedicated key with only the permission to read the objects you must publish (`s3:GetObject` on the bucket or folder). `s3:ListBucket` is not needed:

- **without** ListBucket, a missing file makes many storages answer `403 AccessDenied`; OtterRoute turns it into `404` for the visitor;
- **with** ListBucket a missing file gives `404 NoSuchKey`. It works either way.

A key with more permissions than necessary is an extra risk if the node were compromised.

## How the file is found

The route adds the **folder** to the start of the path: with bucket `catalog` and folder `photos/`, the request `/boat.jpg` reads `photos/boat.jpg`. The path is normalized: `.` and `..` are rejected, `%2F` (an encoded slash) is rejected because it is ambiguous, multiple slashes are collapsed. A path ending in `/` is a "folder" and answers `404`.

## What can go wrong

| Message | Most likely cause |
|---|---|
| `403 SignatureDoesNotMatch` | Wrong secret key, wrong **region**, wrong addressing style or machine **clock** off by more than 15 minutes. |
| `403 InvalidAccessKeyId` | Wrong access key, or one from another provider/region. |
| "Storage unreachable" | Wrong endpoint, network or firewall, storage DNS does not resolve. |
| "the file is not accessible" | Wrong bucket, folder or test file name, or the key cannot read it. |
| "internal endpoint … requires allow_private_endpoint" | The endpoint is a local address: tick the dedicated checkbox. |

OtterRoute **does not follow storage redirects** (for example an AWS `301 PermanentRedirect` when the region is wrong): set the exact region.

All messages are explained in [Troubleshooting](./troubleshooting#buckets).
