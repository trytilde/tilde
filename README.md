# Tilde

This repository owns Tilde's open-source server and all public SDKs, client
libraries and framework adapters. Cloud and on-prem enterprise development lives
in `trytilde/tilde-cloud`, which consumes the SDKs from here.

The initial import is cloud commit `5659e91dbbd478085e20e8a662ef852162039406`.
The OSS simplification is in progress: remove management RBAC, groups and general
API keys, retaining agent runtime capabilities and an agent-specific deployment
registration key exposed on the Capabilities and Deployments screens. Until that
cutover lands, the imported server still includes the original management IAM.

Tilde aims to be the ubiqiuotous meta-harness for AI agents. Build AI agents in your framework of choice, register them in the Tilde Agent registry and manage them from a single place. Today we offer an agent registry to see and manage deployed agents, integrations with chat channels to forward messages from email, Whatsapp or realtime audio to your agent and an audit trail of agent traces & conversations that you can export.

Conversation activity history is batched asynchronously from a bounded in-memory buffer.
It can lose queued events on a crash or overflow. Canonical messages and operational
records remain durable; audit views can lag their updates briefly.

We plan to add:

- Tools (managed third party integrations, proxy external MCP servers, deploy custom tool endpoints)
- Memory
- Skills (centrally manage all skills available to agents in the registry)

## Build and run

