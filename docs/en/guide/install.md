---
source_commit: dd1d866
---
# Installation

OtterRoute is a single binary (`otterroute`) plus the panel folder. You need a machine with a public port (80) reachable by the domains you want to serve.

## Requirements

- Linux, macOS or a container. The gateway has no system dependencies.
- Disk space for the cache (default **10 GiB**, adjustable).
- An S3-compatible bucket with a **read-only** key (see [S3 buckets](./buckets-s3)).
- A domain whose DNS you can edit (see [Domains and DNS](./domains-dns)). To try it locally, `*.localhost` names are enough.

## With Docker

```sh
git clone https://github.com/garzuu/OtterRoute.git && cd OtterRoute
docker build -t otterroute .
docker run -d --name otterroute \
  -p 80:80 -p 127.0.0.1:9090:9090 \
  -v otterroute-data:/data \
  otterroute
```

- The panel answers on `http://127.0.0.1:9090` **from the machine only**: map it to `127.0.0.1`, not to all interfaces.
- The `/data` volume holds cache and state (users, credentials, configuration): keep it and [back it up](./upgrades-backup).
- The image starts as an unprivileged user. In the repository, `compose.yaml` sets `net.ipv4.ip_unprivileged_port_start=0` to let it use port 80; with `docker run` the same is achieved with `--sysctl net.ipv4.ip_unprivileged_port_start=0`.

## From a script (Linux and macOS)

The script downloads the release, verifies its **checksum and signature** and installs it under `/usr/local` (on Linux, as root, it also creates the user and the systemd service, with permissions that allow [automatic update](./upgrades-backup)):

```sh
curl -fsSL https://github.com/garzuu/OtterRoute/releases/latest/download/install.sh | sudo sh
# or a specific version:  sudo sh install.sh --version 0.1.0
```

Running it again updates an existing installation (it keeps the old executable as `otterroute.prev`). Afterwards: `sudo systemctl enable --now otterroute`.

## From source

```sh
cargo build --release -p otterroute        # gateway → target/release/otterroute
cd web && npm ci && npm run build          # panel   → web/dist
OTR_UI_DIR=web/dist ./target/release/otterroute
```

On first start, with no configuration, the node starts empty and listens on **80** (public traffic) and on `127.0.0.1:9090` (panel). On Linux port 80 needs privileges: give the binary the `cap_net_bind_service` capability (`setcap 'cap_net_bind_service=+ep' otterroute`) or use another port with `OTR_LISTEN` and a proxy in front.

## As a service (systemd)

```ini
# /etc/systemd/system/otterroute.service
[Unit]
Description=OtterRoute
After=network-online.target

[Service]
User=otterroute
ExecStart=/usr/local/bin/otterroute
Environment=OTR_UI_DIR=/usr/local/share/otterroute/ui
Environment=OTR_STATE_DIR=/var/lib/otterroute/state
Environment=OTR_CACHE_DIR=/var/lib/otterroute/cache
Environment=OTR_CONFIG=/var/lib/otterroute/config.yaml
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## Standard ports

| Port | Use | Changed with |
|---|---|---|
| **80** | Public HTTP (domains arrive here) | `OTR_LISTEN` for listening; **Settings → HTTP** for the port used in checks |
| **443** | HTTPS (with [automatic certificates](./https)) | `OTR_HTTPS_LISTEN` for listening; Settings → Ports for the port used in redirects |
| **9090** | Panel and API, localhost only | `OTR_ADMIN_LISTEN` |

All options are in the [environment variables reference](/en/reference/environment).

## Check that it is running

```sh
curl -s http://127.0.0.1:9090/healthz      # {"status":"ok"}
```

Then open `http://127.0.0.1:9090/` in the browser: it is the [first run](./first-run).
