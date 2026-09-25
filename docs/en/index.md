---
layout: home
title: OtterRoute
source_commit: dd1d866
hero:
  name: OtterRoute
  text: Your S3 buckets, on all your domains
  tagline: A self-hosted gateway that publishes files from several private buckets through several domains, with a disk cache. Connect a storage, attach a domain and the file is online.
  image:
    src: /brand/welcome.png
    alt: The OtterRoute otter waving
  actions:
    - theme: brand
      text: How it works
      link: /en/guide/how-it-works
    - theme: alt
      text: Installation
      link: /en/guide/install
    - theme: alt
      text: DNS and S3 providers
      link: /en/providers/
features:
  - title: Many domains, many buckets
    details: Routing by host and path prefix. Each bucket has its own credentials and its own cache, isolated from the others.
  - title: Disk cache
    details: A single request to the storage per object even with many clients, streaming without loading files in RAM, stale copies served if the storage is down.
  - title: Domains really verified
    details: The panel checks that DNS resolves and that a request to the domain reaches this node. If a domain stops working, it tells you.
  - title: Users, permissions and 2FA
    details: Roles and scopes on actions, two-step verification with an authenticator app, activity log.
  - title: Statistics and Prometheus
    details: Requests, cache hits, bandwidth, errors and latency in the panel, and the same metrics in Prometheus format.
  - title: Read-only, by design
    details: Only GET and HEAD, credentials stay on the node and the query string never reaches the storage.
---
