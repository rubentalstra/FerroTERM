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
$ ferroterm --index /path/to/ferroterm-index
```

The server starts, opens the index read-only, and listens for FHIR requests. You
configure the listen address, the index path, and the served FHIR versions
through the settings described in [Configuration](configuration.md).

## Run the container (planned)

```console
$ docker run --rm -p 8080:8080 \
    -v /path/to/ferroterm-index:/data/index:ro \
    ghcr.io/rubentalstra/FerroTERM:<tag> \
    --index /data/index
```

Mount the index read-only. The server needs no writable volume for serving,
because the index is built offline by a separate tool.

## Check that it is serving

Once the server is up, a FHIR client can call the terminology operations. The
CapabilityStatement tells a client which operations and FHIR versions this
deployment serves:

```console
$ curl http://localhost:8080/metadata
```

See [The FHIR terminology API](../integrate/fhir-api.md) for the operations
themselves.
