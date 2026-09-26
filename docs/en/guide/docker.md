---
source_commit: dd1d866
---
# Docker

The official image is `ghcr.io/garzuu/otterroute` (public, for `linux/amd64` and `linux/arm64`). It contains the node, the panel and the [offline guide](./install) under `/docs/`.

## Tags

| Tag | What it follows |
|---|---|
| `0.1.0` | Exactly that version: never changes. |
| `0.1` | The latest fix of the 0.1 series (`0.1.1`, `0.1.2`…). **Recommended for production.** |
| `latest` | The latest stable version, including major changes: handy to try, risky in production. |

Test versions have a suffixed tag (`0.1.0-rc3`) and do not move `0.1` or `latest`.

## Starting

```sh
docker run -d --name otterroute --restart unless-stopped \
  -p 80:80 -p 443:443 -p 127.0.0.1:9090:9090 \
  --sysctl net.ipv4.ip_unprivileged_port_start=0 \
  -v otterroute-data:/data \
  ghcr.io/garzuu/otterroute:0.1
```

- `-p 127.0.0.1:9090:9090` keeps the **panel local only**: never publish 9090 on all interfaces.
- `--sysctl …=0` is needed because the process does not run as root and must use ports 80 and 443.
- The `/data` volume holds **everything** that matters (users, keys, certificates, configuration, cache): do not lose it.

With Compose (`compose.yaml` in the repository):

```yaml
services:
  gateway:
    image: ghcr.io/garzuu/otterroute:0.1
    restart: unless-stopped
    ports: ["80:80", "443:443", "127.0.0.1:9090:9090"]
    sysctls:
      net.ipv4.ip_unprivileged_port_start: 0
    volumes:
      - gateway-data:/data
volumes:
  gateway-data:
```

## Updating

The container does not update from the inside. The panel warns you about the new version ([Upgrades](./upgrades-backup)); then:

```sh
# 1. back up the volume (see Upgrades and backup)
# 2. pull the new version
docker pull ghcr.io/garzuu/otterroute:0.1
# 3. recreate the container with the same volume
docker stop otterroute && docker rm otterroute
docker run -d --name otterroute ...   # the same command as before
```

With Compose: `docker compose pull && docker compose up -d`. Data and configuration stay in the volume; migrations happen by themselves on first start; login sessions are lost. There is a downtime of a few seconds.

## Updating automatically (Watchtower)

If you want containers to update themselves you can use [Watchtower](https://containrrr.dev/watchtower/): it checks the registry and recreates the container when there is a new image for the tag you use.

```yaml
services:
  gateway:
    image: ghcr.io/garzuu/otterroute:0.1        # series tag, not latest
    labels:
      - com.centurylinklabs.watchtower.enable=true
  watchtower:
    image: containrrr/watchtower
    command: --label-enable --interval 86400   # once a day
    volumes: ["/var/run/docker.sock:/var/run/docker.sock"]
```

::: warning What you give up with automatic updates
Watchtower does not back up the volume nor can it go back if the new version has a problem. Use the series tag (`:0.1`), not `latest`, and back up the volume periodically. It gives a service access to the Docker socket: decide whether you are comfortable with that.
:::

## Going back

Restart with the previous version's tag (`ghcr.io/garzuu/otterroute:0.1.0`). If the new version has already migrated the data, also restore the volume backup made before the update.

## Checking the version

```sh
docker exec otterroute otterroute --version
```

or in the panel, in **Settings → Updates**.
