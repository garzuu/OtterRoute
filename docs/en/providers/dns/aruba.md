---
source_commit: dd1d866
---
# Aruba

How to make a domain managed on Aruba point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Log in to the management panel (admin.aruba.it) → the domain → **DNS management**.
2. **Add record**, choose the type (`A`, `AAAA`, `CNAME`), enter host and target, TTL.
3. Confirm and save the configuration. You can also schedule activation for a future date.

## Things to watch

- **Propagation**: Aruba states about 24 hours.
- You can have different TTLs for different hosts of the same domain.
- **Apex**: the sources consulted do not show an alias/flattening record type: on the apex use `A`/`AAAA`.
- Aruba's guides vary between hosting, domains and Aruba Business: look for the one for your service.
- The panel uses Aruba's name servers only if the domain uses them.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [Managing a CNAME record (guide.aruba.it)](https://guide.aruba.it/hosting-e-domini/gestione-dns/gestione-name-server-e-record/gestire-record-cname)
- [Guide to Aruba's DNS panel](https://www.aruba.it/magazine/hosting/la-gestione-del-pannello-dns-di-aruba.aspx)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
