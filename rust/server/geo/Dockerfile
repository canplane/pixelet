# pxl-geo's image — the world's data server.
#
# Build context is the repo root: docker build -f rust/server/geo/Dockerfile .

# Must equal rust/rust-toolchain.toml; the build runs under the image's own
# toolchain (RUSTUP_TOOLCHAIN), so rustup fetches none of the file's targets.
FROM rust:1.98.1-slim-trixie AS build
WORKDIR /src
ENV RUSTUP_TOOLCHAIN=$RUST_VERSION

COPY rust ./rust
COPY xpute/rust ./xpute/rust

# The server profile, not release: release aborts on panic.
# `sharing=locked`: both images share the registry cache, and cargo's package
# lock is outside the mount (`failed to unpack libm ... File exists`).
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/rust/target \
    cd rust && cargo build --profile server -p pxl-geo \
 && cp target/server/pxl-geo /usr/local/bin/pxl-geo

# No CA bundle or shell: the only connection is db-geo on the internal network, without TLS.
FROM gcr.io/distroless/cc-debian12
COPY --from=build /usr/local/bin/pxl-geo /usr/local/bin/pxl-geo

EXPOSE 3000
CMD ["pxl-geo"]
