# Code structure

This page maps the repository. See [architecture](architecture.md) for how the parts
fit together and [patterns](patterns.md) for the conventions to follow.

## Top level

```text
.
├── AGENTS.md              Contributor rules for coding agents and humans
├── CONTEXT.md             Domain model: definitions and how they interact
├── Cargo.toml             Rust workspace (crates/*; vendor/rotel is excluded)
├── Taskfile.yml           Every dev, build, test, codegen and release command
├── buf.yaml, buf.gen.yaml Protobuf lint and breaking rules; Rust code generation
├── cornucopia.toml        Cornucopia settings: queries/ -> crates/queries
├── compose.yaml           Local PostgreSQL, MinIO, ClickHouse and an engine service
├── Dockerfile             Release image with the embedded web UI
├── .changie.yaml          Changie release-note configuration
├── .changes/              Changie fragments (unreleased/) and released notes
├── VERSION, CHANGELOG.md  Release version and generated changelog
├── crates/                Rust crates
├── queries/               PostgreSQL queries for Cornucopia, plus non-Cornucopia SQL
├── migrations/            PostgreSQL migrations, embedded in the binary
├── proto/                 Protobuf and ConnectRPC contracts
├── web/                   React management UI and provider setup pages
├── sdk/ts/                TypeScript SDK workspace (packages and examples)
├── sdk/py/                Python SDK workspace (packages and examples)
├── scripts/               Shell, Python and Node scripts that Task calls
├── dev/                   Development-only tooling
├── vendor/rotel/          Vendored Rotel OpenTelemetry fork
├── .github/               CI workflows and composite actions
└── docs/                  These pages and the architecture diagram
```

## Rust crates

```text
crates/
├── tilde/                 The gateway and sidecar (package `tilde`)
│   ├── build.rs           Embeds migrations/*.sql in the binary
│   ├── src/bin/tilde/     Gateway binary: config, route groups, workers
│   │                      (dev_agent.rs and dev_skills.rs are debug builds only)
│   ├── src/bin/tilde-sidecar/  Sidecar binary
│   ├── src/agent/         Agent registry, health, lifecycle, avatars
│   ├── src/chat/          Threads, runs, invocations, channel providers, ingress,
│   │                      access control, audit, runtime RPCs
│   ├── src/connections/   Connection lifecycle, setup broker, OAuth, provider catalog
│   │   └── catalog/<provider>/  Built-in provider definitions and setup UIs
│   ├── src/deployment/    Deployments, run protocol, routing, sidecar protocol,
│   │                      Lambda wakes, native ingress
│   ├── src/tools/         Tool catalog, managed providers, MCP, tool hosts
│   ├── src/inference/     Inference gateway, usage audit, prices, budgets
│   ├── src/prompts/       Prompt versions and matching
│   ├── src/skills/        Skill sources, sync, packages, built-in catalog
│   ├── src/telemetry/     OTLP intake, S3 queue (spool), ClickHouse, viewers
│   ├── src/iam/           Capabilities, invocation tokens, route group composition
│   ├── src/identities/    Identity grouping
│   ├── src/encryption.rs  Secret encryption (seed or AWS KMS)
│   ├── src/database.rs    Pool, migrator; database/ holds notifications
│   ├── src/config.rs      Typed environment configuration
│   ├── src/network.rs     Host and origin checks
│   ├── src/generated/     Generated Rust contracts (do not edit)
│   └── tests/             Integration tests; tests/common/ holds shared fixtures
├── queries/               Generated `tilde-queries` crate (do not edit)
├── client-aws-config/     AWS credential resolution
├── client-aws-sigv4/      AWS Signature Version 4 signing
└── client-aws-kms/        KMS client (GenerateDataKey, Decrypt)
```

Each domain module owns a `db.rs` that wraps its generated queries, and an `rpc.rs`
or `rpc/` directory that implements its ConnectRPC services.

## SQL

```text
queries/<domain>/*.sql     Cornucopia queries, one directory per domain
queries/_migrations/       Migration ledger SQL used by the migrator
queries/_logs/, _traces/, _metrics/  ClickHouse SQL
queries/_tests/            SQL used only by tests
migrations/<version>_<description>.sql  Ordered PostgreSQL migrations
```

Directories starting with `_` are ignored by Cornucopia.

## Contracts

`proto/tilde/<audience>/v1/` groups contracts by who calls them:

