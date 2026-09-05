# Behind a reverse proxy

FerroTERM speaks plain HTTP and authenticates nobody. That is deliberate: a
deployment already has a proxy, and a terminology server is a poor place to
reimplement TLS, tokens, and rate limits. This page is the shape that gets you
from that to a served endpoint.

<!-- toc -->

## What the server does and does not do

| The server | The proxy |
|---|---|
| answers FHIR terminology requests | terminates TLS |
| reads its indexes, writes only what `FERROTERM_RESOURCES` names | authenticates the caller |
| logs a line per request with no bodies and no free text | rate-limits and sheds load |
| states its endpoint and its declared security in the capability statement | forwards the client's `X-Request-Id` |

The read path holds no patient data: a request names a code system, a code, and
sometimes a display, and the log line carries only the `system`, `url`,
`version`, `code`, `codeA`, and `codeB` parameters. Free-text parameters, request
bodies, and response bodies are never logged, so a proxy log is the only place a
`filter=` term can appear. Keep proxy access logs to the same standard.

## The quickstart, proxied

`compose.yaml` carries a `proxied` profile: Caddy in front, the server on the
compose network only.

```bash
FERROTERM_DOMAIN=tx.example.org \
FERROTERM_BASE_URL=https://tx.example.org \
docker compose --profile proxied up
```

With the default domain (`localhost`) Caddy issues from its own internal CA, so
the profile works offline; name a real domain and it fetches and renews a
certificate itself. The server publishes no host port in this profile: the proxy
reaches it as `ferroterm:8080` inside the network.

## The base URL

A server behind a proxy answers on an address it never sees: the proxy
terminates TLS and forwards a plain request, so the socket the process bound is
not the URL a client used. `FERROTERM_BASE_URL` is that URL, without a version
prefix and without a trailing slash:

```
FERROTERM_BASE_URL=https://tx.example.org
```

Each version's capability statement then states its own endpoint, so a client
that reads one learns where to send the next request:

```json
"implementation": {
  "description": "FerroTERM terminology server",
  "url": "https://tx.example.org/r4b"
}
```

Both `GET /r4b/metadata` and `GET /r4b/metadata?mode=terminology` carry it. A
deployment that sets nothing states no URL rather than a wrong one.

## Forwarded headers

Terminate TLS at the proxy and forward the original scheme, host, and client
address. Caddy's `reverse_proxy` sets `X-Forwarded-For`, `X-Forwarded-Proto`,
and `X-Forwarded-Host` by itself; nginx needs them spelled out:

```nginx
location / {
    proxy_pass         http://ferroterm:8080;
    proxy_set_header   Host              $host;
    proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
    proxy_set_header   X-Forwarded-Proto $scheme;
    proxy_set_header   X-Forwarded-Host  $host;
    proxy_set_header   X-Request-Id      $request_id;
    proxy_read_timeout 60s;
}
```

The server does not read these headers: it takes its public URL from
`FERROTERM_BASE_URL` rather than from a header a client could forge, which is
the same reason a proxy should strip an inbound `X-Forwarded-*` it did not set.
It does read `X-Request-Id`, and echoes it, so a proxy that generates one (as
nginx does above) gets the same id in the server's log line. See
[Metrics and request identifiers](observability.md).

## Authentication

**FerroTERM validates no tokens, and this is a decision rather than a gap.**
Bearer-token validation in the server would mean shipping a JWKS client, a
clock, a cache, and an issuer policy, and every deployment that already runs a
gateway would then have two places where an expired token is judged. The FHIR
security page puts it the same way: a server relies on the deployment's
infrastructure, and the capability statement declares what that is
(<https://hl7.org/fhir/R4B/security.html>).

So authenticate at the proxy, and tell clients what you did:

```
FERROTERM_SECURITY_SERVICE=SMART-on-FHIR
```

The value is codes of the FHIR `restful-security-service` value set
(`OAuth`, `SMART-on-FHIR`, `Basic`, `Certificates`, `Kerberos`, `NTLM`),
comma-separated, and they appear in `CapabilityStatement.rest.security.service`.
A deployment that declares none says so in words: "The server requires no
authentication of its own; a deployment puts its own in front of it."

If your gateway speaks SMART on FHIR, the scopes worth granting are read-only:
the terminology surface reads, and the only writes are the resource endpoints
and `$closure`, which a deployment enables by naming `FERROTERM_RESOURCES` and
can leave unset.

## Rate limits and timeouts

The proxy is the place for both. An expansion of a large implicit value set is
the expensive request to bound: `ValueSet/$expand` answers at most 1,000 members
without `count` and asks for paging beyond that, so a limit on requests per
second per client, plus a read timeout in the tens of seconds, is enough. Cap
the request body too (a `Parameters` POST with `tx-resource` resources is the
largest thing a client sends).

Health and metrics stay off the public route:

```
handle /health /metrics {
    respond 404
}
```

Probe them from inside the network instead, where the container's own port is
reachable.
