---
source_commit: dd1d866
---
# Cloudflare

How to make a domain managed on Cloudflare point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Dashboard → select the domain → **DNS** → **Records** → **Add record**.
2. Choose `A` (or `AAAA`), enter name and IP.
3. Decide the proxy state: **DNS only** (grey cloud) or **Proxied** (orange).

## Things to watch

- **Proxied (orange)**: DNS answers with Cloudflare's addresses, not your IP. Visitors reach Cloudflare, which forwards to the node. The panel's verification will not find the node and will stay on "another server answers": for verification use **DNS only** or ignore the warning. With the proxy, [HTTPS and proxy](/en/guide/https-proxy) applies too.
- **Apex**: Cloudflare allows a CNAME on the apex thanks to *CNAME flattening* (it resolves the chain and answers with the IPs).
- A CNAME to a name with no A/AAAA records produces empty answers, which look like a propagation problem.
- Cloudflare caches too: the CDN copy adds to OtterRoute's.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [CNAME flattening](https://developers.cloudflare.com/dns/cname-flattening/)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
