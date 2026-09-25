FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
RUN cargo build --release --locked -p otterroute || cargo build --release -p otterroute

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data otter \
 && mkdir -p /data/cache /data/state && chown -R otter /data
COPY --from=build /src/target/release/otterroute /usr/local/bin/otterroute
USER otter
ENV OTR_CACHE_DIR=/data/cache \
    OTR_STATE_DIR=/data/state \
    OTR_CONFIG=/etc/otterroute/config.yaml \
    OTR_LISTEN=0.0.0.0:8080 \
    OTR_ADMIN_LISTEN=0.0.0.0:9090
VOLUME ["/data"]
EXPOSE 8080 9090
ENTRYPOINT ["otterroute"]
