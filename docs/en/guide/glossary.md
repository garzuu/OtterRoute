---
source_commit: dd1d866
---
# Glossary

**Apex** — the domain without a subdomain (`example.com`). It does not allow a plain CNAME: see ALIAS.

**ALIAS / ANAME / CNAME flattening** — records that behave like a CNAME but on the apex; the name varies by provider.

**Bucket** — container of files on an S3 storage.

**Negative cache** — brief memory (`ttl_not_found`) of the fact that a file does not exist.

**Destination** — a bucket (with an optional prefix) reachable through a given storage.

**Domain** — public name that points to the node.

**Endpoint** — address of the S3 service (`https://s3.example.com`).

**Route** — rule *domain + prefix → destination + cache policy*.

**Node** — an OtterRoute installation.

**Path-style / virtual-host** — the two ways of addressing a bucket: `endpoint/bucket/key` or `bucket.endpoint/key`.

**Cache policy** — set of durations (`ttl`, `ttl_not_found`, `serve_stale_on_error`) and query parameters to consider.

**Verification proof** — the node's answer to `/.well-known/otterroute/check`, with which the panel makes sure the domain reaches the right node.

**Scope** — a permission on an action (`domains:write`); a role is a set of scopes.

**SigV4** — AWS's request-signing method, used by the gateway to talk to the storage.

**Storage** — an S3 service with its credentials.

**TOTP** — time-based 6-digit codes from the authenticator app (RFC 6238).

**TTL** — validity duration: for DNS, how long a record stays in resolver caches; for OtterRoute's cache, how long a copy is fresh.
