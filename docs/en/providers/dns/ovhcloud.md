---
source_commit: dd1d866
---
# OVHcloud

How to make a domain managed on OVHcloud point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Customer area → **Web Cloud → Domains** → the domain → **DNS zone** tab → **Add an entry**.
2. Choose `A`/`AAAA`/`CNAME`, subdomain and target, TTL (default or custom).
3. Confirm.

## Things to watch

- **Trailing dot in CNAMEs**: if the target is a name, write it with the dot (`node.example.net.`); without it, OVHcloud appends your domain.
- **Propagation**: OVHcloud states up to 24 hours. To speed up future changes you can lower the default TTL from the *Actions on my zone* menu.
- If the domain uses name servers other than OVHcloud's, the zone here has no effect.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [Editing an OVHcloud DNS zone](https://docs.ovhcloud.com/en/guides/web-cloud/domains/dns-zone-edit)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
