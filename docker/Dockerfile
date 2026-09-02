# syntax=docker/dockerfile:1
# The FerroTERM server image: one static binary on distroless static, per
# platform. The binaries are not built here. The release workflow builds and
# attests them in the SLSA L3 lane (release-build.yml), verifies each
# attestation, and stages the binaries under dist/<os>/<arch>/ before this
# file is built for linux/amd64 and linux/arm64 (release-image.yml).
#
# Build locally from a staged dist/ tree, from the repository root:
#   docker buildx build -f docker/Dockerfile --platform linux/arm64 .

# distroless static: ca-certificates, tzdata, /etc/passwd with the nonroot
# user (uid 65532), a /tmp, no shell, no package manager, about 2 MiB. Pinned
# by index digest; Dependabot (docker ecosystem) proposes the bumps.
FROM gcr.io/distroless/static-debian13:nonroot@sha256:1c2c046bc09ed40fad370b599a0b1ae7987f55b01e247cf27a7c27cd97e5bbc7

# The platform of the manifest being built, set by buildx per platform.
ARG TARGETOS
ARG TARGETARCH

# Root-owned and not writable by the runtime user.
COPY --chmod=0555 dist/${TARGETOS}/${TARGETARCH}/ferroterm /usr/local/bin/ferroterm

# The binary's default listen address is loopback, which no container can
# publish; the image default is every interface on the documented port.
ENV FERROTERM_LISTEN=0.0.0.0:8080
EXPOSE 8080/tcp

# Numeric on purpose: the kubelet cannot resolve a user name against an image
# it does not read, so `runAsNonRoot: true` refuses a named USER.
USER 65532:65532

# Exec form: the binary is PID 1 and receives SIGTERM itself (there is no
# shell to forward it); it stops gracefully on SIGTERM and SIGINT.
ENTRYPOINT ["/usr/local/bin/ferroterm"]

# Build-independent OCI labels; the release workflow adds the build-dependent
# ones (version, revision, created) and mirrors every label as an index
# annotation, which is what GHCR reads for the package page.
LABEL org.opencontainers.image.title="FerroTERM" \
      org.opencontainers.image.description="A pure-Rust FHIR terminology server for SNOMED CT, LOINC, and other clinical code systems" \
      org.opencontainers.image.url="https://ferroterm.eu" \
      org.opencontainers.image.documentation="https://ferroterm.eu/docs/" \
      org.opencontainers.image.source="https://github.com/rubentalstra/FerroTERM" \
      org.opencontainers.image.vendor="FerroTERM" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.base.name="gcr.io/distroless/static-debian13:nonroot"
