---
source_commit: dd1d866
---
# Azure DNS

How to make a domain managed on Azure DNS point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Portal → **DNS zones** → your zone → **Record sets → Add**.
2. Name, type `A`/`AAAA`/`CNAME`, TTL, IP address.
3. Save.

## Things to watch

- **Apex**: DNS does not allow a CNAME on the apex. Azure offers **Alias** records, but towards Azure resources (public IP, Traffic Manager, CDN, Front Door, another record in the zone). Towards an external node use an `A` record with its IP.
- If the node has a Standard-type Azure public IP, an A/AAAA record of *alias* type follows it when the IP changes.
- To create alias records the **Microsoft.Network** resource provider must be registered.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [Alias records in Azure DNS](https://learn.microsoft.com/en-us/azure/dns/dns-alias)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
