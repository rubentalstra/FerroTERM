# Behind a reverse proxy

FerroTERM speaks plain HTTP and, by default, authenticates nobody. That is
deliberate: a deployment already has a proxy, and a terminology server is a
poor place to reimplement TLS and rate limits. This page is the shape that gets
you from that to a served endpoint, and the one part the server can take over
itself: bearer validation on the routes that change content
([Validating tokens in the server](#validating-tokens-in-the-server)).

<!-- toc -->

## What the server does and does not do

| The server | The proxy |
|---|---|
| answers FHIR terminology requests | terminates TLS |
| reads its indexes, writes only what `FERROTERM_RESOURCES` names | authenticates the caller, unless `FERROTERM_OIDC_ISSUER` moves that into the server |
| logs a line per request with no bodies and no free text | rate-limits and sheds load |
| states its endpoint and its declared security in the capability statement | forwards the client's `X-Request-Id` |

The read path holds no patient data: a request names a code system, a code, and
sometimes a display, and the log line carries only the `system`, `url`,
`version`, `code`, `codeA`, and `codeB` parameters. Free-text parameters, request
bodies, and response bodies are never logged, so a proxy log is the only place a
`filter=` term can appear. Keep proxy access logs to the same standard.

## What the server connects to

By default, nothing. The server binary carries no general HTTP client, so a
deployment can refuse it every outbound route and it still answers every
request. The indexes are files it opens read-only, and the one socket it opens
itself is the health probe: a `GET /health` to its own listener over loopback,
because the image has no shell to run a probe with. Fetching a release from a
terminology service happens outside the server, on a machine of your choosing,
and the server reads the artifacts that run writes.

The one exception is the OIDC issuer, and only when you name one. With
`FERROTERM_OIDC_ISSUER` set the server reads that issuer's discovery document
and its JWKS at start, and the key set again when a token names a key it has
not read (see [Validating tokens in the server](#validating-tokens-in-the-server)).
That is the whole of its outbound traffic; leave the variable unset and it
makes none.

`scripts/checks/no-client-in-server.sh` is the evidence. The
`no-client-in-server` job in `.github/workflows/ci.yml` runs it on every pull
request, reading the binary's resolved dependency tree and failing when an HTTP
client reaches it.

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

The default is that FerroTERM validates no tokens: the proxy authenticates and
the capability statement declares what it enforces. The FHIR security page puts
it the same way, a server relies on the deployment's infrastructure and states
what that is (<https://hl7.org/fhir/R4B/security.html>). Authenticate at the
proxy, and tell clients what you did:

```
FERROTERM_SECURITY_SERVICE=SMART-on-FHIR
```

The value is codes of the FHIR `restful-security-service` value set
(`OAuth`, `SMART-on-FHIR`, `Basic`, `Certificates`, `Kerberos`, `NTLM`),
comma-separated, and they appear in `CapabilityStatement.rest.security.service`.
A deployment that declares none says so in words: "The server requires no
authentication of its own; a deployment puts its own in front of it."

If your gateway speaks SMART on FHIR, the scopes worth granting are read-only:
the terminology surface reads, and the only routes that change content are the
resource endpoints and `$closure`, which a deployment enables by naming
`FERROTERM_RESOURCES` and can leave unset.

## Validating tokens in the server

Take the gateway route when one gateway already fronts every service you run
and no client writes to FerroTERM. Take the server route when writes matter:
`FERROTERM_RESOURCES` is set, clients or an editor persist `CodeSystem`,
`ValueSet`, and `ConceptMap` resources, and you want the server itself to know
which caller may change what. A gateway can protect a path; it cannot tell a
terminologist's token from a reporting client's without repeating the scope
rules FerroTERM already applies.

Name the issuer, and the server takes the SMART App Launch resource-server role
(<https://hl7.org/fhir/smart-app-launch/conformance.html>). It issues no tokens
of its own: your authorization server does that, and FerroTERM only reads its
public metadata.

```
FERROTERM_OIDC_ISSUER=https://auth.example.org/realms/tx
FERROTERM_OIDC_AUDIENCE=https://tx.example.org
FERROTERM_OIDC_ADMIN_SCOPE=ferroterm/admin
```

At start the server reads `{issuer}/.well-known/openid-configuration` and the
JWKS it names. It refuses to start when either does not answer, so a server
that cannot check a token never serves a surface it has declared protected.
The issuer URL is `https`, and a plain-HTTP one is refused unless its host is
the loopback, because those bytes decide which signatures the server trusts.
Install your certificate authority in the image when the issuer runs under a
private one: the server uses the platform trust store.

What changes once it is set:

- `POST`, `PUT`, and `DELETE` on `CodeSystem`, `ValueSet`, and `ConceptMap`, on
  every served FHIR version, require a bearer token. So does `POST [base]/$closure`,
  which keeps a stored closure table, and every route on the admin listener.
- The read surface stays open. `$lookup`, `$expand`, `$validate-code`,
  `$subsumes`, `$translate`, `/metadata`, `/health`, and `/metrics` answer
  without a token, whichever HTTP method they use.
- `[base]/.well-known/smart-configuration` is served per version, built from
  the issuer's document, and each capability statement declares `SMART-on-FHIR`
  with the `oauth-uris` extension.

The scope for a route is the SMART version 2 one
(<https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html>):

| Route | Scope that grants it |
|---|---|
| `POST /r4b/CodeSystem` | `system/CodeSystem.c` |
| `PUT /r4b/ValueSet/{id}` | `system/ValueSet.u` |
| `DELETE /r4b/ConceptMap/{id}` | `system/ConceptMap.d` |
| `POST /r4b/$closure` | `system/ConceptMap.u` |
| Any admin-listener route | the `FERROTERM_OIDC_ADMIN_SCOPE` value |

SMART names no scope for `$closure`, so that last row is our own reading: the
operation maintains a stored `ConceptMap`
(<https://hl7.org/fhir/R4B/conceptmap-operation-closure.html>), and no
specification governs the admin listener at all.

The combined forms work as the specification defines them, so
`system/CodeSystem.cud` and `system/*.cruds` both grant a create, and the
version 1 `system/CodeSystem.write` and `system/CodeSystem.*` are accepted for
compatibility. A scope narrowed by search parameters
(`system/CodeSystem.cud?url=…`) grants nothing: the server does not evaluate
the restriction, so it refuses rather than widening it. That is why the
discovery document claims `permission-v1` and not `permission-v2`, which names
the granular syntax. Grant an unnarrowed scope.

The token itself is an ordinary JWT signed by the issuer. The server checks the
signature against the JWKS, `iss`, `exp`, `nbf`, `aud` when you configured one,
and the `typ` header when the issuer sets one: a token typed as something other
than an access token (an ID token, for example) is refused rather than spent
here.

A client obtains its token through SMART Backend Services, which is the flow a
sync service or an editor uses
(<https://hl7.org/fhir/smart-app-launch/backend-services.html>):
`client_credentials` with `private_key_jwt`, the scopes above, and the token
presented as `Authorization: Bearer`. What comes back on a refusal follows
RFC 6750 §3:

| Situation | Answer |
|---|---|
| No token | `401`, `WWW-Authenticate: Bearer realm="…"`, an `OperationOutcome` with `login` |
| Expired, malformed, wrong issuer, wrong audience, wrong signature | `401` with `error="invalid_token"` |
| Valid token, no scope for the route | `403` with `error="insufficient_scope"` and the scope it wanted |

Key rotation needs nothing from you. A token naming a `kid` the server has not
read makes it fetch the JWKS again, at most once a minute, so publishing the
new key before you sign with it is enough.

## The admin listener stays behind the proxy

`FERROTERM_ADMIN_LISTEN` binds a second listener that serves `POST /reload`
and nothing else. Without `FERROTERM_OIDC_ISSUER` it authenticates nobody
([Configuration](configuration.md#reloading-the-served-set)). Bind it to
`127.0.0.1` or an address on your internal network, publish only the FHIR
listener through the proxy, and reach the reload from the host or from the
orchestration that writes the new artifacts. A deployment that puts the admin
address behind the same public proxy hands anyone a way to make the server
re-read its disks.

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
