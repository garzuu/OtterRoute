FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
RUN cargo build --release --locked -p otterroute || cargo build --release -p otterroute

FROM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web ./
RUN npm run build

FROM node:22-bookworm-slim AS docs
WORKDIR /docs
COPY docs/package.json docs/package-lock.json ./
RUN npm ci
COPY docs ./
# la guida offline vive sotto /docs/ della porta di amministrazione
RUN DOCS_BASE=/docs/ DOCS_LAST_UPDATED=0 npx vitepress build

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data otter \
 && mkdir -p /data/cache /data/state && chown -R otter /data
COPY --from=build /src/target/release/otterroute /usr/local/bin/otterroute
COPY --from=web /web/dist /usr/share/otterroute/ui
COPY --from=docs /docs/.vitepress/dist /usr/share/otterroute/docs
USER otter
ENV OTR_INSTALL=docker \
    OTR_CACHE_DIR=/data/cache \
    OTR_STATE_DIR=/data/state \
    OTR_CONFIG=/data/config.yaml \
    OTR_UI_DIR=/usr/share/otterroute/ui \
    OTR_DOCS_DIR=/usr/share/otterroute/docs \
    OTR_LISTEN=0.0.0.0:80 \
    OTR_ADMIN_LISTEN=0.0.0.0:9090
VOLUME ["/data"]
EXPOSE 80 443 9090
ENTRYPOINT ["otterroute"]
