# pxl-api's image.
#
# Build context is the repo root: docker build -f crates/pxl/server/api/Dockerfile .
# Same stages and reasons as crates/pxl/server/geo/Dockerfile.

FROM rust:1.98.1-slim-trixie AS build
WORKDIR /src

COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/crates/target \
    cargo build --manifest-path crates/Cargo.toml --profile server -p pxl-api \
 && cp crates/target/server/pxl-api /usr/local/bin/pxl-api

FROM gcr.io/distroless/cc-debian12
COPY --from=build /usr/local/bin/pxl-api /usr/local/bin/pxl-api

EXPOSE 3000
CMD ["pxl-api"]
