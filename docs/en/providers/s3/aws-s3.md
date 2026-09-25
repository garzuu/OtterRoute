---
source_commit: dd1d866
---
# AWS S3

## Parameters in the panel

| Field | Value |
|---|---|
| Endpoint | `https://s3.REGION.amazonaws.com` (e.g. `https://s3.eu-west-1.amazonaws.com`) |
| Region | the bucket's (`eu-west-1`, `us-east-1`…) |
| Addressing | `virtual` (recommended) or `path` |

## Access keys

Create a dedicated IAM user with an access key and a read-only policy:

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject"],
    "Resource": ["arn:aws:s3:::BUCKET-NAME/*"]
  }]
}
```

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- **Right region**: if the endpoint or region do not match the bucket, AWS answers with a redirect (301) or a signature error. Always use the bucket's regional endpoint.
- **Path-style**: AWS still supports it in all regions, but its retirement is planned (postponed): prefer `virtual`.
- **Virtual-host and names with dots**: with HTTPS the wildcard certificate covers only buckets without dots in the name.
- If the bucket is encrypted with a KMS key, the IAM user must be able to use that key as well.

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [Virtual hosting of buckets](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html)
- [S3 endpoints and quotas](https://docs.aws.amazon.com/general/latest/gr/s3.html)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
