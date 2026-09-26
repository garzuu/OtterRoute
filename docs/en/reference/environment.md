---
source_commit: dd1d866
---
# Environment variables and options

Every option can be given as a flag (`--cache-dir`) or as an environment variable. The flag takes precedence.

| Variable | Flag | Default | Description |
|---|---|---|---|
| `OTR_CONFIG` | `--config` | `config.yaml` | Configuration file. With the panel it is generated from `panel.json`. |
| `OTR_LISTEN` | `--listen` | `0.0.0.0:80` | Public HTTP address. |
| `OTR_HTTPS_LISTEN` | `--https-listen` | `0.0.0.0:443` | Public HTTPS address. Empty = HTTPS disabled. If the port is not available the node still starts (without HTTPS). |
| `OTR_ACME_DIRECTORY` | `--acme-directory` | — | ACME directory other than Let's Encrypt (for tests, e.g. Pebble). |
| `OTR_ACME_CA_ROOT` | `--acme-ca-root` | — | PEM file with the root CA of the alternative directory. |
| `OTR_UPDATE_CHECK` | `--update-check` | `on` | `off` turns off every request to GitHub looking for new versions. |
| `OTR_UPDATE_API` | `--update-api` | GitHub (official releases) | Releases API for forks or internal mirrors. |
| `OTR_INSTALL` | `--install` | — | How the node is installed (`docker` in the official image): decides the update instructions. |
| `OTR_ADMIN_LISTEN` | `--admin-listen` | `127.0.0.1:9090` | Panel, API, `/healthz` and `/metrics`. Keep it local. |
| `OTR_CACHE_DIR` | `--cache-dir` | `./data/cache` | Disk cache folder. |
| `OTR_CACHE_MAX_BYTES` | `--cache-max-bytes` | `10737418240` (10 GiB) | Maximum cache size. |
| `OTR_CACHE_MAX_OBJECT_BYTES` | `--cache-max-object-bytes` | `1073741824` (1 GiB) | Above this size a file is not saved in cache. |
| `OTR_STATE_DIR` | `--state-dir` | `./data/state` | Users, panel configuration, keys, statistics, log. |
| `OTR_DOMAIN_RECHECK` | `--domain-recheck` | `5m` | How often to recheck domains and buckets that are **not** verified. |
| `OTR_DOMAIN_RECHECK_VERIFIED` | `--domain-recheck-verified` | `15m` | How often to recheck the already verified ones. |
| `OTR_UI_DIR` | `--ui-dir` | `./web/dist` | Folder with the built panel. |
| `OTR_PUBLIC_URL` | `--public-url` | — | Address at which the panel can be reached: if set, notifications contain the link. |
| `OTR_DOCS_DIR` | `--docs-dir` | — | Folder with the built guide: if it exists, it is served by the panel port under `/docs/` (offline guide). |
| `OTR_DOCS_URL` | `--docs-url` | `https://garzuu.github.io/OtterRoute/` | Address of the online guide used by the panel's “Guide” links. Empty = no links. The offline copy takes precedence if present. |
| `OTR_RELOAD_INTERVAL` | `--reload-interval` | `2s` | How often to re-read the configuration file. |
| — | `--reset-user NAME` | — | Resets the user's password and 2FA and exits. |

Durations are written as `30s`, `5m`, `1h`, `24h`, `7d`.

In the Docker image `OTR_LISTEN`, `OTR_STATE_DIR` and `OTR_CACHE_DIR` are already set on the `/data` volumes.

## Logs

The level is controlled with `RUST_LOG`, for example `RUST_LOG=otterroute=debug`.

## Ports

The public HTTP/HTTPS ports shown in the panel's **Settings** are the ones used by checks and by firewall hints. The address the node actually listens on remains `OTR_LISTEN`.

| Port | Use | Note |
|---|---|---|
| 80 | Public HTTP traffic | Serves files and the verification proof. |
| 443 | HTTPS | Automatic or uploaded certificates. [Automatic HTTPS](/en/guide/https) |
| 9090 | Panel, API, metrics | `127.0.0.1` only by default. |
