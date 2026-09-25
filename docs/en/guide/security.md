---
source_commit: dd1d866
---
# Security

What OtterRoute protects, what you have to protect.

## The model

OtterRoute serves **read-only** files from private buckets. Whoever visits has no credentials: the node has them. The main risk is therefore that someone uses the node (or its keys) to read what they should not, or to reach internal networks.

## What the gateway does

- **Read-only.** Only `GET` and `HEAD`; no writes to the storage.
- **Clean paths.** `.`/`..`, encoded slashes (`%2F`, `%5C`) and control characters are rejected: no "path traversal" outside the route's folder.
- **No query to the storage.** The query string is not forwarded and `x-amz-*` headers do not reach the visitor.
- **Internal endpoints blocked.** A storage on private addresses (`localhost`, `10.x`, `192.168.x`, `172.16–31.x`, link-local, `100.64.0.0/10`, IPv6 `fc00::/7`…) is allowed **only** with the *"The storage is on a local network"* checkbox. The check also applies to addresses **resolved by DNS**: a public name pointing to `127.0.0.1` does not pass. So the gateway does not become a proxy into your internal network.
- **Credentials on the node.** Bucket keys are in files with `0600` permissions, never in `config.yaml`, never in API responses.
- **Separate cache copies** per route.

## What you have to do

1. **Do not expose port 9090.** The panel and `/metrics` are meant for `localhost`. To work remotely use an SSH tunnel or a VPN.
2. **Keys with minimum permissions.** A dedicated, read-only key limited to the published bucket (or folder). → [S3 buckets](./buckets-s3)
3. **HTTPS in front of the node**, because plain HTTP protects nothing in transit. → [HTTPS and proxy](./https-proxy)
4. **Mandatory 2FA** at least for administrators. → [Users, permissions and 2FA](./users-2fa)
5. **Back up the state folder**, with the same restricted permissions. → [Upgrades and backup](./upgrades-backup)
6. **Synchronized clock** (NTP): it is needed for signing requests to the storage and for 2FA codes.
7. **Upgrade** the node when new versions come out.

## Sensitive data in the state folder

| File | Contains |
|---|---|
| `secrets/*.json` | Bucket keys, SMTP password and Telegram token (0600 permissions). |
| `users.json` | Password hashes, 2FA secrets, recovery code hashes (0600). |
| `panel.json`, `last-good.yaml` | Configuration (without keys). |
| `audit.jsonl` | Who did what (without secrets). |
| `metrics.json` | Statistics. |

2FA secrets are stored in clear in the file, like bucket keys: protect the folder with system permissions and disk encryption.

## Limits to know

- Sessions live in memory: a restart disconnects everyone.
- The lock after too many errors is per user, not per address: behind a proxy the client address is not reliable.
- There is no e-mail recovery: only reset by an administrator or from the terminal.
- There is no SSO/LDAP or WebAuthn.
