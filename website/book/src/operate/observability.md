# Metrics and request identifiers

FerroTERM answers a Prometheus scrape at `GET /metrics` and names every request
with an `X-Request-Id`. Neither is a FHIR interaction: both live off the FHIR
base path, so a scrape is never mistaken for a terminology request.

## Scraping

```bash
curl -s http://127.0.0.1:8080/metrics
```

The exposition is the text format Prometheus reads, with every metric under the
`ferroterm` prefix:

| Metric | Type | Labels | What it says |
|---|---|---|---|
| `ferroterm_http_requests_total` | counter | `method`, `route`, `status` | the requests answered |
| `ferroterm_http_request_duration_seconds` | histogram | `method`, `route`, `status` | how long each took |
| `ferroterm_code_system_loaded` | gauge | `system`, `version` | one per code system version loaded |

`route` is the matched route, `/r4b/CodeSystem/$lookup`, never the URI, so the
series count stays bounded no matter how many codes clients ask about. The
duration buckets start at half a millisecond and double, which brackets the
bars the engine is held to: a point read under a millisecond, a page of an
expansion under ten.

A scrape configuration is one job:

```yaml
scrape_configs:
  - job_name: ferroterm
    static_configs:
      - targets: ["ferroterm:8080"]
```

The endpoint carries no code system content and no request bodies, so it is
safe to expose to a monitoring network. It is not authenticated: keep it on an
internal listener or behind the same reverse proxy rules as the rest of the
server (see [Configuration](configuration.md)).

## Request identifiers

Every response carries `X-Request-Id`. A client that sends the header gets its
own value back, so a trace started at the caller stays one trace; a client that
sends none gets a UUID the server makes. An id that is empty, longer than 128
characters, or not printable ASCII is replaced rather than echoed, so a header
cannot smuggle a line break into a log.

The same id is a field of the request's log line:

```json
{"timestamp":"2026-09-05T09:21:04.113Z","level":"INFO","message":"request",
 "method":"GET","route":"/r4b/CodeSystem/$lookup","status":200,
 "latency_ms":0.42,"named":"system=http://snomed.info/sct code=73211009",
 "request_id":"0f0f6c1e-9d0e-4a5f-8c1a-1f6b0f1f6b0f"}
```

So a client that reports a slow or refused call gives you the id, and the log
line, the status, and the latency follow from it. Bodies are never logged, and
the `named` field carries only the system, url, version, and code parameters a
request stated.
