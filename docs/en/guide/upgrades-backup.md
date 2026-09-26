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

## Backup from the panel

**Settings → Backup and restore** produces a single `.otrbak` file with users (password hashes), bucket keys, certificates, domains, routes, notifications and the node identity; the cache is not included. The file is **encrypted** (AES-256-GCM, key derived from your passphrase with Argon2id): choose a passphrase of at least 12 characters and keep it **apart** from the file. Without it nothing can be recovered. It needs the `users:manage` scope (Administrator by default).

To restore, pick the file, enter the passphrase and press *Check the file*: the node shows the backup's version and date without changing anything. Then *Restore now*:

- the current state is copied to `state/backups/pre-restore/` (a single copy: the next restore replaces it);
- the node refuses backups made by a **newer version** (upgrade the node first) or with an unknown format; backups from older versions are accepted;
- the node **restarts** (same process) and sessions are lost: sign in with the credentials from the backup.

A restore replaces everything, including the node identity, so it also **moves** a node to another server. After moving, update DNS and, if the node has a new address, recheck the domains.

## Restoring by hand

1. Stop the node.
2. Restore the state folder (and `config.yaml`) with the same permissions.
3. Restart. The node restarts with users, domains, buckets and routes; sessions are lost and visitors rebuild the cache.

## Upgrading

The panel warns you when a new version exists and shows the steps for your type of installation. With a **binary or service** installation the node can also **update itself** (see below); with **Docker** the image is replaced from outside.

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

### Automatic update (binary and service)

If the node runs from an executable downloaded from the release (not Docker nor sources) and the service user can **write in the executable's folder**, **Update now** appears in **Settings → Updates**. From a terminal: `otterroute --self-update` (then restart the service) and `otterroute --check-update` to only learn whether a new version exists.

What it does, in order (if a step fails the installation stays as it was):

1. **Downloads** the package for your platform, only from GitHub, with a 100 MiB limit.
2. **Verifies** the **SHA-256** and the **Ed25519 signature**. The public key is inside the binary; the private one is in the repository's secrets and signs every release. A package without a valid signature is **refused**: manual update is always possible.
3. **Extracts** only the executable, `ui/` and `docs/` (no `..` paths, no symlinks) and **tests** the new executable: it declares the expected version and, started with temporary state and ports (`--self-check`), answers `/healthz`.
4. **Backs up** the state (users, configuration, keys, certificates) into `state/backups/<version>/`, keeping the last 3.
5. **Replaces** the executable, the panel and the guide, keeping the old ones as `otterroute.prev`, `ui.prev`, `docs.prev`.
6. **Restarts** itself with the same PID and the same arguments: a service sees no restart. Login sessions are lost.

**If the new version does not start well**: after the update the node counts starts; if **3 starts in a row get no confirmation** (60 seconds of regular operation) it restores the previous files by itself and restarts with the old version. The reason appears in the bell. Something must **restart the process** when it stops (systemd with `Restart=always` or `on-failure`). If the new executable refuses to start at all, the test in step 3 would already have found out before replacing it.

**Automatic, if you want it** (off by default): the *Apply patch versions by itself* box installs versions of the same series (0.1.x) in the time window you choose (default 03:00–05:00, node time). Minor and major versions stay manual, it does not start during a certificate issuance and never installs a pre-release.

::: warning What it does not do
- A **data schema** change is not undone by itself: if the new version has already migrated the data, to go back also restore the backup in `state/backups/`.
- There is no zero-downtime update: the node is down for a few seconds.
- The signature protects as much as the secret that produces it: if the signing key were compromised a manual release with a new key would be needed.
:::

The package can also be installed from a fork or an internal mirror: set `OTR_UPDATE_API` and the public key you sign with (`OTR_UPDATE_KEY`).

### Binary or service (systemd, launchd), by hand

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
