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

Requests that match no rule have `route="-"`.
