FROM node:24-bookworm-slim AS web
WORKDIR /src
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY web/package.json web/package.json
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
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends cmake git pkg-config libssl-dev libsystemd-dev && apt-get clean
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY vendor vendor
COPY migrations migrations
COPY queries queries
COPY schema schema
COPY scripts/build-corrosion.sh scripts/build-corrosion.sh
COPY .sqlx .sqlx
COPY .cargo .cargo
COPY --from=web /src/crates/tilde/src/generated crates/tilde/src/generated
COPY --from=web /src/web/dist web/dist
COPY --from=web /src/web/provider-dist web/provider-dist
RUN scripts/build-corrosion.sh
RUN cargo build --locked --release --features embedded-web --bins

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 libsystemd0 && apt-get clean && useradd --uid 10001 --create-home engine
COPY --from=rust /src/target/release/tilde /usr/local/bin/tilde
COPY --from=rust /src/target/release/tilde-sidecar /usr/local/bin/tilde-sidecar
COPY --from=rust /src/.tools/corrosion /usr/local/bin/corrosion
COPY --from=rust /src/.tools/corrosion.LICENSE /usr/share/doc/corrosion/LICENSE
RUN mkdir -p /var/lib/tilde-sidecar && chown engine:engine /var/lib/tilde-sidecar
RUN install -d -o engine -g engine -m 0700 /var/lib/tilde/log-queue
ENV LOGS_QUEUE_DIR=/var/lib/tilde/log-queue
USER engine
EXPOSE 8080 8081 8082 8083 8787/udp
ENTRYPOINT ["/usr/local/bin/tilde"]
