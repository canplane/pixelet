# pxl-api's image.
#
# Build context is the repo ROOT (docker build -f crates/pxl/server/api/Dockerfile .):
# the dependencies are path dependencies inside the same Cargo workspace, so the
# whole `crates/` tree has to be present for Cargo to resolve it at all.
# .dockerignore strips node_modules/.git/out/dist before any of this reaches the
# build.
#
# The same two stages pxl-geo's image has, and the same reasons — see
# crates/pxl/server/geo/Dockerfile, which sits beside its own crate as this one does.

FROM rust:1.94-slim-trixie AS build
WORKDIR /src

# Only `crates/` is copied: pxl-api reads nothing outside it, and the generated
# files it does read are committed rather than produced at build time.
COPY crates ./crates

# The server profile, not release: a profile belongs to the workspace root and
# release is the module's, which aborts on panic. Off the browser that is
# wrong — a panicking request should take the request down and not the process.
# See crates/Cargo.toml.
# `sharing=locked` on the registry: the two images build in parallel and mount
# the same cache, while cargo's own package lock lives at
# $CARGO_HOME/.package-cache — outside the mount, so each container has its own
# and neither excludes the other. Two cargos unpacking one crate into one
# directory is what that costs: `failed to unpack libm v0.2.11 ... .cargo-ok:
# File exists` (2026-09-19). The target mount needs no such flag; cargo's build
# lock lives inside it and is therefore shared.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/crates/target \
    cargo build --manifest-path crates/Cargo.toml --profile server -p pxl-api \
 && cp crates/target/server/pxl-api /usr/local/bin/pxl-api

# glibc and nothing else. No shell, no package manager, and no CA bundle: the
# only thing this connects to is db-geo, over the compose file's own internal
# network and without TLS, so there is no certificate to verify and nothing for
# a shell in the image to be useful for.
FROM gcr.io/distroless/cc-debian12
COPY --from=build /usr/local/bin/pxl-api /usr/local/bin/pxl-api

EXPOSE 3000
CMD ["pxl-api"]
