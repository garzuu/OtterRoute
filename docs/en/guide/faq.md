---
source_commit: dd1d866
---
# Frequently asked questions

**Does OtterRoute also serve HTTPS?**
Not directly: put a proxy or CDN in front. → [HTTPS and proxy](./https-proxy)

**Can I use several domains on the same bucket?**
Yes: you create several routes (one per domain) to the same destination.

**Can a domain use several buckets?**
Yes, with different prefixes: `/photos` to one bucket, `/videos` to another. The longest prefix wins.

**Does the bucket have to be public?**
No, and that is the point: it stays private and the node reads it with its key, read-only.

**Can I write to the bucket through the gateway?**
No. Only `GET` and `HEAD`.

**Where are the keys stored?**
In `0600` files on the node, never in `config.yaml` nor in API responses.

**How much space does the cache use?**
Up to `OTR_CACHE_MAX_BYTES` (10 GiB); files larger than `OTR_CACHE_MAX_OBJECT_BYTES` (1 GiB) pass through without being saved.

**How do I empty a route's cache?**
Increase `cache_generation` in the destination; old copies are no longer used. → [Cache](./cache)

**Can I have several nodes?**
Each node is independent, with its own cache and its own panel. There is no shared controller.

**Can I expose the panel on the Internet?**
Not recommended. It listens on `127.0.0.1:9090`; use an SSH tunnel or a VPN. → [Security](./security)

**How do I upgrade?**
Backup, then replace the binary or image. → [Upgrades and backup](./upgrades-backup)

**Does it work with my S3 provider?**
With any S3 compatible with SigV4 and path or virtual-host addressing. → [Providers](/en/providers/)