| Directory | Audience |
| --- | --- |
| `management/v1` | Management API: UI, operators, CI |
| `runtime/v1` | Agents during an invocation |
| `run/v1` | Agent hosts dialing in (`RunService`) |
| `agent_host/v1` | The wake message (`InvokeRequest`) |
| `agent_event_ingress/v1` | Sidecars (`SidecarService`) |
| `tool_host/v1` | Tool hosts (`ToolHostService`) |
| `setup/v1` | Connection setup and identity verification pages |
| `provider/v1` | Remote connection providers |
| `provider/tilde/v1` | The native Tilde chat provider (`ChatService`) |
| `types/v1` | Shared resource types |

## Web

```text
web/
├── src/routes/            TanStack Router file routes; routeTree.gen.ts is generated
├── src/components/        App components; components/ui/ holds shadcn primitives
├── dist/, provider-dist/  Build output, embedded by the binary with embedded-web
├── src/features/          Logs, tracing and observability views
├── src/client.ts          ConnectRPC clients over @trytilde/contracts
├── connection-ui.html     Shared HTML template for provider setup pages
└── vite.connections.config.ts  Builds catalog ui.tsx pages into provider-dist/
```

## SDKs

```text
sdk/ts/packages/
├── sdk/                   @trytilde/sdk: agent host, context, clients, tool host, deploy CLI
├── contracts/             @trytilde/contracts: generated TypeScript contracts
├── sdk-vercel-ai-node/    Vercel AI SDK adapter
├── sdk-langchain-node/    LangChain / LangGraph adapter
├── sdk-mastra-node/       Mastra adapter
├── sdk-openai-agents-node/  OpenAI Agents SDK adapter
├── chat-ui/               Embeddable native chat components
├── chat-proxy/            Server-side proxy for native chat
├── chat-next/             Next.js adapter for the chat proxy
├── connection-ui/         Setup hooks, theme and controls for provider setup pages
└── agent-avatar/          Agent avatar component
sdk/ts/examples/           basic, Vercel AI, LangChain, Mastra, OpenAI Agents and
                           LangSmith example agents; example-tool-server

sdk/py/packages/
├── tilde/                 trytilde: agent host, context, clients, FastAPI, chat proxy
└── tilde-langchain/, tilde-pydantic-ai/, tilde-openai-agents/, tilde-agno/, tilde-crewai/
                           Framework adapters
sdk/py/examples/           One example agent per adapter; example-tool-server
```

## Scripts, tooling and CI

```text
scripts/with-postgres.sh   Runs a command against a disposable PostgreSQL container
scripts/generate-queries.sh  Applies migrations and runs Cornucopia
scripts/check-query-sites.sh  Enforces that only db.rs files call generated queries
scripts/check-generated.mjs  Checks committed Rust contracts are current
scripts/check-changie.sh   Requires a new Changie fragment
scripts/dev.py, dev-api.sh, run-dev-agent.py  Local development launcher
dev/inference-bench/       Inference gateway benchmark against LiteLLM and Bifrost
vendor/rotel/              Rotel fork; TILDE.md lists the local changes
.github/workflows/         ci.yml (checks and tests), changie.yml, build.yml,
                           release.yml, create-release-pr.yml
.github/actions/           setup (toolchains), register-deployment (CI deploy action)
```

## Generated code

| Output | Source | Committed | Regenerate |
| --- | --- | --- | --- |
| `crates/tilde/src/generated/` | `proto/` via `buf.gen.yaml` | Yes | `task generate` |
| `sdk/ts/packages/contracts/gen/` | `proto/` | No | `task generate` |
| Python contracts in `sdk/py/packages/tilde/src/tilde/` | `proto/` | No | `task generate:py` |
| `crates/queries/` | `queries/` and `migrations/` via Cornucopia 1.0.1 | Yes | `task queries:generate` |
| `web/src/routeTree.gen.ts` | `web/src/routes/` | Yes | Vite dev server or `pnpm --dir web build` |

`task generate` installs pinned generators, runs `buf lint` and `buf generate`, and
builds `@trytilde/contracts`. `task queries:generate` needs Docker and
`cargo install cornucopia --version 1.0.1 --locked`. It starts a disposable database,
applies every migration and regenerates the crate. CI runs
`task queries:generate -- --check` and `pnpm generate:check` to catch stale output.
