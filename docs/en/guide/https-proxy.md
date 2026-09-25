---
source_commit: dd1d866
---
# HTTPS and proxy

::: warning HTTPS is not built in
Today OtterRoute answers **only over HTTP** on port 80. Port 443 is reserved and visible in the settings, but the node does not serve it. For HTTPS put a proxy or CDN in front that terminates TLS and forwards traffic to the node.
:::

## Diagram

```text
Visitor ──HTTPS──▶ Proxy / CDN ──HTTP──▶ OtterRoute :80 ──▶ S3 storage
```

The proxy must **keep the original `Host` header**: OtterRoute chooses the route from the domain name.

::: tip Domain verification
The panel's verification contacts **directly** the addresses the domain resolves to, over HTTP on port 80, and expects the node's answer. If the domain resolves to a proxy that does not forward the `/.well-known/otterroute/check` path to the node, or that redirects everything to HTTPS, verification will show *"another server answers"*. The domain still works for visitors: let that path through over HTTP if you want the green tick.
:::

## Caddy

The simplest: it obtains and renews certificates by itself.

```text
cdn.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

With OtterRoute on a different port (`OTR_LISTEN=127.0.0.1:8080`) Caddy can take 80 and 443.

## nginx

```nginx
server {
    listen 443 ssl http2;
    server_name cdn.example.com;
    # ssl_certificate ... ; ssl_certificate_key ... ;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_buffering on;
    }
}
```

## Traefik

```yaml
http:
  routers:
    otterroute:
      rule: "Host(`cdn.example.com`)"
      entryPoints: [websecure]
      tls: { certResolver: letsencrypt }
      service: otterroute
  services:
    otterroute:
      loadBalancer:
        servers:
          - url: "http://127.0.0.1:8080"
```

(This block is a Traefik configuration, not an OtterRoute one.)

## Cloudflare in front of the node

With Cloudflare's orange proxy, visitors use HTTPS and Cloudflare forwards to the node. Keep in mind:

- Cloudflare forwards over HTTP to port 80 only with the **Flexible** SSL mode; in **Full** it contacts the origin over HTTPS (which the node does not serve).
- The domain's DNS resolves to Cloudflare's addresses, not to the node: the panel's verification will not find the node and will stay on "another server answers". For verification set the record to **DNS only** (grey cloud), or ignore the warning.
- Cloudflare caches too: see also the [Cloudflare](/en/providers/dns/cloudflare) sheet.

## What the node does not do

It does not generate certificates, does not read `X-Forwarded-*`, does not redirect from HTTP to HTTPS: all this is up to the proxy.
