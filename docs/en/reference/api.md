---
source_commit: dd1d866
---
# Panel API

The JSON API is the one the panel uses, on the admin port (`127.0.0.1:9090`). It is not a stable interface: it may change between versions.

Authentication: session cookie obtained with `POST /api/login`. Requests with a body must use `Content-Type: application/json` (protection against cross-origin requests).

## Without login

| Request | Description |
|---|---|
| `GET /healthz` | `{"status":"ok"}` |
| `GET /metrics` | Prometheus metrics. |
| `GET /api/session` | Session state, whether setup is needed, 2FA policy. |
| `POST /api/setup` | Creates the first administrator (only if none exists). |
| `POST /api/login`, `POST /api/login/2fa` | Login. |
| `POST /api/logout` | Logout. |

## With login

| Request | Scope |
|---|---|
| `GET /api/panel` | — |
| `GET /api/metrics?range=1h\|24h\|7d` | `metrics:read` |
| `PUT /api/settings` | `settings:write` |
| `PUT /api/https` | `settings:write` |
| `POST /api/domains`, `/api/domains/check`, `/api/domains/test`, `/api/domains/redirect` | `domains:write` |
| `POST /api/certs/issue`, `/api/certs/upload` | `domains:write` |
| `DELETE /api/domains/{host}` | `domains:write` |
| `POST /api/buckets`, `/api/buckets/check`, `/api/buckets/test` | `buckets:write` |
| `DELETE /api/buckets/{id}` | `buckets:write` |
| `POST /api/rules`, `DELETE /api/rules/{id}` | `routes:write` |
| `POST /api/probe` | `routes:read` |
| `POST /api/purge`, `POST /api/warm` | `routes:write` |
| `PUT /api/rules/{id}` (enable/disable signed links), `POST /api/links` | `routes:write` |
| `POST /api/links/rotate` | `settings:write` |
| `POST /api/diagnose` | `routes:read` |
| `/api/users`, `/api/audit`, `/api/policy` | `users:manage` |
| `GET/PUT /api/notifications`, `POST /api/notifications/test` | `notifications:manage` |
| `/api/me`, `/api/me/password`, `/api/me/2fa/…` | — |

A request body cannot exceed 64 KB.

## Public verification page

On the public port: `GET /.well-known/otterroute/check?nonce=…` answers with a proof the panel uses to make sure the domain reaches this node.
