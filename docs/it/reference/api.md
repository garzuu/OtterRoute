# API del pannello

L'API JSON è quella che usa il pannello, sulla porta di amministrazione (`127.0.0.1:9090`). Non è un'interfaccia stabile: può cambiare tra versioni.

Autenticazione: cookie di sessione ottenuto con `POST /api/login`. Le richieste con corpo devono usare `Content-Type: application/json` (protezione contro le richieste cross-origin).

## Senza login

| Richiesta | Descrizione |
|---|---|
| `GET /healthz` | `{"status":"ok"}` |
| `GET /metrics` | Metriche Prometheus. |
| `GET /api/session` | Stato della sessione, se serve il setup, criterio 2FA. |
| `POST /api/setup` | Crea il primo amministratore (solo se non esiste). |
| `POST /api/login`, `POST /api/login/2fa` | Accesso. |
| `POST /api/logout` | Uscita. |

## Con login

| Richiesta | Scope |
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
| `PUT /api/rules/{id}` (link firmati e immagini al volo), `POST /api/links` | `routes:write` |
| `POST /api/links/rotate` | `settings:write` |
| `POST /api/diagnose` | `routes:read` |
| `/api/users`, `/api/audit`, `/api/policy` | `users:manage` |
| `GET/PUT /api/notifications`, `POST /api/notifications/test` | `notifications:manage` |
| `/api/me`, `/api/me/password`, `/api/me/2fa/…` | — |

Il corpo di una richiesta non può superare 64 KB.

## Pagina di verifica pubblica

Sulla porta pubblica: `GET /.well-known/otterroute/check?nonce=…` risponde con una prova che il pannello usa per accertarsi che il dominio arrivi a questo nodo.
