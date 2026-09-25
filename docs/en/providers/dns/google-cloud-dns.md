---
source_commit: dd1d866
---
# Google Cloud DNS

How to make a domain managed on Google Cloud DNS point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Console → **Network services → Cloud DNS** → your zone → **Add standard**.
2. DNS name (leave empty for the apex), type `A`/`AAAA`/`CNAME`, TTL (required), data.
3. Create. With `gcloud`: `gcloud dns record-sets create cdn.example.com. --type=A --ttl=300 --rrdatas=IP --zone=ZONE`.

## Things to watch

- **Trailing dot**: names without the trailing dot are considered relative to the zone. In a CNAME always write the full name with the dot (`node.example.net.`), otherwise the value is extended with the zone's domain.
- **Apex**: CNAMEs are not allowed on the apex; use `A`/`AAAA`. The `@` symbol does not create the apex record: leave the name empty.
- The TTL must be given explicitly on every record.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [Adding records in Cloud DNS](https://docs.cloud.google.com/dns/docs/records)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
