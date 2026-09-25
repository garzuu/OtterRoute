---
source_commit: dd1d866
---
# Cloudflare R2

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://ACCOUNT_ID.r2.cloudflarestorage.com` |
| Region | `auto` |
| Addressing | `path` |

## Access keys

Dashboard → **R2 → Manage API tokens** → create an S3 token with **Object Read only** permission, limited to the bucket. You get *Access Key ID* and *Secret Access Key* (the secret is shown only once).

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- The documentation gives `auto` as the region; `us-east-1` and the empty value are treated as `auto`.
- For buckets with a jurisdiction (EU, FedRAMP) the endpoint has a different format: check the bucket page.
- GetObject and HeadObject support `Range` requests.
- `x-amz-request-payer` and `x-amz-expected-bucket-owner` are not supported: the gateway does not use them.

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [R2: S3 API compatibility](https://developers.cloudflare.com/r2/api/s3/api/)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
