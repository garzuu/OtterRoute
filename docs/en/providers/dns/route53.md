---
source_commit: dd1d866
---
# AWS Route 53

How to make a domain managed on AWS Route 53 point to your OtterRoute node.

## What to create

In the OtterRoute panel the domain shows you the records to create. In essence:

| Type | Name | Value |
|---|---|---|
| `A` | `cdn` (or the chosen subdomain) | the public IPv4 of the node (or of your proxy) |
| `AAAA` | same | the IPv6, if you use it |
| `CNAME` | `cdn` | a name that already points to the node (subdomains only, never the apex) |

## Steps

1. Route 53 console → **Hosted zones** → your zone → **Create record**.
2. Type `A` (or `AAAA`), name, value = node IP, TTL as you like.
3. Save.

## Things to watch

- **Apex**: you cannot create a CNAME on the apex. Route 53 offers **Alias** records, but only towards AWS resources (CloudFront, ELB, S3 buckets configured as websites, another record in the same zone…), **not** towards an arbitrary IP or domain. If the node is not on AWS, on the apex use an `A` record with its IP.
- An Alias record towards an AWS resource has no editable TTL: it uses the resource's.
- A CNAME answers for any query type; an Alias only if name and type match.

## Verifying

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Then press **Recheck** in the panel: verification runs exactly these two tests (DNS, then HTTP on port 80 with the node's proof). See [Domains and DNS](/en/guide/domains-dns) and [Troubleshooting](/en/guide/troubleshooting).

::: info Official sources — verified on 2026-09-26
- [Alias and CNAME in Route 53](https://docs.aws.amazon.com/Route53/latest/DeveloperGuide/resource-record-sets-choosing-alias-non-alias.html)

The sheet summarizes the provider's documentation on the date shown; details may change. **It has not been tested live with a real account.**
:::
