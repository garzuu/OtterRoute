---
source_commit: dd1d866
---
# DigitalOcean Spaces

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://REGION.digitaloceanspaces.com` (e.g. `nyc3`) |
| Region | the datacenter's (`nyc3`, `ams3`…) |
| Addressing | `virtual` (as the documentation indicates) |

## Access keys

From the **API → Spaces Keys** panel create a key; if available, with limited (read-only) access to the bucket.

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- The SDK documentation uses `us-east-1` as the region in clients for AWS requirements, while the real location is given by the endpoint. In the gateway the region is part of the signature: if you get signature errors, try both the datacenter code and `us-east-1`.
- Presigned URLs do not work with the Spaces CDN; the gateway does not use them.

## Known issues

To verify: the region to use in the signature (`nyc3` or `us-east-1`) is not clear from the documentation alone; try it with the panel's **Test connection**.

::: info Official sources — verified on 2026-09-26
- [Using AWS SDKs with Spaces](https://docs.digitalocean.com/products/spaces/how-to/use-aws-sdks/)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
