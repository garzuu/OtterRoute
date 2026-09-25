---
source_commit: dd1d866
---
# Wasabi

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://s3.REGION.wasabisys.com` (us-east-1: `s3.wasabisys.com`) |
| Region | the region code (`eu-central-1`, `eu-south-1`…) |
| Addressing | `virtual` or `path` |

## Access keys

Wasabi console → **Access Keys** → create a key (better for a user with a read-only policy on the bucket).

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- The endpoint depends on the bucket's region: if you get it wrong you receive signature errors or redirects. Regions are listed on the official page (Americas, EMEA with aliases like `s3.it-1.wasabisys.com`, APAC).

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [Wasabi service URLs by region](https://docs.wasabi.com/docs/what-are-the-service-urls-for-wasabis-different-storage-regions)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
