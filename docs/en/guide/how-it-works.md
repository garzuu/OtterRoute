---
source_commit: dd1d866
---
# How it works

OtterRoute is a **read-only gateway** in front of one or more S3-compatible buckets. It receives visitors' HTTP requests, works out which bucket and folder to serve the file from and returns it, keeping a copy on disk for later requests.

## The path of a request

```text
visitor ──► DNS ──► [proxy / CDN / load balancer] ──► OtterRoute :80
                                                          │
                        1. host + path → route (longest prefix wins)
                        2. fresh copy in cache?  ── yes ──► response (X-Cache: HIT)
                                   │ no
                        3. signed request (SigV4) to the bucket ──► S3
                        4. the file reaches the visitor while it is written to cache
                                                          (X-Cache: MISS)
```

1. **Routing.** The request host (`img.company.com`) and the start of the path (`/photos/`) select a *route*. If several rules match, the longest prefix wins, compared by whole segments: `/docs/` does not catch `/docsx/`.
2. **Cache.** If a fresh copy exists on disk the response starts immediately. Expired copies are revalidated with the storage (`304`) and, if the storage does not answer, they can still be served for a while.
3. **Storage.** Otherwise OtterRoute asks the bucket for the file with a signed request (AWS Signature v4, written from scratch) using the credentials of *that* bucket. Many simultaneous visitors cause **a single** request to the storage.
4. **Response.** The file is streamed while being saved to cache, without loading it in memory. The `X-Cache` header tells what happened: `HIT`, `MISS`, `STALE`, `REVALIDATED`, `BYPASS`.

## The parts of the system

| Part | Where | What it does |
|---|---|---|
| **Public gateway** | port **80** (HTTP) | Serves files to visitors. `443` (HTTPS) is reserved but not served yet: see [HTTPS and proxy](./https-proxy). |
| **Panel and API** | port **9090**, `localhost` only | Domains, buckets, routes, users, statistics. Never expose it. |
| **Cache** | folder on disk | Copies of files, with a space limit (LRU). |
| **State** | folder on disk | Users, generated configuration, credentials, statistics, activity log. |

The panel is not an accessory: it **generates** the gateway configuration (`config.yaml`) from the domains, buckets and routes you enter, validates it and applies it immediately, with no restart. If the new configuration is invalid the previous one stays active.

## What it does and does not do

- ✅ Serves files from private buckets on several domains, with cache, `ETag`, `Range`, `If-None-Match`, `If-Modified-Since`.
- ✅ Verifies that domains really reach the node and rechecks over time.
- ✅ Shows statistics and exposes them to Prometheus.
- ❌ **Read-only**: it accepts `GET` and `HEAD`, no writes to the storage.
- ❌ **No built-in HTTPS**: put a proxy in front (Caddy, nginx, Traefik, Cloudflare).
- ❌ **No folder listing**: `/folder/` answers `404`, there is no automatic `index.html`.
- ❌ The **query string never reaches the storage** and is not part of the cache key, except for the parameters a cache policy allows.

::: tip Next step
To try it out, go to [Installation](./install) and then [First run](./first-run). If you already have a running node and are missing DNS or storage, open the [sheet for your provider](/en/providers/).
:::
