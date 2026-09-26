---
source_commit: dd1d866
---
# Panel over HTTPS

By default the panel answers **only on the local port** (`127.0.0.1:9090`) and you reach it with an SSH tunnel. If you prefer to use it from a browser without a tunnel, you can serve it also over **HTTPS on a domain of the node**, with the same certificate and the same port 443 as your sites.

::: warning The panel becomes reachable from the Internet
You decide to expose it. Before enabling it: long passwords, [mandatory two-step verification](./users-2fa) (Users → Security) and, if you can, an unpredictable domain. The lock after 5 errors in 5 minutes protects from repeated attempts, but it does not replace 2FA.
:::

## What you need

- A **domain dedicated** to the panel (for example `admin.example.com`), registered in *Domains*. It must have no routes: the panel and files are not mixed.
- A **certificate in use** for that domain: [automatic](./https) or uploaded by hand. Without a certificate activation is refused, because the panel would not open.
- The node **listening for HTTPS** (`OTR_HTTPS_LISTEN`, by default 443) and the port reachable from outside.

## Enabling it

**Settings → Panel over HTTPS**: choose the domain (only suitable ones appear) and press *Enable*. The node shows the address to open and, if 2FA is not mandatory, a warning.

From a terminal, with the same access as before:

```sh
curl -s -b /tmp/otr.jar -H 'Content-Type: application/json' -X PUT \
  http://127.0.0.1:9090/api/admin-host -d '{"host":"admin.example.com"}'
```

To disable it: `-d '{"host":null}'` or *Disable* in Settings.

## How it behaves

- On `https://admin.example.com/` the panel answers; the rest of your domains do not change.
- HTTP for that domain redirects to HTTPS (`308`): the panel is **never served in clear**. Certificate challenges and domain verification stay on HTTP, so issuance and renewal keep working.
- The session cookie becomes **`Secure`**, plus `HttpOnly` and `SameSite=Strict`.
- The local port `127.0.0.1:9090` keeps working, independently of this setting: if the certificate expires or something breaks, you are not locked out.
- You cannot delete the domain or add routes to it while the panel is active on it.

## Behind Cloudflare

With the orange cloud, set SSL/TLS to **Full** (or Full strict with an origin certificate). In **Flexible** Cloudflare talks to the node over HTTP and the panel would redirect forever. The node does not use `X-Forwarded-*` headers.

## Limits

- There is no (yet) allow-list of addresses: whoever reaches the domain sees the login page. To restrict access use the server's firewall or Cloudflare Access.
- Only one domain can serve the panel.
- An expired certificate makes the panel unreachable on that domain: use the local tunnel to recover.
