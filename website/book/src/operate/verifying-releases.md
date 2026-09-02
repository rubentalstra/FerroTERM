# Verifying releases

Every Notio release artifact carries a signed provenance attestation. Verify it
before you run a binary, so you know the artifact was built by the project's own
release workflow and not tampered with.

> [!NOTE]
> The release pipeline is stood up and activates on the first tag. Until a
> release exists, the recipe below is the interface you will use, not a check you
> can run yet. The design and rationale are in
> [`docs/ci-cd.md`](https://github.com/rubentalstra/notio/blob/main/docs/ci-cd.md).

<!-- toc -->

## What a release carries

Each release artifact ships with:

- An embedded dependency list, written into the binary's `.dep-v0` section by
  `cargo auditable`.
- A CycloneDX SBOM.
- A `.sha256` checksum.
- A keyless Sigstore provenance attestation and a signed SBOM, both bound to the
  artifact digest and signed by the release workflow's own identity.

The signing is keyless through Sigstore, so there is no long-lived key to manage
or leak.

## Verify the provenance

Use the GitHub CLI to verify an artifact against the workflow that is allowed to
sign it. Replace `<tag>` and `<target>` with the release you downloaded:

```console
$ gh attestation verify notio-<tag>-<target>.tar.gz -R rubentalstra/notio \
    --signer-workflow rubentalstra/notio/.github/workflows/release-build.yml
```

The `--signer-workflow` flag is the point of the check. It requires that the
attestation was produced by that exact reusable workflow in this repository, so a
signature from any other workflow or repository fails. Notio builds its releases
in a reusable workflow to reach SLSA Build Level 3, where the signing identity is
not reachable by user build steps.

## Check the checksum

Confirm the download matches its published checksum:

```console
$ sha256sum -c notio-<tag>-<target>.tar.gz.sha256
```

Run both checks. The checksum tells you the bytes are intact, and the attestation
tells you where the bytes came from.
