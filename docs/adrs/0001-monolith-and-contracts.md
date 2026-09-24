# ADR 0001: Monolith with contract-driven ConnectRPC

Status: Accepted

The engine is a single-workspace monolith with one Cargo crate initially. Domain
boundaries are Rust modules and concrete services. One root `migrations/` directory
and one embedded migration runner cover all tables. Domain-local migration histories
and microservice-shaped core/router/API crate families are not part of this design.

Protobuf is the source of truth. Pinned prebuilt generators produce checked-in
Rust and JavaScript/TypeScript definitions independently of compiling the server.
The React application consumes the same generated service. The release build
embeds React assets into the executable after the web build.

The first product domain is agent. Its fields are metadata; an endpoint is not an
execution engine. Future domains can add modules and central migrations when their
complete workflows are implemented. Chat and provider integrations are not
included in the initial scope.

Application queries are standalone `.sql` files under `queries/{domain}/`.
Cornucopia validates their parameter and result contracts against disposable
Postgres and generates the committed `crates/queries` crate. Domain `db` modules
call generated functions over deadpool clients and tokio-postgres transactions;
services own locks and transaction boundaries. CI checks generated-code freshness.
Dynamic test-fixture DDL and migration bootstrap SQL remain runtime SQL. This
database checking step is separate from Protobuf client generation.

2026-09-16: Replaced SQLx macros, pool and migration runtime with Cornucopia 1.0.1,
deadpool and tokio-postgres. Existing migration versions and SHA-384 checksums are
preserved when upgrading the ledger to `_tilde_migrations`. The pool carries its
own configuration and shutdown signal; dedicated LISTEN connections use that
configuration and are polled continuously, including during subscription.

The Cargo crate, executable and frontend branding are named `tilde`. The crate
lives at `crates/tilde`. Taskfile commands orchestrate setup, contract generation,
full builds and local development through the existing scripts. Persisted
cryptographic context labels remain unchanged by this naming-only refactor so
existing encrypted values and KMS-wrapped keys remain readable.
