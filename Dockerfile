# syntax=docker/dockerfile:1.7

FROM rust:1.85.1-bookworm@sha256:e51d0265072d2d9d5d320f6a44dde6b9ef13653b035098febd68cce8fa7c0bc4 AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY apps ./apps
COPY clients ./clients
COPY crates ./crates

RUN --mount=type=cache,id=mamahjong-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mamahjong-target,target=/build/target,sharing=locked \
    cargo build --locked --release --package mamahjong-server \
    && mkdir --parents /artifacts \
    && mkdir --parents /artifacts/data \
    && cp /build/target/release/mamahjong-server /artifacts/ \
    && cp /build/target/release/mamahjong-healthcheck /artifacts/

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:fccdbb0a547c14e23fcf4ce8ad62ca5d43b4faae8d22cd292f490fef9946c96e AS runtime

COPY --from=builder --chown=65532:65532 \
    /artifacts/mamahjong-server \
    /usr/local/bin/mamahjong-server
COPY --from=builder --chown=65532:65532 \
    /artifacts/mamahjong-healthcheck \
    /usr/local/bin/mamahjong-healthcheck
COPY --from=builder --chown=65532:65532 \
    /artifacts/data/ \
    /var/lib/mamahjong/

ENV MAMAHJONG_BIND_ADDRESS=0.0.0.0:8080 \
    RUST_LOG=info

EXPOSE 8080
USER 65532:65532
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/mamahjong-healthcheck"]

ENTRYPOINT ["/usr/local/bin/mamahjong-server"]
