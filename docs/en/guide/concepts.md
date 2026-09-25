---
source_commit: dd1d866
---
# Concepts

A few words, always the same, throughout the panel and this guide.

## Domain

The DNS name visitors use to reach the files (`media.company.com`). A domain is **verified** when (1) DNS resolves it and (2) an HTTP request going through that name really reaches *this* node. Only verified domains can be used in routes. → [Domains and DNS](./domains-dns)

## Bucket

An S3 storage with its credentials **and** the name of the bucket to read from. In the panel a "bucket" gathers endpoint, region, addressing, keys and bucket name; keys are never shown again after saving. → [S3 buckets](./buckets-s3)

## Route

The rule that links a **domain + a path prefix** to a **bucket + a folder**. With the route `media.company.com` + `/docs/` → bucket `documents`, folder `public/`, the request `media.company.com/docs/prices.pdf` reads `documents/public/prices.pdf`. → [Routes](./routes)

## Cache policy

How long a copy is fresh (`ttl`), how long a missing file is remembered (`ttl_not_found`), how long expired copies are served if the storage is down (`serve_stale_on_error`) and which query parameters matter (`query_keys`). The panel creates a `standard` policy: **1 hour**, **60 seconds**, **24 hours**, no query parameters. → [Cache](./cache)

## Configuration and version

The gateway works on a `config.yaml` file with a `version` that increases at every change. The panel **regenerates it entirely** from the data it manages (domains, buckets, routes) and applies it immediately; if you write the file by hand the panel recognizes it and leaves it alone (`hand_managed`). → [config.yaml](/en/reference/config)

## Node

An OtterRoute instance with its own cache and state folders. Each node has an identity (`node-id`) with which it proves, during verification, that it is the one answering on a domain.

## Users, roles, scopes

Who accesses the panel and what they can do: a **role** (Administrator, Operator, Read-only, Custom) is a set of **scopes**, that is, of allowed actions. → [Users, permissions and 2FA](./users-2fa)
