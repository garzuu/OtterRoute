---
source_commit: dd1d866
---
# Hetzner Object Storage

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://LOCATION.your-objectstorage.com` (`fsn1`, `nbg1`, `hel1`) |
| Region | the location code (e.g. `fsn1`) |
| Addressing | `virtual` |

## Access keys

In the console, inside the project, generate the S3 credentials. The secret **can no longer be recovered** after creation: save it right away.

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- Endpoints are tied to the **location**, credentials to the **project**: each location/project combination has its own bucket and keys.
- The official examples disable path-style (`path: off`): use `virtual`.

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [Using S3 tools with Hetzner](https://docs.hetzner.com/storage/object-storage/getting-started/using-s3-api-tools)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
