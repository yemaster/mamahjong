FROM node:24.14.1-bookworm-slim@sha256:b506e7321f176aae77317f99d67a24b272c1f09f1d10f1761f2773447d8da26c AS game-web-builder

WORKDIR /web

COPY apps/game-web/package.json apps/game-web/package-lock.json ./
RUN --mount=type=cache,id=mamahjong-npm-game,target=/root/.npm,sharing=locked \
    npm ci
COPY apps/game-web ./
# 开发模式开关：compose 的 build-arg 到这里变成 Vite 构建期环境变量，前端用
# import.meta.env.VITE_MAMAHJONG_DEV_MODE 读它。默认 false（生产构建关闭）。
ARG MAMAHJONG_DEV_MODE=false
ENV VITE_MAMAHJONG_DEV_MODE=${MAMAHJONG_DEV_MODE}
RUN npm run build

FROM node:24.14.1-bookworm-slim@sha256:b506e7321f176aae77317f99d67a24b272c1f09f1d10f1761f2773447d8da26c AS admin-web-builder

WORKDIR /web

COPY apps/admin-web/package.json apps/admin-web/package-lock.json ./
RUN --mount=type=cache,id=mamahjong-npm-admin,target=/root/.npm,sharing=locked \
    npm ci
COPY apps/admin-web ./
RUN npm run build

FROM rust:1.85.1-bookworm@sha256:e51d0265072d2d9d5d320f6a44dde6b9ef13653b035098febd68cce8fa7c0bc4 AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY apps ./apps
COPY clients ./clients
COPY crates ./crates

RUN --mount=type=cache,id=mamahjong-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mamahjong-target,target=/build/target,sharing=locked \
    cargo build --locked --release --package mamahjong-server \
    && mkdir --parents /artifacts/data/assets /artifacts/admin /artifacts/game \
    && cp /build/target/release/mamahjong-server /artifacts/ \
    && cp /build/target/release/mamahjong-healthcheck /artifacts/

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:fccdbb0a547c14e23fcf4ce8ad62ca5d43b4faae8d22cd292f490fef9946c96e AS server-base

ARG BUILD_DATE
ARG VCS_REF
ARG VERSION=dev

LABEL org.opencontainers.image.title="MaMahjong Server" \
    org.opencontainers.image.description="Self-hosted multiplayer Mahjong API and realtime server" \
    org.opencontainers.image.source="https://github.com/yemaster/mamahjong" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.revision="${VCS_REF}" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.licenses="MIT"

COPY --from=builder --chown=65532:65532 \
    /artifacts/mamahjong-server \
    /usr/local/bin/mamahjong-server
COPY --from=builder --chown=65532:65532 \
    /artifacts/mamahjong-healthcheck \
    /usr/local/bin/mamahjong-healthcheck
COPY --from=builder --chown=65532:65532 \
    /artifacts/data/ \
    /var/lib/mamahjong/
COPY --from=builder --chown=65532:65532 \
    /artifacts/admin/ \
    /usr/share/mamahjong/admin/
COPY --from=builder --chown=65532:65532 \
    /artifacts/game/ \
    /usr/share/mamahjong/game/

ENV MAMAHJONG_BIND_ADDRESS=0.0.0.0:8080 \
    MAMAHJONG_ADMIN_WEB_DIR=/usr/share/mamahjong/admin \
    MAMAHJONG_GAME_WEB_DIR=/usr/share/mamahjong/game \
    RUST_LOG=info

EXPOSE 8080
USER 65532:65532
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/mamahjong-healthcheck"]

ENTRYPOINT ["/usr/local/bin/mamahjong-server"]

# Publish this target as registry.abstrax.cn/mamahjong/server.
FROM server-base AS server

FROM nginx:1.29.4-alpine3.23-slim@sha256:441b69e13e79b436f9b617910633b6b6adce314c3788c3238dcd8e03b4cb512e AS frontend-base

ENV MAMAHJONG_SERVER_URL=http://server:8080 \
    MAMAHJONG_GAME_WEB_URL=http://web:8080 \
    MAMAHJONG_ADMIN_WEB_URL=http://admin-web:8080 \
    NGINX_ENVSUBST_FILTER=MAMAHJONG_

COPY deployment/nginx/web.conf.template /etc/nginx/templates/default.conf.template

EXPOSE 8080
STOPSIGNAL SIGQUIT

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://127.0.0.1:8080/health || exit 1

# Publish this target as registry.abstrax.cn/mamahjong/web.
FROM frontend-base AS web

ARG BUILD_DATE
ARG VCS_REF
ARG VERSION=dev

LABEL org.opencontainers.image.title="MaMahjong Web" \
    org.opencontainers.image.description="MaMahjong browser game client with configurable API proxy" \
    org.opencontainers.image.source="https://github.com/yemaster/mamahjong" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.revision="${VCS_REF}" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.licenses="MIT"

COPY --from=game-web-builder /web/dist/ /usr/share/nginx/html/game/

# Publish this target as registry.abstrax.cn/mamahjong/admin-web.
FROM frontend-base AS admin-web

ARG BUILD_DATE
ARG VCS_REF
ARG VERSION=dev

LABEL org.opencontainers.image.title="MaMahjong Admin Web" \
    org.opencontainers.image.description="MaMahjong administration console with configurable API proxy" \
    org.opencontainers.image.source="https://github.com/yemaster/mamahjong" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.revision="${VCS_REF}" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.licenses="MIT"

COPY deployment/nginx/admin.conf.template /etc/nginx/templates/default.conf.template
COPY --from=admin-web-builder /web/dist/ /usr/share/nginx/html/admin/

# Keep the default build target backward-compatible with compose.yaml.
FROM server-base AS runtime

COPY --from=game-web-builder --chown=65532:65532 \
    /web/dist/ \
    /usr/share/mamahjong/game/
