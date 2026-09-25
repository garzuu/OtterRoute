---
source_commit: dd1d866
---
# Backblaze B2

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://s3.REGION.backblazeb2.com` (e.g. `s3.us-west-004.backblazeb2.com`) |
| Region | the endpoint's (`us-west-004`, `us-east-005`…) |
| Addressing | `path` or `virtual` (both supported) |

## Access keys

From **Application Keys** create a read-only key limited to the bucket. *keyID* is the access key, *applicationKey* is the secret (shown only once). Keys with S3 access are needed.

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- The endpoint is found in the bucket details: it depends on where the bucket was created.
- B2 requires v4 signatures (v2 is not supported); the region is part of the signature, so it must match the endpoint's.

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [Backblaze B2 S3-compatible API](https://www.backblaze.com/docs/cloud-storage-s3-compatible-api)
- [Calling the S3 API](https://www.backblaze.com/docs/cloud-storage-call-the-s3-compatible-api)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
