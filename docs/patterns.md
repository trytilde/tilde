# Patterns

These are the conventions to follow when changing code. `AGENTS.md` is the source of
truth. See [architecture](architecture.md) for the system and
[code structure](code-structure.md) for where things live.

## General rules

From `AGENTS.md`:

- Prefer simple code. Use few, light traits and little generic or dynamic dispatch.
- Add no fields for future use. Use structured fields, not JSON blobs or `metadata`
  bags, unless a maintainer approves.
- Do not assume backwards compatibility unless a maintainer says it is needed. Remove
  unused fields from contracts, structs and tables.
- Comment complex functions, public APIs and unclear edge cases only.
- Keep `docs/` small. Document implementation details in the source files they
  describe.

## PostgreSQL

- Write SQL in `queries/<domain>/*.sql`, never inline in Rust. Do not use SQLx.
- Run `task queries:generate` to regenerate the `tilde-queries` crate in
  `crates/queries/`. Commit the result.
- Call generated queries only from the domain's `db.rs`
  (for example `crates/tilde/src/prompts/db.rs`). `task check:queries` enforces this.
- `db.rs` functions take `&impl GenericClient` and never open transactions. Services
  own transaction boundaries and lock ordering.
- Use the deadpool pool from `crate::database` with tokio-postgres.
- Add schema changes as a new file in `migrations/`, named
  `<version>_<description>.sql` with a timestamp version. Never edit an applied
  migration: the migrator checks each migration's checksum at startup.
- SQL outside Cornucopia (migration ledger, ClickHouse, test fixtures) goes in the
  `queries/_*` directories.

## Contracts

- Contracts live in `proto/tilde/<audience>/v1/`. Put a service under the audience
  that calls it: `management`, `runtime`, `run`, `agent_host`,
  `agent_event_ingress`, `tool_host`, `setup`, `provider` or `provider/tilde`. Shared
  types go in `types/v1`.
- Management contracts must not import runtime cache types.
- Never renumber or reuse a field number. When you remove a field, add `reserved` for
  its number and name. CI runs `buf breaking` against the base branch on pull
  requests.
- Run `task generate` after editing a `.proto` file and commit
  `crates/tilde-contracts/src/generated/`. Mount services through the domain's `rpc.rs` or
  `rpc/` module and the route group composition in `crates/tilde/src/iam/listeners.rs`.

## Secrets

- Encrypt every credential, token or key before storing it. Use
  `Encryption::seal` with a `SecretBinding` (resource kind, resource ID, field name)
  from `crates/tilde/src/encryption.rs`.
- Decrypt with `Encryption::open`, which returns a `SecretString`. Hold plaintext in
  `SecretString` or `zeroize::Zeroizing`, and drop it as soon as the call that needs
  it is done.
- Never derive `Debug` or `Serialize` on types that hold secrets, and never log them.
- Never return provider credentials to agents or management reads. The gateway
  injects them into upstream requests.

## Connection providers

- A built-in provider lives in `crates/tilde/src/connections/catalog/<provider>/`.
  `mod.rs` returns its `definition()`: connection types, capabilities (`channel`,
  `inference`, `tool`), credential source and JSON Schema. Register it in
  `builtins()` in `catalog/mod.rs`.
- Static credentials and standard OAuth use the shared page in `catalog/_standard/`.
  Only a `CredentialSource::Custom` flow gets its own `catalog/<provider>/ui.tsx`.
- Do not add HTML files per provider. Pages are built from `web/connection-ui.html`
  by `task build:connection-ui`.
- Put setup helpers in `sdk/ts/packages/connection-ui` and keep them small. Use React
  Hook Form for credential forms.

## Tool providers

- A managed tool provider lives in `crates/tilde/src/tools/providers/<id>.rs` (or a
  directory). It implements `ToolProvider`: tool definitions with provider-local
  names, and `invoke`, which receives schema-validated input.
- For one-call REST tools, use the table in `tools/providers/rest.rs`.
- Declare the provider's connection type with the `tool` capability in its
  `definition()`. Register the definition in `connections::catalog::builtins()` and
  the `(provider, type)` pair in `tools::providers::provider()`.
- Leave naming, assignment, credentials and audit to the catalog in
  `crates/tilde/src/tools/`.
- Test against a fake upstream, as in `crates/tilde/tests/managed_tool_providers.rs`.

## Web

- Use shadcn primitives from `web/src/components/ui/`. Add new ones with the shadcn
  CLI (`web/components.json`).
- Build forms with React Hook Form.
- Add routes as TanStack Router files in `web/src/routes/` with `createFileRoute`.
  `routeTree.gen.ts` is generated. Commit it, but never edit it by hand.
- Call the API through the ConnectRPC clients in `web/src/client.ts`, which use
  `@trytilde/contracts`.
- Run `task check:web` for formatting, lint and types. Run `task fix:web` to apply
  fixes.

## Tests

- Test orchestration across boundaries. For example: service, then PostgreSQL, then
  another domain, then back. Do not write tests for trivial code such as field
  setters.
- Rust integration tests live in `crates/tilde/tests/`, with shared fixtures in
  `tests/common/`. Read test infrastructure from `TEST_*` variables through
  `common::Storage`.
- Run tests that need a database through `scripts/with-postgres.sh`. It starts a
  disposable PostgreSQL container on a random loopback port, exports `DATABASE_URL`
  and `TEST_DATABASE_URL`, runs the command and removes the container. For example:
  `scripts/with-postgres.sh cargo nextest run -p tilde --test deployments`. Run Rust tests with
  `cargo nextest run`, never `cargo test`; `.config/nextest.toml` holds the profiles (`--profile ci`
  in `task test`).
- `task test` runs the full suite. Focused tasks such as `task test:chat`,
  `task test:sidecar` and `task test:traces` start what they need.

## Tasks and scripts

- Add development commands to `Taskfile.yml`. Do not add scripts to `package.json`.
- Put orchestration that is too complex for a task in `scripts/`, and call it from a
  task.

## Changelog

- Every pull request adds a Changie fragment in `.changes/unreleased/`. Run
  `task change`. Kinds are `Added`, `Changed` and `Removed`.
- For a change with no release note, add a fragment with `body: ""`.
- CI checks this with `scripts/check-changie.sh`. Run `task check-changie` locally.

## Domain model

- Keep `CONTEXT.md` current. Update the definitions there when you add or change a
  domain concept or how concepts interact.