Install [Task](https://taskfile.dev/docs/installation), then run:

```sh
task build  # Complete release build, including embedded React UI
task dev    # Rust API plus Vite; first copy .env.example to .env
task test   # Rust and Connect client tests against disposable Postgres
task check  # Generated code, frontend types, formatting and Clippy
```

Build and dev install dependencies and generate contracts first. The release
binary is `target/release/tilde` (or under `CARGO_TARGET_DIR` when configured).
Task loads your `.env` and private `.env.local` overrides. Set `DATABASE_URL` and
encryption configuration there; exported environment variables take precedence.
When changing dev ports, also update their matching listen/public URLs in `.env`. `task generate` runs
contract generation independently of Rust compilation. `task --list` lists commands.

## Versioning

[Changie](https://changie.dev/) manages one shared release version for the Rust
workspace (including `tilde --version`), every package in `sdk/ts/packages/*` and every
Python package in `sdk/py/packages/*`.
The private web app ships inside the binary; private workspace roots and the
example agent are not release artifacts. `VERSION` and `.changes/` record the
release version; `.changie.yaml` updates Cargo and SDK manifests on merge.

Install Changie v1.24.0 (the version used to validate this setup) and Python 3.11+.
Every PR adds a fragment, following the same categories as `tilde-api`:

```sh
task change -- --kind Changed --body "Describe the change." --interactive=false
task check-changie               # Compare with main; accepts -- <base-ref>
task version:check
task release:prepare -- patch    # Or minor, major, auto, or v1.2.3
```

Like `tilde-api`, `auto` selects a **major** bump for Added, Changed, or Removed.
Use an explicit bump when preparing a patch or minor release. Release preparation
batches fragments, merges the changelog, updates package versions, refreshes
the Cargo, pnpm and uv lockfiles, and checks version agreement. Install dependencies first with
`task setup` and `cargo fetch --locked`. Review and commit the generated changes
together before tagging `v<version>` and building/publishing the release artifacts.
`release:prepare` only prepares versions locally. `task sdk:pack` builds public npm SDK
tarballs in `dist/npm` and `task sdk:py:pack` builds the Python SDK sdists and wheels in
`dist/py`; the Release workflow publishes them.

### CI and publishing

- **Changie** requires a new PR fragment, validates fragments with Changie's batch
  dry-run, checks versions, and verifies that `changie merge` produces no drift.
- **Check** runs generation, compatibility, formatting, Clippy, Cornucopia, and integration checks.
- **Build** packs the npm and Python SDK packages and builds and smoke-tests the
  Linux amd64 Docker image (`tilde:ci`) without pushing it.
- **Create Release PR** is manually dispatched on the default branch. Select a
  patch/minor/major/auto bump or give an explicit version; it updates `codex/release` with versions, lockfiles,
  release notes, and a blank acknowledgement so the PR passes the fragment rule.
- **Release** runs when that PR's `VERSION` change lands on the default branch (or on
  manual dispatch there). It reruns Changie, Check, and Build against the commit, then
  publishes the container to `ghcr.io/trytilde/tilde` as `<version>`, `<major>.<minor>` and
  `latest`, the public `@trytilde/*` packages to npm and the Python SDK packages to PyPI, and
  finally creates the `v<version>` tag and GitHub release with the packages attached. Rust
  crates remain internal build dependencies; the Rust distribution is the container, not
  crates.io packages. The proprietary Tilde Cloud image is released from `trytilde/tilde-cloud`.

Configure `GIT_BOT_TOKEN` with repository contents and pull-request write access
for release PR creation; a bot token allows the PR to trigger CI. Configure
`NPM_TOKEN` with publish access to the `@trytilde` scope (including any new packages)
and the npm permissions needed for unattended publishing. PyPI uses trusted publishing:
register this repository's `release.yml` as a trusted publisher on each PyPI project. GHCR
and GitHub Releases use the repository's `GITHUB_TOKEN`; allow Actions to write packages
and releases.
Require the Changie, Check, and Build checks before merging release PRs.

Publishing currently accepts stable semantic versions only. All public SDK packages
release together; versions already on npm, PyPI or GHCR are skipped, so a partially
failed release is completed by rerunning it for the **same commit**. Releases start at 3.0.0,
above the `@trytilde/sdk` (0.3.0) and `@trytilde/sdk-vercel-ai-node` (2.0.0) versions
published from `trytilde/dispatch`, so no release version can collide with them. The tag and
GitHub release are created only after the image and every package are published. A tag
pointing at another commit is rejected. Registry publication is not atomic: one registry can
succeed while another fails.

For PRs without release notes (including a release preparation PR), add a new
`.changes/unreleased/Changed-YYYYMMDD-HHMMSS.yaml` acknowledgement with:

```yaml
kind: Changed
body: ""
time: 2026-09-09T12:00:00+02:00 # Use the current timestamp
```

Changie does not traverse dependency graphs or automatically bump dependents.
All SDK packages instead bump together, including a high-level SDK when an
internal dependency changes. Use `workspace:*`, `workspace:^`, or `workspace:~`
for internal package dependencies: pnpm replaces them with the current dependency
version/range during **pnpm pack/publish**. Publish all public packages for each
shared release. An external dependency update still needs a fragment and a release;
editing a dependency alone does not change any package's own version.
Independent package releases would require a separate dependency-aware release
planner. New languages need manifest replacements and version checks added here.

## Development

Requires the pinned Rust toolchain, Node 22.12+ (Node 24 recommended), pnpm and
Postgres 16+. Docker runs local development Postgres and the disposable test harness.

```sh
pnpm install --frozen-lockfile
pnpm tools
pnpm generate
```

`pnpm tools` downloads pinned official code-generator binaries and checks their
SHA-256 digests. `pnpm generate` runs Buf and the generators directly. It needs
**no Rust compiler, application build, running server, database or cloud keys**.
The generated Rust is checked in, so ordinary Cargo builds need neither Node nor
Buf. The TypeScript contracts are not: `@trytilde/contracts`
(`sdk/ts/packages/contracts`) generates them into its ignored `gen/` directory on
build, and the web app and `@trytilde/sdk` import from that package instead of
carrying copies. `pnpm generate`, the SDK workspace build and every web script build
it first, so a fresh clone only needs `task setup`.
Cornucopia bindings are checked into `crates/queries`, so Rust compilation needs no database.

Copy `.env.example` to ignored `.env` for development; `.env.test` supplies fixture-test defaults.
The development seed is public and suitable only for disposable local data. For
persistent data, generate a seed with `tilde generate-key` or `openssl rand -base64 32`
and retain it in ignored `.env.local`. Never replace the seed for an existing database.

Put private keys (for example `OPENAI_API_KEY` for the example agents or
`NGROK_AUTHTOKEN`) in ignored `.env.local`, and test-only overrides in ignored
`.env.test.local`. Exported variables take precedence, followed by `.env.local`, then `.env`
for development. Test tasks instead use `.env.test.local`, then `.env.test`; the disposable
Postgres wrapper supplies the actual test database URL. Task parses dotenv values as data;
do not shell-source them, as they may contain multiline PEM keys.

```sh
task dev
task test:chat       # all six adapters, local HTTP fixtures and disposable Postgres
```

The fixture suite checks Slack, GitHub, AgentMail, Linq, Meta WhatsApp and Telnyx
WhatsApp through encrypted connections, agent tools, upstream HTTP, signed inbound
webhooks and the stored conversation audit. It covers duplicate callbacks, rejected
signatures and upstream send failures, and needs no provider credentials.

Alternatively, use exported variables:

```sh
export POSTGRES_PASSWORD='your-password'
export DATABASE_URL='postgres://engine:your-password@127.0.0.1:5432/engine'
export ENGINE_ENCRYPTION_BACKEND=seed
export ENGINE_ENCRYPTION_KEY="$(openssl rand -base64 32)"
pnpm dev
```

Retain that seed before writing important secrets. `pnpm dev` runs Rust on
127.0.0.1:8080 and Vite on 127.0.0.1:5173. Vite proxies RPCs to Rust. Set `API_PORT`
and `WEB_PORT` to override them. Alternatively, a built executable can generate a
seed with `tilde generate-key`.

Rust settings are strongly typed in `crates/tilde/src/config.rs` using `envconfig`.
Development runs `tilde check-config` before starting listeners. Invalid settings
identify the variable without printing secret values. The packaged binary reads
only the process environment; automatic `.env` loading is limited to development.

## SQL queries and migrations

PostgreSQL SQL lives in `queries/{domain}/*.sql`. Cornucopia 1.0.1 checks query
parameters and result types against Postgres and generates `crates/queries`.
Each domain's `db` module exposes typed operations over a deadpool client or
transaction. Services own transaction boundaries, including locks and commits.

After changing SQL or adding a migration:

```sh
cargo install cornucopia --version 1.0.1 --locked
task queries:generate
```

The task starts disposable Postgres, applies `migrations/`, and regenerates the
bindings. Commit the SQL and `crates/queries` together. Neither ordinary Rust
builds nor Protobuf generation needs Postgres or Cornucopia installed.

CI verifies generated bindings and database access boundaries:

```sh
task queries:generate -- --check
task check:queries
```

The runtime migrator embeds the ordered central migrations, verifies SHA-384
checksums, and applies changes under a transaction-scoped lock. On upgrade it
renames the existing SQLx migration ledger to `_tilde_migrations`, preserving
applied versions without rerunning them. Migration bootstrap SQL is in
`queries/_migrations`; ClickHouse SQL is in `queries/_logs`. Neither is input
to the PostgreSQL query generator.

## License

Apache-2.0. See LICENSE.

## AWS KMS

KMS uses the copied `client-aws-config`, `client-aws-sigv4` and `client-aws-kms`
primitives. Configure `ENGINE_ENCRYPTION_BACKEND=aws-kms`, `ENGINE_KMS_KEY_ID`, and
`AWS_REGION` (or `AWS_DEFAULT_REGION`); unset `ENGINE_ENCRYPTION_KEY`. Supply AWS
credentials through environment variables, an ECS role or a shared credentials
file. See [credential sources and limitations](crates/client-aws-config/README.md).
Seed mode needs no AWS configuration or network calls.


See the [chat domain](CONTEXT.md#chat), the [TypeScript SDK and example agents](sdk/ts/README.md)
and the [Python SDK](sdk/py/README.md).

## Agent execution

Tilde never calls an agent. An agent process dials in with its deployment token
(`connectAgent` / `connect_agent`) and receives wakes as frames on that connection; a Lambda
deployment is invoked through the AWS invoke API instead. Each running invocation opens an
outbound invocation control stream to its callback URL, so steering, cancellation and
suspension reach that exact execution without host affinity.

The same protocol runs against a local sidecar. The SDK establishes its control
subscription before user code executes. Suspension uses a framework checkpoint hook
and ends execution; resume starts a new invocation. See the
[SDK README](sdk/ts/README.md).

## Deployments

Every agent has deployments: the declared record of what is running, tied to a commit,
created by CI with `DeploymentService.RegisterDeployment` (idempotent on your deployment
id). CI should pass the repository as
`owner/repo` or a full URL along with the commit SHA, message, branch and author; the
Deployments tab shows the commit message as the row title and links the SHA to the
commit on GitHub. Registering returns a token once; the
process that runs the deployment dials in with it. New threads route to the serving
deployment (`Latest` selects the newest; `Weighted` splits new conversations) and
existing threads stay pinned to theirs. Targets:

- **Gateway**: an agent process using `connectAgent` (TypeScript) or `connect_agent`
  (Python) that dials the gateway with its deployment token and receives wakes over that
  connection. Tilde never calls an agent, so the process exposes no endpoint and needs no
  signing key; deployments have no public URL.

- **Lambda**: the gateway invokes the function ARN asynchronously using its own AWS
  credentials; deploy `createLambdaHandler` from the SDK.
- **Sidecar**: replicas of `tilde-sidecar` dial in with the token; see below.

For GitHub Actions, use the [registration action](.github/actions/register-deployment/README.md)
to register CI releases and receive a masked deployment token. It also provides a
manual handoff link for users who configure runtime secrets themselves.

## Sidecar deployments

Gateway-only deployments need nothing beyond the gateway. To run an agent beside a
sidecar, register a **Sidecar** deployment in its **Deployment** tab and configure the
failure policy. Use the same deployment token for every replica of that release.

Register a sidecar deployment in the **Deployments** tab (or from CI with
`RegisterDeployment`) and use its token for every replica. The container image
(`ghcr.io/trytilde/tilde`) includes `tilde` and `tilde-sidecar`. Run the sidecar next to your SDK-hosted
agent process:

```bash
export ENGINE_SIDECAR_GATEWAY_URL=https://tilde.example.com
export ENGINE_SIDECAR_AGENT_TOKENS='<deployment-token>'
tilde-sidecar
```

The agent process dials the sidecar with the same token through the run protocol. Tilde
and its sidecars never call an agent, so the process exposes no endpoint:

```ts
connectAgent({
  gatewayUrl: "http://127.0.0.1:8081/agents/<agent-id>",
  deploymentToken: process.env.TILDE_DEPLOYMENT_TOKEN!,
  run,
});
```

A sidecar wake fails and is retried until that process is connected and heartbeating.
For multiple agents, separate tokens with commas. The sidecar keeps no state on disk. It dials the gateway's `sidecar` route group over one
connection, receives configuration, credentials and thread leases from that stream, and
publishes typed events back for projection into Postgres. The gateway never connects to
a sidecar, so replicas may sit behind NAT.

The sidecar binds two listeners: `ENGINE_SIDECAR_RUNTIME_LISTEN` (default
`127.0.0.1:8081`, loopback only) for the agent process, and `ENGINE_SIDECAR_LISTEN`
(default `0.0.0.0:8082`) for provider webhooks and native ingress. Agent routes are
prefixed with `/agents/<agent-id>`; provider webhooks use
`/agents/<agent-id>/connections/webhooks/<connection-id>`. Set `ENGINE_SIDECAR_PUBLIC_URL`
to the externally reachable origin of that listener and put a load balancer in front of
the replicas.

The replica that first receives work for a conversation takes its lease in the same
gateway call that hydrates it, and holds it while the conversation stays warm: turn state
lives in its memory, the agent process is invoked over loopback, channel replies use the
replicated credentials, and events on that conversation cost no further gateway calls on
the hot path. Registry calls the agent makes with its invocation token are verified
locally and relayed to the gateway, which checks the same token against the thread lease
and capability ceiling before answering. Requests that land on another replica are handed
straight to the holder, sidecar to sidecar; requests on the gateway are queued for the
holder and answered through the gateway. The gateway serves reads of sidecar
conversations from its projection, and completed messages in rooms with several sidecar
agents are relayed to each holder. Conversations idle for
`ENGINE_SIDECAR_THREAD_IDLE_SECONDS` leave memory and release their lease, and
`ENGINE_SIDECAR_CACHE_BYTES` (default 256 MiB per agent) evicts the least recently active
idle conversations early. Stopping a sidecar releases idle leases immediately and reports
the instance gone, so live work moves to another replica without waiting out the
liveness window.

Replicas heartbeat every three seconds; a holder unheard from for fifteen seconds loses
its leases. Under **Assign to new node** the gateway hands active runs to another live
replica, which restarts them from their objective. Under **Stop** the runs fail. A
replica whose publishes go unacknowledged for ten seconds stops executing, so a
partitioned holder never keeps running beside its replacement. Writes are acknowledged
from replica memory and reach Postgres when the replica publishes them in batches, so a
replica that dies loses the events it had not yet published, and a replica whose
outbox fills sheds streaming deltas first and then its oldest events (never heartbeats, lease releases or directive results) rather than failing
callers. External effects should therefore use idempotency keys.

Attachments use bounded sidecar memory (128 MiB per upload, 256 MiB total) until
uploaded to gateway S3 storage; configure the existing `ENGINE_S3_*` settings on the
gateway. Use `task dev:sidecar` for local development; `task test:sidecar` runs two
in-process replicas against Postgres and exercises leases, projection, gateway forwarding and
failover.

## Route groups and authentication

One listener (`ENGINE_LISTEN` / `--listen`, default `127.0.0.1:8080`) serves every
route group. `ENGINE_SERVE` / `--serve` selects the groups a process mounts:

| Group | Routes | Authentication |
| --- | --- | --- |
| `management` | Management RPCs, connection setup, embedded UI | None: put your own authenticating proxy in front |
| `runtime` | Agent runtime RPCs, invocation controls, OTLP uploads | Signed invocation connect token |
| `ingress` | Provider webhooks and native conversation ingress | Provider signatures or scoped ingress tokens |
| `sidecar` | The sidecar protocol | Agent deployment tokens |

The default is `all`. Operators who want network isolation run separate processes with
different `ENGINE_SERVE` values rather than separate ports. Binding outside loopback
requires `--allow-network`.

The management API and web UI have no login, users or API keys: anyone who reaches the
`management` group administers the installation. Never expose it directly; serve it behind
your own authenticating proxy (for example an identity-aware proxy, VPN or Tailscale), or
run a process without the group (`ENGINE_SERVE=ingress,runtime,sidecar`) on the public
network. Agent runtimes never receive management access.

The agent's Capabilities tab also controls how new messages are scheduled:
**Queue** finishes the active response then processes each message separately;
**Interrupt** cancels it and starts fresh work; **Queue and batch** combines pending
messages into the next response. The API owns this policy per agent/thread. Agent
handlers respond to one invocation and honor its cancellation signal.

`ENGINE_PUBLIC_URL` is also the default origin for provider webhooks and agent
callbacks. Set `ENGINE_INGRESS_PUBLIC_URL` when providers reach the engine through a
different origin, and `ENGINE_RUNTIME_PUBLIC_URL` when agent hosts do; the latter is
the callback URL delivered on invocation.

Provider OAuth callbacks and connection-brokering methods retain their own narrow
protocol credentials on the management API.

### Agent capabilities

Create/update agents with typed `capabilities` fields, or use the registry editor:

```json
{
  "toolsInvoke": {"mode": "TARGET_SELECTION_SELECTED", "ids": ["sendMessage"]},
  "workRead": "BINARY_PERMISSION_YES",
  "workWrite": "BINARY_PERMISSION_YES",
  "runUpdate": "BINARY_PERMISSION_YES",
  "agentsInvoke": {"mode": "TARGET_SELECTION_SELECTED", "ids": ["TARGET-AGENT-UUID"]}
}
```

`agentsCreate`, `threadRead`, `workRead`, `workWrite` and `runUpdate` use the
`BinaryPermission` enum (`NO` or `YES`). The other capability fields use
`TargetPermission`, with `TargetSelection.NONE`, `ALL`, or `SELECTED` and IDs
only for `SELECTED`. Agent targets use UUIDs; tool targets use catalog names.
Missing fields deny. Omitting capabilities on update preserves them; an empty
capabilities message clears them. Grants cannot exceed the grantor's authority.
The edit screen saves toggle changes immediately and agent selections on modal
confirmation. Tool-name edits save on blur or Enter. Failed saves restore the last
saved setting and display an error; creation submits its initial permissions with
the registration request.

Connect tokens expire after five minutes. The SDK renews them at four minutes
while the invocation and run are active, retaining or narrowing original authority.
RPC authorization verifies signed identity, participant and capability claims locally.
An issued token retains its authority until expiry; cancellation, pause, lease expiry
or lost access blocks renewal. Stop controls still abort cooperative SDK work immediately.
Agent updates that redirect endpoints cannot be used to
capture a more privileged agent's credentials.

The SDK exposes invocation-authenticated `ctx.agents` registry methods and
`ctx.invokeAgent({agentId, objective})` for agents already participating in the
current thread.

### Optional management routes and React serving

Set these independently in the process environment or development `.env`:

```dotenv
ENGINE_SERVE=all
ENGINE_WEB_ENABLED=true
```

`ENGINE_SERVE=ingress,runtime,sidecar` removes management and
connection-brokering routes. Agent-facing routes
and background workers continue running. The dev launcher exposes this switch as
`ENGINE_MANAGEMENT_ENABLED=false`.

`ENGINE_WEB_ENABLED=false` disables embedded React assets in packaged builds
and prevents Vite from starting under `task dev`. It does not disable management
RPCs.

In packaged builds the embedded UI is served by the same listener as the API. A
separately served UI needs its API paths proxied to an enabled management group;
dev Vite supports `ENGINE_DEV_URL` for that upstream. When web is disabled, the dev
public URL defaults to the API port rather than Vite's port.

### Example SDK agent

`REGISTER_DEV_AGENTS=1 task dev` registers and starts one example agent per SDK adapter:
the Vercel AI SDK, LangChain, OpenAI Agents and Mastra TypeScript agents under `dev/`, and the
LangChain, Pydantic AI, OpenAI Agents, Agno and CrewAI Python agents under `sdk/py/examples`. Each has
a stable registry ID and its own Gateway deployment; the launcher rotates that deployment's
token on every start and the process dials in with it, so no example listens on a port.
`DEV_AGENTS` (comma-separated keys from `.env.example`) limits which ones start. Existing
names and grants are preserved across restarts. Combine with `TS_ADDR` as usual.

Set `OPENAI_API_KEY` in `.env.local`. No key is
printed or passed in command arguments. `OPENAI_MODEL` defaults to `gpt-4o-mini`.
The registration command exists only in debug builds. See the small implementation
in `sdk/ts/examples/vercel-ai-example-agent/src`; the workspaces are built and synced before dev startup.

### Local provider webhooks with ngrok

Install the [ngrok CLI](https://ngrok.com/download), then add to `.env.local`:

```dotenv
NGROK_ENABLED=true
NGROK_DOMAIN=your-domain.ngrok-free.app
NGROK_AUTHTOKEN=your-token
```

Run `task dev`; ngrok forwards to the engine port at `ADDRESS:API_PORT`.
The launcher sets `ENGINE_INGRESS_PUBLIC_URL=https://NGROK_DOMAIN`, overriding any
configured ingress public URL while ngrok is enabled. This also updates the webhook
URLs displayed and copied in connection setup iframes. The tunnel forwards the whole
port, including the unauthenticated management routes: set `ENGINE_MANAGEMENT_ENABLED=false`
or restrict the tunnel when the domain is reachable by others.

All connection webhook URLs and provider manifests use the ingress public origin.
Existing provider webhook registrations must be updated to the new URL shown on
the connection; starting the engine does not rewrite provider registrations.
`/connections/webhooks/{connection_id}` belongs to the ingress group, which remains
active when management and web are disabled. Configure the externally reachable
origin with `ENGINE_INGRESS_PUBLIC_URL`.

Ngrok is disabled by default and belongs only to Task's dev launcher; the packaged
Rust server never starts it. It stops with Ctrl+C or a dev service failure.
`task dev:ngrok` can attach a dev tunnel separately; the running API must already
use the matching ingress public URL and port.

### Development over Tailscale

```sh
TS_ADDR=100.102.116.12 task dev
```

Task loads `.env.local` and `.env`. The small dev launcher applies
`TS_ADDR` to local management and ingress binds, Vite/HMR, browser origins, setup
URLs and local MinIO URLs, and enables network binding.
Custom external URLs and the agent runtime/database settings remain unchanged.
Ngrok still overrides only the ingress public URL.

Without `TS_ADDR`, dotenv values pass through directly. `.env.example` lists the
local defaults. When changing ports, update their matching listen addresses and
URLs together. When web is disabled, point `ENGINE_PUBLIC_URL` at the
management API.

`task dev` starts the project's persistent Compose Postgres and waits for it to
be healthy when `DATABASE_URL` uses the default local engine database
(`engine` user/database on localhost or 127.0.0.1 port 5432). Set
`POSTGRES_PASSWORD` to the same password used in `DATABASE_URL`; `.env.example`
provides matching development defaults. Port 5432 is published only on loopback.
Custom/external database URLs remain deployment-managed. `task dev:postgres` can
also start the default local database separately.

## Invocation tracing with Langfuse

Langfuse stores platform and agent traces. Configure it on the gateway:

```sh
LANGFUSE_BASE_URL=https://cloud.langfuse.com
LANGFUSE_PUBLIC_KEY=pk-lf-...
LANGFUSE_SECRET_KEY=sk-lf-...
# Optional browser-facing origin for self-hosted Langfuse:
# LANGFUSE_PUBLIC_URL=https://langfuse.example.com
```

Agents upload OTLP to `/v1/traces` on their runtime listener with their invocation
token. A sidecar stamps accepted spans with the verified invocation scope and ships
them to the gateway in its publish stream; the gateway forwards to Langfuse. Only the
gateway has Langfuse credentials. The management tracing tab reads from Langfuse;
Postgres does not retain permanent trace history.

The sidecar relay and gateway delivery queue are bounded. Sidecar batches leave memory
once the gateway accepts them, and gateway payloads are cleared after delivery.
Temporary gateway replay receipts suppress repeated batches; delivery retries use
Postgres notifications and scheduled deadlines. Overload returns retryable errors.
Gateway delivery records expire after seven days. Conversation retention settings
do not delete Langfuse history.

Without Langfuse configuration, tracing is disabled. Final agent uploads remain
authorized for five minutes after invocation end. See the
[tracing implementation](crates/tilde/src/tracing/README.md) for delivery and
scope details.

Agent creation can omit an endpoint. Deployment registration specifies its execution
type; Gateway and Sidecar connect outbound using their deployment token. Custom avatar uploads use the `ENGINE_S3_*`
settings in `.env.example`; `task dev` starts MinIO and creates the private avatar bucket.
`task dev:s3` starts only storage, and `task test:avatars` checks real S3 upload/read/replace.
When setting `ADDRESS` for Tailscale, `task dev` uses that address for signed avatar URLs.
For other deployments, set `ENGINE_S3_PUBLIC_ENDPOINT` if the browser-facing storage URL
is different from `ENGINE_S3_ENDPOINT`; Docker Compose also accepts `ENGINE_S3_CONTAINER_ENDPOINT`.


`task dev` starts local Langfuse at [http://127.0.0.1:3003](http://127.0.0.1:3003),
with web/worker, ClickHouse, Redis, its own Postgres database, and a separate MinIO
bucket. Development defaults live in `compose.yaml`; override them in the existing
`.env` file. No separate Langfuse environment file is generated. Database volumes
preserve data across restarts. Log in as `developer@tilde.local` with the default
password `tilde-local-password`, or your `LANGFUSE_DEV_LOGIN_PASSWORD` override.
Local project key overrides use `DEV_LANGFUSE_PUBLIC_KEY` and
`DEV_LANGFUSE_SECRET_KEY` so they do not select external-collector mode.
`LANGFUSE_PORT` changes the port; `TS_ADDR` sets the browser-facing address.
`DEV_LANGFUSE_ENABLED=0 task dev` skips local Langfuse.

To use Langfuse Cloud or another self-hosted instance, supply all three variables:

```sh
LANGFUSE_BASE_URL=https://cloud.langfuse.com
LANGFUSE_PUBLIC_KEY=pk-lf-your-project
LANGFUSE_SECRET_KEY=sk-lf-your-project
```

Complete external configuration skips the local stack. Optional
`LANGFUSE_PUBLIC_URL` supplies a different browser-facing address. Credentials
remain on the gateway. With no configuration, observability controls are disabled
and authenticated valid OTLP uploads are successfully discarded. Configured
outages retain queued telemetry and show an unavailable state.

The agent **Tracing** tab includes Observations and Sessions, model/tool/error
presets, time and input/output filters, column preferences, and a trace inspector.
Its Open in Langfuse links open the corresponding external view in a new tab.
The viewer uses Langfuse's supported public API: newest-first cursor pagination
and clearly labeled partial session groups.

Rotel accepts agent traces with connect tokens, preserving the five-minute final
upload window. A bounded temporary Postgres outbox supplies durable forwarding;
trace history lives in Langfuse. Payloads are unencrypted protobuf and are removed
after delivery; replay receipts expire after seven days. See the
[tracing boundary](crates/tilde/src/tracing/README.md) for delivery semantics and the
companion sidecar integration. The Postgres trace-history draft remains separate.

## Agent logs

`task dev` also starts a dedicated local ClickHouse service for agent log history.
The **Logs** tab on an agent supports time/severity/message filters, invocation and
trace correlation, record inspection, pagination, and a bounded live view.
Send instrumented OTel logs to the runtime callback URL plus `/v1/logs`, using the
agent connect token as a Bearer credential. Standard OTLP/HTTP JSON and protobuf
are supported; application stdout capture requires OTel instrumentation.

Set `DEV_LOGS_ENABLED=0` to skip local ClickHouse. To use an existing database, set
`LOGS_CLICKHOUSE_URL`, `LOGS_CLICKHOUSE_DATABASE`, `LOGS_CLICKHOUSE_USER`, and
`LOGS_CLICKHOUSE_PASSWORD` in `.env`. Tilde creates the log table in that database.
Optional `LOGS_OTLP_ENDPOINT` and `LOGS_OTLP_HEADERS` forward to another collector
independently of local storage. Without a storage URL, the history UI is disabled;
forwarding can remain enabled. Without either destination, valid authenticated
uploads are discarded.

Logs are batched into bounded, persistent disk delivery queues under
`LOGS_QUEUE_DIR`; use a distinct persistent volume for each gateway. Historical
logs never pass through application Postgres. Pending delivery expires after 24
hours, history after seven days. External forwarding can drop copies if its queue
fills while local storage continues. See the [logs boundary](crates/tilde/src/logs/README.md)
for limits, overload behavior, and sidecar semantics. Run `task test:logs` to test
against a disposable Postgres database and isolated ClickHouse database.
