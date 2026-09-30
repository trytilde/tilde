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
# .cargo/config.toml compiles through sccache; its cache mount below keeps crates between local builds.
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
RUN --mount=type=cache,target=/root/.cache/sccache --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --locked --release --features embedded-web --bins && sccache --show-stats

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 libsystemd0 && apt-get clean && useradd --uid 10001 --create-home tilde
COPY --from=rust /src/target/release/tilde /usr/local/bin/tilde
COPY --from=rust /src/target/release/tilde-sidecar /usr/local/bin/tilde-sidecar
USER tilde
EXPOSE 8080 8081 8082
ENTRYPOINT ["/usr/local/bin/tilde"]
