# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM node:22-bookworm-slim AS relay-deps
WORKDIR /relay
COPY scripts/package.json scripts/package-lock.json ./
RUN npm ci --omit=dev

FROM node:22-bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd -r -u 10001 -m -d /app appuser
WORKDIR /app
COPY --from=builder /app/target/release/aquachain-agent-gateway /app/gateway
COPY --from=relay-deps /relay/node_modules /app/scripts/node_modules
COPY scripts/relay-submit-data.mjs scripts/package.json /app/scripts/
USER appuser
ENV AGENT_GATEWAY_HOST=0.0.0.0
EXPOSE 8081
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||process.env.AGENT_GATEWAY_PORT||8081)+'/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["/app/gateway"]
