---
source_commit: dd1d866
---
# Images on the fly

OtterRoute can resize and convert images **at request time**, without you having to prepare many versions of the same file. Put the high-resolution original in the bucket and ask for the size you need in the address:

```text
https://cdn.example.com/photos/boat.jpg?w=800&h=600&fit=cover&fmt=webp&q=80
```

The original is never modified. Each variant is created on first access and from then on served from the disk cache like any other file.

## Enabling it

From **Routes**, expand the row and in the **Access and images** section tick *Images on the fly*. The **Access** column shows the *Images* badge. It is a per-route choice: without it, the parameters are ignored and the file is served as is.

## Parameters

| Parameter | Values | What it does |
|---|---|---|
| `w` | 1–4096 | Maximum width in pixels. |
| `h` | 1–4096 | Maximum height in pixels. |
| `fit` | `inside` (default), `cover` | `inside`: the image fits inside the `w`×`h` box keeping proportions. `cover`: fills the box and crops at the center (needs `w` and `h`; with a single side it is the same as `inside`). |
| `fmt` | `webp`, `jpeg`, `png`, `auto` | Output format. Without `fmt` it stays the original's (GIFs become PNG). `auto` picks WebP if the browser declares it in `Accept`, otherwise the original's format, and adds `Vary: Accept`. |
| `q` | 30–95 (default 80) | Quality for JPEG and WebP. Ignored for PNG. |

Rules to know:

- **It never enlarges**: asking `w=3000` for an image 1200 wide returns 1200. With `cover`, if the box exceeds the original it shrinks keeping its proportions.
- The photos' **EXIF orientation** is applied: a photo taken in portrait stays in portrait.
- JPEG has no transparency: transparent parts become white.
- An out-of-range or unknown value (`w=0`, `w=5000`, `fmt=avif`) gives `400`.
- Unrelated parameters (`utm_source=…`) are ignored as always.
- It applies only to `.jpg`, `.jpeg`, `.png`, `.gif` and `.webp` files. Others (SVG, PDF…) are served untransformed.

## Cache and cost

- Each combination of parameters is a **separate variant** in cache, with the same policy as other files (`X-Cache: HIT`/`MISS`, expiry after 1 hour and regeneration). Parameters are normalized (`w=100&h=50` and `h=50&w=100` are the same variant) and the limits above prevent an unlimited number of variants.
- The **original** is downloaded from the storage only once and stays in cache: asking other sizes of the same file does not download it again.
- Transformation is CPU work: the node runs **only a few at a time** (half the cores, between 1 and 4) and other requests wait. With a variant already in cache the cost is that of a normal file.
- `Range` and `HEAD` requests work on variants.
- With [signed links](./signed-links) the signature check happens first; **image parameters are not covered by the signature**: whoever has a link can ask for any allowed size of *that* file.

## Emptying variants

**Purge all** of the route ([Cache](./cache)) also makes all variants unreachable. **Purge file** instead removes only the original's copy: its variants stay until expiry (1 hour) or the next *Purge all*. Changing the *Images on the fly* option empties the route's cache by itself.

## Safety limits

| Limit | Value | If you exceed it |
|---|---|---|
| Original's size | 25 MiB | `413` |
| Original's pixels | 64 million | `422` |
| Memory to decode | 384 MiB | `422` |
| Maximum time for a transformation | 30 seconds | `503` |

A file that is not really an image (or is damaged) gives `422` and is not cached. A missing original gives `404` as always.

## To try

```sh
curl -sI 'https://cdn.example.com/photos/boat.jpg?w=400&fmt=webp' | grep -iE 'content-type|x-cache|content-length'
```

The first request is `MISS` and a bit slower (decoding and re-encoding); the second is `HIT`.

## Limits

- No AVIF (the encoder is too slow to do it on the fly) and no SVG.
- Animated GIFs become a still image (the first frame).
- There are no filters (blur, watermark) or coordinate crops: only resizing, centered `cover` and conversion.
- The node does not look at other caches: a CDN in front must include the query (`?w=…`) and, with `fmt=auto`, the `Accept` header in its key.
