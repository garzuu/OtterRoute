---
source_commit: dd1d866
---
# Automatic HTTPS

OtterRoute can serve **HTTPS by itself**: it obtains a free certificate for every verified domain, renews it before it expires and uses it to answer on port 443. No proxy in front is needed. If you already have a proxy or CDN that handles TLS, see [HTTPS and proxy](./https-proxy).

## How it works

1. In **Settings → Automatic HTTPS** tick *Obtain and renew certificates* (and, optionally, give a contact email for the CA).
2. For every **verified** domain the node asks Let's Encrypt for a certificate using the **HTTP-01** challenge: the CA makes a request to `http://yourdomain/.well-known/acme-challenge/…` on port 80 and the node answers with the proof.
3. The certificate is saved in `certs/<domain>/` in the state folder (the key with `0600` permissions) and used immediately: the node picks the right certificate from the name the browser asks for (SNI).
4. **30 days before expiry** the node renews it by itself (for short-lived certificates the window shrinks to a third of their life, so they are not renewed at every pass). If an issuance fails, it retries after an hour (CAs limit attempts) and warns you in the bell and, if enabled, in [notifications](./notifications).

The state of each domain is in the **HTTPS** column of the *Domains* page: *Valid*, *Expiring* (less than 14 days), *Expired*, *Not issued*, *Issuing*, *Error* (with the CA's message). From the expanded row you can **Request** or **Renew now** a certificate.

## What you need

- The domain must be **verified** and reach this node on **port 80 from the Internet**: the CA must be able to reach the challenge. A firewall blocking port 80, or a proxy/CDN that does not forward the `/.well-known/acme-challenge/` path, makes issuance fail.
- The node must be able to **listen on 443** (or on the `OTR_HTTPS_LISTEN` port). If the port is not available the node still starts, without HTTPS, and says so in the log and in Settings.
- Public names: `.localhost` domains and IP addresses cannot have a public certificate.
- **No wildcards** (`*.example.com`): with HTTP-01 each name has its own certificate. For a wildcard certificate obtain it elsewhere and upload it (see below).

::: warning Behind Cloudflare or another proxy
If the domain points to a proxy (for example Cloudflare with the orange cloud), the challenge reaches the proxy and not the node: issuance fails. In that case leave certificates to the proxy and use [HTTPS and proxy](./https-proxy).
:::

## Trying it without limits: the staging environment

Let's Encrypt has request limits. Tick *Use the test environment (staging)* to try the whole path without consuming them: staging certificates are **not valid in browsers**. To move to real certificates untick it: the node requests new ones (the account with the CA is separate for each environment).

## Redirecting HTTP to HTTPS

In a domain's row, after obtaining the certificate, you can tick *Redirect HTTP to HTTPS*: the node answers `308` towards `https://…` keeping path and query. It cannot be enabled until the domain has a certificate, because the site would become unreachable.

These always stay on HTTP: **domain verification** (`/.well-known/otterroute/check`) and **ACME challenges**, so verification and renewal keep working.

## Uploading your own certificate

For a corporate CA, a wildcard certificate or one you already bought, in the domain's expanded row choose **Upload a certificate**: paste the chain (certificate + intermediates) and the private key in PEM. The certificate is used immediately, but it is **not renewed by itself**: at expiry it must be uploaded again (you see it in the domain's state and in the alerts).

## Ports and settings

| | Default | How to change it |
|---|---|---|
| HTTPS listening | `0.0.0.0:443` | `OTR_HTTPS_LISTEN` (empty = HTTPS disabled) |
| Public HTTPS port (for redirects) | 443 | Settings → Ports |

The Docker image exposes 443: map it with `-p 443:443` (already present in `compose.yaml`).

## Security

- Private keys are in `certs/<domain>/privkey.pem` (`0600` permissions); the ACME account key is in `secrets/`. See [Security](./security) and [Upgrades and backup](./upgrades-backup): include them in the backup, or let them be regenerated.
- The node accepts only TLS 1.2 and 1.3, with ALPN `h2` and `http/1.1`.
- A handshake with no certificate for the requested name is refused: no "fallback" certificate is ever shown.

## Limits

- Only the HTTP-01 challenge: no wildcard certificates issued by the node, and port 80 must be reachable.
- One node per domain: with several nodes behind the same name the challenge may reach a different node from the one that requested it.
- There is no revocation from the panel; to revoke use the CA's tools.
- The flow with Let's Encrypt is covered by tests against a test ACME server (Pebble) in CI, not by tests on real production domains.
