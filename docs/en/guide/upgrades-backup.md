---
source_commit: dd1d866
---
# Upgrades and backup

## What to save

Everything that matters is in the **state folder** (`OTR_STATE_DIR`, `/data/state` in the Docker image):

| Item | Notes |
|---|---|
| `users.json` | Users, roles, 2FA. `0600` permissions. |
| `panel.json` | Domains, buckets, routes, settings. |
| `secrets/` | Bucket keys. `0600` permissions. |
| `last-good.yaml` | The last valid configuration (the node uses it if the indicated file is unavailable). |
| `node-id` | The node's identity. |
| `notify.json`, `notify-state.json`, `notify-log.json` | [Notification](./notifications) settings, already-notified problems, latest sends. |
| `metrics.json`, `audit.jsonl` | Statistics and activity log. |

The `config.yaml` file (`OTR_CONFIG`) is **generated** by the panel from `panel.json`: it can be regenerated, but keeping a copy costs nothing.

The **cache** (`OTR_CACHE_DIR`) on the other hand should not be saved: it rebuilds itself.

## Making a backup

Files are written atomically, so you can copy the folder even with the node running. For a 100% consistent copy stop it for a moment.

```sh
tar czf otterroute-state-$(date +%F).tgz -C /var/lib/otterroute state config.yaml
chmod 600 otterroute-state-*.tgz
```

The backup contains keys and hashes: keep it encrypted and with restricted permissions.

## Restoring

1. Stop the node.
2. Restore the state folder (and `config.yaml`) with the same permissions.
3. Restart. The node restarts with users, domains, buckets and routes; sessions are lost and visitors rebuild the cache.

## Upgrading

1. Make a backup.
2. Replace the binary (or the image) and restart.
3. Log in again: sessions do not survive a restart.

**Automatic migrations:** if you upgrade from a version with a single administrator (`admin.json`), on first start the user is migrated to `users.json` with the same Administrator role and the same password; the old file becomes `admin.json.migrated`.

There is no automatic way back: if you need to go back, restore the backup made before the upgrade.

## If the node does not start

- **"no valid configuration available"**: the file indicated in `OTR_CONFIG` is not readable and `last-good.yaml` is missing. Restore the backup.
- **Port in use or not allowed**: check `OTR_LISTEN` and the privileges for port 80.
- Logs (`RUST_LOG=otterroute=info`) show which configuration is active and why a change was rejected.
