---
source_commit: dd1d866
---
# Signed links

To publish **restricted** files (invoices, paid downloads, previews for a client) you can require that a route serves files only to whoever has a **signed link with an expiry**. The bucket stays private as always: the link is the temporary key that opens *that* file, for a limited time.

```text
https://cdn.example.com/invoices/2026-09.pdf?exp=1790000000&sig=9f2c…e1
```

## How it works

- `exp` is the expiry (Unix seconds). `sig` is an **HMAC-SHA256** signature computed with a **secret key of the node** over three values: the domain, the path and the expiry.
- Changing even a single character of the path or of the expiry invalidates the signature. Without the key you cannot forge a link, nor extend its validity.
- The check happens **before the cache**: a file already in cache is not served to whoever has no valid link.
- For every reason (missing, expired, tampered link, rotated key) the node answers the same `403`: whoever tries does not learn what is missing.
- `GET`, `HEAD` and `Range` work (videos and resumed downloads). The signature is not part of the cache key: the same file is saved only once, whichever link opens it.

## Enabling them and creating a link

From **Routes**, expand the row and in the **Signed links** section (`routes:write` is required):

1. Tick *Files of this route open only with a signed link*. From then on, without a signature the node answers 403, even for files already in cache. The **Access** column shows *Signed links*.
2. Type the file (for example `invoices/2026-09.pdf`), choose the validity (from 5 minutes to 30 days) and press **Create link**.
3. **Copy** the full address: it already contains `exp` and `sig`. Tick *https://* if there is a proxy with HTTPS in front of the node.

The API accepts a validity from 1 second to 365 days.

The link is valid for **that path and that domain**. To share several files you need several links.

## Rotating the key

**Rotate the key** (`settings:write` is required) replaces the node's key: **all** links issued so far stop working, for every route. Use it if a link fell into the wrong hands before expiring, or after an incident. There is no revocation of a single link: a short expiry is the main protection.

## From your own program

If you want to generate links from your own service, without going through the panel, the signature is easy to reproduce. You need the key, which is in the state folder's `secrets/_signing.json` file (`key` field, in hexadecimal):

```python
import hmac, hashlib, time

def signed_url(key_hex, host, path, ttl=3600, scheme="https"):
    exp = int(time.time()) + ttl
    msg = f"{host}\n{path}\n{exp}".encode()
    sig = hmac.new(bytes.fromhex(key_hex), msg, hashlib.sha256).hexdigest()
    return f"{scheme}://{host}{path}?exp={exp}&sig={sig}"
```

`path` is the **decoded** path normalized as the node sees it (for example `/invoices/2026-09.pdf`), with a lowercase `host`. If the name has spaces or special characters, in the address it must be percent-encoded, but in the signature it is written in clear. The key is a secret: keep it only on your server and remember that rotating it from the panel changes it.

## Diagnosis

The [Diagnosis](./diagnosis) page has a **Signed link** step: paste the full address and it tells you whether it is valid (and when it expires), expired, tampered or missing.

## Limits

- Single links cannot be revoked, only all together (by rotating the key).
- A link is valid until it expires, for whoever has it: it is not tied to a user or an IP address. If it ends up in a forwarded email or a log, whoever reads it can open that file.
- A correct clock on the node is needed (NTP): the expiry is compared with the node's time.
- Caches **in front of** the node (CDN, proxy) may serve a file even after expiry if they already have it cached: for restricted files configure them not to store responses with `?sig=`.
