---
source_commit: dd1d866
---
# MinIO and Garage (self-hosted)

## Parameters in the panel

| Field | MinIO | Garage |
|---|---|---|
| Endpoint | `http://host:9000` | `http://host:3900` |
| Region | `us-east-1` | `garage` |
| Addressing | `path` | `path` |
| Local network | tick the box if the endpoint is private | same |

## Access keys

**MinIO**: create a user with a read-only policy:

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject"],
    "Resource": ["arn:aws:s3:::*/*"]
  }]
}
```

`mc admin policy create ALIAS readonly-get policy.json`, then assign it to the user. **Garage**: `garage key create application-name`, then allow reading on the bucket with `garage bucket allow --read BUCKET --key application-name`.

The gateway only reads: permission to read objects (`s3:GetObject`) on the bucket or folder is enough. → [S3 buckets](/en/guide/buckets-s3)

## Things to watch

- The gateway has been tested live only against a local fake S3 with SigV4 signing (e2e tests): this sheet has not been tested on a real MinIO or Garage installation.
- Endpoint on `localhost` or a private network: **tick** *The storage is on a local network*, otherwise it is rejected ([Security](/en/guide/security)).
- With Garage, the documentation recommends `path-style`.
- If you put a proxy with HTTPS in front, use the proxy's URL as the endpoint.

## Known issues

None emerged from the documentation; however we could not test it live.

::: info Official sources — verified on 2026-09-26
- [MinIO: creating a policy](https://docs.min.io/enterprise/aistor-object-store/reference/cli/admin/mc-admin-policy/mc-admin-policy-create/)
- [Garage: connecting applications](https://garagehq.deuxfleurs.fr/documentation/connect/apps/)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
