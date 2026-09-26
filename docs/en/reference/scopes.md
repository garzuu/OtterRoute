---
source_commit: dd1d866
---
# Scopes and roles

| Scope | What it allows |
|---|---|
| `metrics:read` | See traffic statistics |
| `domains:read` | See domains |
| `domains:write` | Add, check and delete domains |
| `buckets:read` | See buckets |
| `buckets:write` | Add, check and delete buckets |
| `routes:read` | See routes and test files |
| `routes:write` | Create and delete routes |
| `settings:write` | Change node settings |
| `users:manage` | Manage users, security and activity log |
| `notifications:manage` | Configure email and Telegram notifications |

A scope `x:write` always includes `x:read`.

## Predefined roles

| Role | Scopes |
|---|---|
| **Administrator** (`admin`) | all |
| **Operator** (`operator`) | `metrics:read`, `domains:write`, `buckets:write`, `routes:write` (+ the implicit reads) |
| **Read-only** (`viewer`) | `metrics:read`, `domains:read`, `buckets:read`, `routes:read` |
| **Custom** (`custom`) | the chosen ones |

## Scope required for each API action

| Request | Scope |
|---|---|
| `GET /api/panel` | none (the data shown depends on the read scopes) |
| `GET /api/metrics` | `metrics:read` |
| `PUT /api/settings` | `settings:write` |
| `PUT /api/https`, `PUT /api/admin-host`, `PUT /api/updates`, `POST /api/update/check` | `settings:write` |
| `POST /api/domains`, `/api/domains/check`, `/api/domains/test`, `/api/domains/redirect` | `domains:write` |
| `POST /api/certs/issue`, `/api/certs/upload` | `domains:write` |
| `DELETE /api/domains/{host}` | `domains:write` |
| `POST /api/buckets`, `/api/buckets/check`, `/api/buckets/test` | `buckets:write` |
| `DELETE /api/buckets/{id}` | `buckets:write` |
| `POST /api/rules`, `DELETE /api/rules/{id}` | `routes:write` |
| `POST /api/probe` | `routes:read` |
| `POST /api/purge`, `POST /api/warm` | `routes:write` |
| `PUT /api/rules/{id}`, `POST /api/links` | `routes:write` |
| `POST /api/links/rotate` | `settings:write` |
| `POST /api/diagnose` | `routes:read` |
| `/api/users…`, `/api/audit`, `/api/policy` | `users:manage` |
| `GET/PUT /api/notifications`, `POST /api/notifications/test` | `notifications:manage` |
| `/api/me…` | none (logged-in user) |
