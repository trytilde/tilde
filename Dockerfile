# syntax=docker/dockerfile:1
FROM node:24-bookworm-slim AS web
WORKDIR /src
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY web/package.json web/package.json
# The web app links @trytilde/contracts by path; the target must exist before install.
COPY sdk/ts/packages/contracts/package.json sdk/ts/packages/contracts/package.json
RUN pnpm install --frozen-lockfile
COPY sdk/ts sdk/ts
RUN pnpm --dir sdk/ts install --frozen-lockfile
COPY web web
COPY crates/tilde/src/connections/catalog crates/tilde/src/connections/catalog
COPY proto proto
COPY buf.yaml buf.gen.yaml ./
COPY scripts/install-codegen.mjs scripts/install-codegen.mjs
RUN pnpm tools && pnpm generate && pnpm --dir sdk/ts build && pnpm --dir web build && pnpm --dir web exec tsc -p tsconfig.connections.json && pnpm --dir web exec vite build --config vite.connections.config.ts

FROM rust:1.98.1-bookworm AS rust
ARG TARGETARCH
ARG SCCACHE_VERSION=0.15.0
RUN arch="$([ "$TARGETARCH" = arm64 ] && echo aarch64 || echo x86_64)" && \
    curl -fsSL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-${arch}-unknown-linux-musl.tar.gz" \
    | tar -xz --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE_VERSION}-${arch}-unknown-linux-musl/sccache"
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends cmake git pkg-config libssl-dev libsystemd-dev protobuf-compiler && apt-get clean
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY vendor vendor
COPY migrations migrations
COPY queries queries
COPY .cargo .cargo
COPY --from=web /src/crates/tilde/src/generated crates/tilde/src/generated
COPY --from=web /src/web/dist web/dist
COPY --from=web /src/web/provider-dist web/provider-dist
# CI passes Depot Cache's endpoint and token as secrets so crates compile through sccache; a build
# without them (local `docker build`) compiles directly. The registry cache mount helps local builds.
RUN --mount=type=secret,id=sccache_endpoint --mount=type=secret,id=sccache_token \
    --mount=type=cache,target=/usr/local/cargo/registry \
    endpoint="$(cat /run/secrets/sccache_endpoint 2>/dev/null || true)" && \
    if [ -n "$endpoint" ]; then \
      export SCCACHE_WEBDAV_ENDPOINT="$endpoint" RUSTC_WRAPPER=sccache; \
      token="$(cat /run/secrets/sccache_token 2>/dev/null || true)"; \
      if [ -n "$token" ]; then export SCCACHE_WEBDAV_TOKEN="$token"; fi; \
      sccache --start-server || export RUSTC_WRAPPER=; \
    fi && \
    cargo build --locked --release --features embedded-web --bins && \
    if [ -n "${RUSTC_WRAPPER:-}" ]; then sccache --show-stats; fi

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 libsystemd0 && apt-get clean && useradd --uid 10001 --create-home tilde
COPY --from=rust /src/target/release/tilde /usr/local/bin/tilde
COPY --from=rust /src/target/release/tilde-sidecar /usr/local/bin/tilde-sidecar
USER tilde
EXPOSE 8080 8081 8082
ENTRYPOINT ["/usr/local/bin/tilde"]
