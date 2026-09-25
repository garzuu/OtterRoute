---
source_commit: dd1d866
---
# config.yaml

With the panel the file is **generated**: do not edit it by hand (the panel rewrites it). If you want to manage it yourself, do not use the panel for those resources. The node re-reads the file every `OTR_RELOAD_INTERVAL`; an invalid configuration is rejected and the last good one stays active.

Unknown fields are **errors**: a typo does not go unnoticed.

## Complete example

```yaml
version: 1
storages:
  - id: garage
    endpoint: http://127.0.0.1:3900
    region: garage
    addressing: path
    allow_private_endpoint: true
    credentials:
      access_key_env: GARAGE_KEY
      secret_key_env: GARAGE_SECRET
destinations:
  - id: photos
    storage: garage
    bucket: photos
    prefix: ""
    revision: 1
    cache_generation: 1
cache_policies:
  - id: standard
    ttl: 1h
    ttl_not_found: 60s
    serve_stale_on_error: 24h
routes:
  - id: cdn-photos
    host: cdn.example.com
    path_prefix: /photos
    strip_prefix: true
    destination: photos
    cache_policy: standard
```

## `version`

Integer; raise it at every change (the panel does it by itself). It appears in `otterroute_config_version`.

## `storages[]`

| Field | Default | Description |
|---|---|---|
| `id` | — | Unique name. |
| `endpoint` | — | URL of the S3 service. |
| `region` | `us-east-1` | Region for the SigV4 signature. |
| `addressing` | `path` | `path` (`endpoint/bucket/key`) or `virtual` (`bucket.endpoint/key`). |
| `allow_private_endpoint` | `false` | Allows endpoints on private or local addresses. |
| `credentials` | — | See below. |

### `credentials`

Only one of the two forms:

| Field | Description |
|---|---|
| `access_key_env` + `secret_key_env` | Names of environment variables that hold the keys. |
| `secret_file` | Path of a JSON file `{"access_key": "...", "secret_key": "..."}` (what the panel writes, `0600` permissions). |

## `destinations[]`

| Field | Default | Description |
|---|---|---|
| `id` | — | Unique name. |
| `storage` | — | `id` of a storage. |
| `bucket` | — | Bucket name. |
| `prefix` | empty | Folder in the bucket. |
| `revision` | `0` | Free number; raising it signals that the destination has changed. |
| `cache_generation` | `0` | Raising it **invalidates** this destination's cached copies. |

## `cache_policies[]`

| Field | Default | Description |
|---|---|---|
| `id` | — | Unique name. |
| `ttl` | — | Duration of a fresh copy. |
| `ttl_not_found` | `60s` | Duration of the negative cache. |
| `query_keys` | none | Query parameters that are part of the cache key. |
| `serve_stale_on_error` | `0` | How long to serve an expired copy if the storage is down. |

## `routes[]`

| Field | Default | Description |
|---|---|---|
| `id` | — | Unique name. |
| `host` | — | Domain. |
| `path_prefix` | — | Path prefix (`/` for the whole domain). |
| `strip_prefix` | `true` | Removes the prefix before looking for the file. |
| `destination` | — | `id` of a destination. |
| `cache_policy` | — | `id` of a policy. |

Durations: `30s`, `5m`, `1h`, `24h`, `7d`.
