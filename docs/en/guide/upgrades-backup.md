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
| `certs/` | HTTPS certificates and their keys (0600). They can be reissued, but not the ones uploaded by hand. |
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

OtterRoute **does not update itself**: the panel warns you when a new version exists and shows the steps for your type of installation; you do the update.

### Knowing whether there is a new version

The node checks **once a day** the public releases on GitHub (one request, sending nothing about the node). If it finds a newer one, an alert appears in the bell and a card in **Settings → Updates**, with notes and commands. From there you can also **Check now**, include test versions (pre-releases) or turn the check off. With `OTR_UPDATE_CHECK=off` the node never contacts GitHub (closed environments). After an update, for 24 hours an alert tells you which version you came from.

### Common steps

1. Make a backup of the state folder.
2. Replace the program with the new version (according to the installation type below) and restart.
3. Log in again: sessions do not survive a restart.

### Docker

The image is not replaced from the inside: you pull the new one and recreate the container **with the same volume**. → [Docker](./docker)

```sh
docker pull ghcr.io/garzuu/otterroute:0.1
docker stop otterroute && docker rm otterroute
# re-run the SAME "docker run" command as before (same ports, same /data volume)
```

### Binary or service (systemd, launchd)

Download from the [releases page](https://github.com/garzuu/OtterRoute/releases) the package for your platform, verify its checksum and replace the executable (and the `ui/` and `docs/` folders next to it):

```sh
sha256sum -c otterroute-vX.Y.Z-linux-x86_64.tar.gz.sha256
tar xzf otterroute-vX.Y.Z-linux-x86_64.tar.gz
sudo systemctl stop otterroute
sudo install -m 0755 otterroute-vX.Y.Z-linux-x86_64/otterroute /usr/local/bin/otterroute
sudo systemctl start otterroute
```

Keep the old executable (`otterroute.prev`) until you have seen the new version start correctly.

### From source

```sh
git pull && cargo build --release -p otterroute && (cd web && npm ci && npm run build)
```

**Automatic migrations:** if you upgrade from a version with a single administrator (`admin.json`), on first start the user is migrated to `users.json` with the same Administrator role and the same password; the old file becomes `admin.json.migrated`.

There is no automatic way back: if you need to go back, restore the backup made before the upgrade.

## If the node does not start

- **"no valid configuration available"**: the file indicated in `OTR_CONFIG` is not readable and `last-good.yaml` is missing. Restore the backup.
- **Port in use or not allowed**: check `OTR_LISTEN` and the privileges for port 80.
- Logs (`RUST_LOG=otterroute=info`) show which configuration is active and why a change was rejected.
