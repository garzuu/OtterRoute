---
source_commit: dd1d866
---
# Prometheus metrics

`GET http://127.0.0.1:9090/metrics`, text format, no authentication. → [Statistics](/en/guide/statistics)

| Metric | Type | Labels | Description |
|---|---|---|---|
| `otterroute_requests_total` | counter | `route`, `cache`, `class` | Requests served. `cache`: `HIT`, `MISS`, `STALE`, `REVALIDATED`, `BYPASS`, `none`; `class`: `2xx`…`5xx`. |
| `otterroute_response_bytes_total` | counter | `route` | Bytes sent to visitors. |
| `otterroute_upstream_errors_total` | counter | `route`, `storage` | Errors towards the storage. |
| `otterroute_request_duration_seconds` | histogram | `route` | Latency up to the response (`_bucket`, `_sum`, `_count`). |
| `otterroute_cache_bytes` | gauge | — | Space used by the cache. |
| `otterroute_cache_max_bytes` | gauge | — | Cache limit. |
| `otterroute_cache_entries` | gauge | — | Copies in cache. |
| `otterroute_cache_inflight` | gauge | — | Downloads from the storage in progress. |
| `otterroute_config_version` | gauge | — | Version of the active configuration. |
| `otterroute_build_info` | gauge | `version`, `install` | Always 1: running version and installation type (`docker`, `service`, `binary`, `source`). |
| `otterroute_image_transforms_total` | counter | `result` | [Images on the fly](/en/guide/images): `ok`, `error` (invalid file, 422), `too_large` (413), `busy` (timed out or interrupted, 503). |
| `otterroute_image_transform_seconds_sum` / `_count` | counter | — | Total time and number of successful transformations: the average is `sum / count`. |
| `otterroute_signed_links_denied_total` | counter | — | Requests refused with `403` for a missing, expired or invalid [signed link](/en/guide/signed-links). |
| `otterroute_acme_issuances_total` | counter | `result` | Issuances of [automatic certificates](/en/guide/https): `ok`, `error`. |
| `otterroute_certificate_not_after_timestamp_seconds` | gauge | `host` | Expiry of the domain's certificate (Unix seconds); absent if there is none. |
| `otterroute_certificate_serving` | gauge | `host` | 1 if the node serves on HTTPS an unexpired certificate for the domain. |
| `otterroute_update_available` | gauge | — | 1 if a newer version than the one in use exists. |
| `otterroute_update_last_check_timestamp_seconds` | gauge | — | Last check for new versions (0 = never). |
| `otterroute_update_checks_total` | counter | `result` | Checks for new versions: `ok`, `error`. |
| `otterroute_updates_total` | counter | `result` | Updates seen by this process: `applied` (start after a version change), `rolled_back`. |

Requests that match no rule have `route="-"`.

## Useful queries and alerts

```text
# certificates expiring within 14 days
(otterroute_certificate_not_after_timestamp_seconds - time()) < 14 * 86400

# a domain with no certificate in use
otterroute_certificate_serving == 0

# average duration of an image transformation (last 5 minutes)
rate(otterroute_image_transform_seconds_sum[5m]) / rate(otterroute_image_transform_seconds_count[5m])

# failed or postponed transformations
sum(rate(otterroute_image_transforms_total{result!="ok"}[5m]))

# the node has not checked for versions for more than 3 days (with the check on)
time() - otterroute_update_last_check_timestamp_seconds > 3 * 86400

# a new version exists
otterroute_update_available == 1
```

