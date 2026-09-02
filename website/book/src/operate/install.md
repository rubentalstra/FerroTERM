# Install and run

This page describes how you install and start the server. The distribution is in
design, so treat the steps as the planned shape.

> [!WARNING]
> FerroTERM has no released binary or image yet. The commands below are the intended
> interface, not something you can run today. Follow the repository for release
> announcements.

<!-- toc -->

## Planned distribution

FerroTERM ships as a single static binary and as a container image. Both carry the
whole server: there is no JVM to install, no Elasticsearch to run, and no
external database to provision. You provide a built SNOMED CT index (see
[Loading a SNOMED CT edition](loading-snomed.md)) and point the server at it.

## Run the binary (planned)

Download the binary for your platform and its attestation, verify the
attestation (see [Verifying releases](verifying-releases.md)), then run it:

```console
$ FERROTERM_INDEX=/path/to/ferroterm-index ferroterm
```

The server opens the index read-only, refuses to start on a missing or damaged
one, and listens for FHIR requests on `127.0.0.1:8080`. The listen address, the
index directories, and the default display language are environment variables
described in [Configuration](configuration.md).

## Run the container

The image is `ghcr.io/rubentalstra/ferroterm`, published for `linux/amd64` and
`linux/arm64` with every release. It holds the static server binary on a
distroless base: no shell, no package manager, a numeric non-root user
(`65532`), and the listen address preset to `0.0.0.0:8080`.

```console
$ docker run --rm -p 8080:8080 \
    -v /path/to/ferroterm-index:/data/index:ro \
    -e FERROTERM_INDEX=/data/index \
    ghcr.io/rubentalstra/ferroterm:<version>
```

Mount the index read-only. The server needs no writable volume for serving,
because the index is built offline by a separate tool, so the container runs
with a read-only root filesystem.

Tags are `<version>`, `<major.minor>`, and `latest`. A tag can move; a digest
cannot, so a deployment pins the digest and verifies its provenance and SBOM
before the first pull (see [Verifying releases](verifying-releases.md)):

```console
$ gh attestation verify oci://ghcr.io/rubentalstra/ferroterm:<version> \
    -R rubentalstra/FerroTERM \
    --signer-workflow rubentalstra/FerroTERM/.github/workflows/release-image.yml
```

The image carries no `HEALTHCHECK`; in Kubernetes, point the readiness and
liveness probes at `GET /health`, set `runAsNonRoot: true`,
`readOnlyRootFilesystem: true`, and drop every capability. The server stops
cleanly on `SIGTERM`.

## Check that it is serving

Once the server is up, a FHIR client can call the terminology operations. The
CapabilityStatement tells a client which operations and FHIR versions this
deployment serves:

```console
$ curl http://localhost:8080/metadata
```

See [The FHIR terminology API](../integrate/fhir-api.md) for the operations
themselves.
