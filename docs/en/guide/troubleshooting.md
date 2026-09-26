---
source_commit: dd1d866
---
# Troubleshooting

Look for the message you see in the panel or in the response. If you cannot find it, the [Diagnosis](./diagnosis) page follows a file's path and says where it stops.

## Domains

| Message / state | Cause | What to do |
|---|---|---|
| **DNS error** — *No A/AAAA record found for …* | The domain has no records to the node, or it has not propagated yet. | Create the record ([Domains and DNS](./domains-dns)); wait up to the TTL (sometimes 48 hours); the panel rechecks by itself every 5 minutes. |
| **Node unreachable** | The domain resolves, but nobody answers on port 80 of the address. | Check firewall, port forwarding and that the node listens on port 80 (`OTR_LISTEN`). |
| **Node unreachable** — *On … another server/service answers* | Another server is on the address (a proxy, a CDN, the old site). | Point the record to the right node, or let the `/.well-known/otterroute/check` path through to the node. |
| Verified, but it does not open from the browser | Your computer resolves the name differently (DNS cache, `/etc/hosts`) or you use HTTPS. | `dig +short yourdomain`; try `curl -H 'Host: …' http://IP/`. HTTPS is not served by the node: [HTTPS and proxy](./https-proxy). |
| The `.local` domain does not open | `.local` is reserved for mDNS. | Use `.localhost` or a line in `/etc/hosts`. |

## Buckets

| Message | Cause | What to do |
|---|---|---|
| *The storage rejected the credentials or the request (…)* | Wrong key, insufficient permissions, wrong region or endpoint. | Recheck keys, region and addressing style ([S3 buckets](./buckets-s3)); look for the code (`SignatureDoesNotMatch`, `AccessDenied`, `AuthorizationHeaderMalformed`…). |
| *Storage unreachable: …* | Network, endpoint DNS, TLS, port or firewall. | Try from the node: `curl -I https://ENDPOINT`. |
| Endpoint rejected (private address) | The endpoint is `localhost` or on a private network. | Tick *"The storage is on a local network"* if intended ([Security](./security)). |
| The bucket goes into error after a while | Expired or revoked key, or storage down. | Check the key; the panel periodically rechecks buckets and reports the error in the alerts icon. |

## Routes and files

| Symptom | Cause | What to do |
|---|---|---|
| `404` on a file that exists | Wrong prefix or folder; the path prefix is removed before the lookup. | Use the panel's test ([Routes](./routes)); check folder and name. |
| `404` from a domain | No route for that host, or the prefix does not match. | Create the route; the longest prefix wins. |
| A correct file still gives `404` | Negative cache (60 seconds). | Wait a minute. |
| The file does not update | The copy is still fresh (1 hour). | Wait, or increase `cache_generation` ([Cache](./cache)). |
| `403` with `?exp=&sig=` in the address | The route requires [signed links](./signed-links) and the link is expired, tampered with or issued with a key that was later rotated. | Create a new link; the [Diagnosis](./diagnosis) page says which of the three it is. |
| `400` with `?w=` or `?fmt=` | A parameter of [images on the fly](./images) is not valid (`w`/`h` 1–4096, `q` 30–95, `fmt` webp/jpeg/png/auto). | Fix the value. |
| `422` on an image | The file is not a valid image, is damaged or exceeds the limits (64 million pixels). | Check the original in the storage. |
| `405` / `400` | The gateway accepts only `GET` and `HEAD` (405); paths with `..` or control characters are rejected (400). | Use a normal path. |
| `X-Cache: STALE` | The storage does not answer and an expired copy is served. | Restore the storage. |

## Panel and access

| Message | Cause | What to do |
|---|---|---|
| *permission denied: scope … required* | Your role does not include the action. | Ask an administrator ([Users](./users-2fa)). |
| *too many attempts: try again in N minutes* | 5 errors in 5 minutes. | Wait. |
| *wrong code* | Wrong or already used 2FA code, clock off. | Synchronize the phone's time; wait for the next code or use a recovery code. |
| Lost password/2FA (last administrator) | — | `otterroute --reset-user NAME` on the node. |
| The panel does not open | Port 9090 listens only on `127.0.0.1`. | Use an SSH tunnel: `ssh -L 9090:127.0.0.1:9090 server`. |
| Login required again after restart | Sessions are in memory. | Normal. |

## Notifications

| Symptom | What to do |
|---|---|
| Send test: *SMTP: …* | Check server, port and security (STARTTLS 587, TLS 465); user and password; sometimes an app password is needed ([Notifications](./notifications#email-smtp)). |
| Send test: *chat not found* / *Unauthorized* | Wrong chat id, the bot never received a message from that chat, or wrong token ([Notifications](./notifications#telegram)). |
| Nothing arrives after a failure | The problem must last beyond the delay (default 10 minutes) and meet the channel's threshold. |

## HTTPS certificates

| Symptom | What to do |
|---|---|
| Certificate in error: *the CA did not validate the domain* | The domain must reach this node on **port 80 from the Internet**; a firewall, a proxy or Cloudflare make the challenge fail ([Automatic HTTPS](./https)). |
| The browser says "invalid certificate" but the domain is *Valid* | The **staging** environment is on: turn it off in Settings → HTTPS. |
| The site does not open on HTTPS but the certificate exists | The node is not listening on 443 (port in use or permissions): look at Settings and the log; with Docker map `-p 443:443`. |
| After enabling the redirect the site does not open | A proxy in front redirects in turn, in a loop: disable the redirect on one of the two. |

## Startup

| Symptom | What to do |
|---|---|
| *permission denied* on port 80 | Run as root, use `setcap 'cap_net_bind_service=+ep'`, or change `OTR_LISTEN`. |
| *address already in use* | Another process uses the port: `ss -ltnp \| grep :80`. |
| The node starts but the panel is empty | `OTR_UI_DIR` is missing (default `./web/dist`): build the panel or use the Docker image. |

## Collecting information

```sh
RUST_LOG=otterroute=debug otterroute
curl -s  http://127.0.0.1:9090/healthz
curl -s  http://127.0.0.1:9090/metrics | head
```

Also look at the **alerts** (bell) and the **activity log**.
