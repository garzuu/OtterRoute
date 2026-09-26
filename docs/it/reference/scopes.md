# Scope e ruoli

| Scope | Cosa permette |
|---|---|
| `metrics:read` | Vedere le statistiche di traffico |
| `domains:read` | Vedere i domini |
| `domains:write` | Aggiungere, controllare ed eliminare domini |
| `buckets:read` | Vedere i bucket |
| `buckets:write` | Aggiungere, controllare ed eliminare bucket |
| `routes:read` | Vedere gli instradamenti e provare i file |
| `routes:write` | Creare ed eliminare instradamenti |
| `settings:write` | Modificare le impostazioni del nodo |
| `users:manage` | Gestire utenti, sicurezza e registro attività |
| `notifications:manage` | Configurare le notifiche email e Telegram |

Uno scope `x:write` include sempre `x:read`.

## Ruoli predefiniti

| Ruolo | Scope |
|---|---|
| **Amministratore** (`admin`) | tutti |
| **Operatore** (`operator`) | `metrics:read`, `domains:write`, `buckets:write`, `routes:write` (+ le letture implicite) |
| **Sola lettura** (`viewer`) | `metrics:read`, `domains:read`, `buckets:read`, `routes:read` |
| **Personalizzato** (`custom`) | quelli scelti |

## Scope richiesto per ogni azione dell'API

| Richiesta | Scope |
|---|---|
| `GET /api/panel` | nessuno (i dati mostrati dipendono dagli scope di lettura) |
| `GET /api/metrics` | `metrics:read` |
| `PUT /api/settings` | `settings:write` |
| `PUT /api/https`, `PUT /api/admin-host` | `settings:write` |
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
| `/api/me…` | nessuno (utente connesso) |
