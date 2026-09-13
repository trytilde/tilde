# Tilde

Tilde aims to be the ubiqiuotous meta-harness for AI agents. Build AI agents in your framework of choice, register them in the Tilde Agent registry and manage them from a single place. Today we offer an agent registry to see and manage deployed agents, integrations with chat channels to forward messages from email, Whatsapp or realtime audio to your agent and an audit trail of all agent traces & conversations that you can export.

We plan to add:

- Tools (managed third party integrations, proxy external MCP servers, deploy custom tool endpoints)
- Memory
- Skills (centrally manage all skills available to agents in the registry)

## Build and run

Install [Task](https://taskfile.dev/docs/installation), then run:

```sh
task build  # Complete release build, including embedded React UI
task dev    # Rust API plus Vite; uses the configuration documented below
task test   # Rust and Connect client tests against disposable Postgres
task check  # Generated code, frontend types, formatting and Clippy
```

Build and dev install dependencies and generate contracts first. The release
binary is `target/release/tilde` (or under `CARGO_TARGET_DIR` when configured).
Task loads committed `.env` defaults and private `.env.secrets` / `.env.local` overrides. Set `DATABASE_URL` and
encryption configuration there; exported environment variables take precedence.
Use `API_PORT` and `WEB_PORT` to select development ports. `task generate` runs
contract generation independently of Rust compilation. `task --list` lists commands.

## Versioning

[Changie](https://changie.dev/) manages one shared release version for the Rust
workspace (including `tilde --version`) and every package in `sdk/ts/packages/*`.
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
lockfiles offline, and checks version agreement. Install dependencies first with
`task setup` and `cargo fetch --locked`. Review and commit the generated changes
together before tagging `v<version>` and building/publishing the release artifacts.
`release:prepare` only prepares versions locally. `task sdk:pack` builds public SDK
tarballs in `dist/npm`; `task sdk:publish` publishes unpublished workspace packages
to npm (`-- --dry-run` previews publishing).

### CI and publishing

- **Changie** requires a new PR fragment, validates fragments with Changie's batch
  dry-run, checks versions, and verifies that `changie merge` produces no drift.
- **Check** runs generation, compatibility, formatting, Clippy, SQLx, and integration checks.
- **Build** builds and smoke-tests Linux x86-64 and macOS ARM64 binaries with the
  embedded UI, uploads archives/checksums and npm tarballs, and builds/tests the
  Linux amd64 Docker image without pushing it.
- **Create Release PR** is manually dispatched on the default branch. Select a
  patch/minor/major/auto bump; it updates `codex/release` with versions, lockfiles,
  release notes, and a blank acknowledgement so the PR passes the fragment rule.
- **Release** is manually dispatched on the default branch after merging that PR.
  It reruns Changie, Check, and Build against the dispatch commit, creates the
  `v<version>` tag and a draft release with artifacts, publishes public SDK packages
  to npm and `ghcr.io/<owner>/<repository>:<version>` plus `:latest`, then makes the
  GitHub release public. Rust crates remain internal build dependencies; the Rust
  distribution is the binary/container, not crates.io packages.

Configure `GIT_BOT_TOKEN` with repository contents and pull-request write access
for release PR creation; a bot token allows the PR to trigger CI. Configure
`NPM_TOKEN` with publish access to the `@trytilde` scope (including any new packages)
and the npm permissions needed for unattended publishing. GHCR and GitHub Releases
use the repository's `GITHUB_TOKEN`; allow Actions to write packages and releases.
Require the Changie, Check, and Build checks before merging release PRs.

Publishing currently accepts stable semantic versions only. All public SDK packages
release together and pnpm skips versions already published. If publishing partially
fails, rerun the failed jobs for the **same workflow run/commit**; the GitHub release
stays a draft until both registries succeed. A tag pointing at another commit is
rejected, and a completed release cannot be republished through this workflow.
Registry publication is not atomic: one registry can succeed while another fails.

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
Postgres 16+. Docker runs local development Postgres, Dex, and the disposable test harness.

```sh
pnpm install --frozen-lockfile
pnpm tools
pnpm generate
```

`pnpm tools` downloads pinned official code-generator binaries and checks their
SHA-256 digests. `pnpm generate` runs Buf and the generators directly. It needs
**no Rust compiler, application build, running server, database or cloud keys**.
JavaScript and TypeScript declaration output works in both React and Node.
Generated code is checked in; ordinary Cargo builds need neither Node nor Buf.
SQLx uses checked-in offline metadata, so Rust compilation also needs no database.

The repository commits `.env` and `.env.test` as development and test defaults.
The development seed is public and suitable only for disposable local data. For
persistent data, generate a seed with `tilde generate-key` or `openssl rand -base64 32`
and retain it in ignored `.env.local`. Never replace the seed for an existing database.

`secrets.enc.yaml` is the original Tilde SOPS document, encrypted with the existing
AWS KMS key; `.sops.yaml` defines that recipient for future edits. Its top-level
credentials feed development, and its `test` map overrides them for tests. The
loader extracts only chat-provider credentials, so legacy database/infrastructure
settings do not override this runtime's defaults. Decrypted files stay ignored.

```sh
# Authenticate to the KMS-owning AWS account first (the original profile is shown).
aws sso login --profile daniel_soma_shared_admin
AWS_PROFILE=daniel_soma_shared_admin task secrets:load
AWS_PROFILE=daniel_soma_shared_admin task secrets:edit # edit and refresh both dotenv files

task dev
task test:chat       # all six adapters, local HTTP fixtures and disposable Postgres
task test:chat:accounts # read-only checks of configured live provider accounts
task test:chat:live -- github # real two-way messages; optional provider name filter
```

Requires SOPS and Python 3 for secret loading; ordinary fixture tests require no
AWS access or provider secrets. Exported variables take precedence, followed by
`.env.local`, `.env.secrets`, then `.env` for development. Test tasks instead use
`.env.test.local`, `.env.secrets.test`, then `.env.test`; the disposable Postgres
wrapper supplies the actual test database URL. Task parses dotenv values as data;
do not shell-source the generated files, which may contain multiline PEM keys.

The fixture suite checks Slack, GitHub, AgentMail, Linq, Meta WhatsApp and Telnyx
WhatsApp through encrypted connections, agent tools, upstream HTTP, signed inbound
webhooks and the stored conversation audit. It covers duplicate callbacks, rejected
signatures and upstream send failures. Live tests are ignored by ordinary `cargo test`
and fail on missing credentials rather than silently skipping.

Both live targets require Cargo feature `live-chat-tests`; they also remain ignored
so `cargo test --all-features` cannot send messages accidentally. Task supplies the
feature and `--ignored`. `test:chat:accounts` retains the read-only account checks.
`test:chat:live` runs one two-way test per adapter, sequentially, and accepts a test
name filter after `--` (for example `slack`, `github`, or `agentmail`).

Two-way tests call `ListTools` and `InvokeTool` over the real local ConnectRPC API,
resolve encrypted credentials, send through the production provider adapter, check
the accepted tool result and durable tool audit, and receive a real peer response.
Following the original tilde-api tests, Hookdeck captures provider callbacks; the
suite fetches the original raw body and headers and replays them into the local
webhook endpoint **without re-signing or fabricating events**. The callback must
pass provider signature/account checks, appear in the outbound conversation, and
remain a single message after duplicate replay. This tests the application ingress;
it does not test a public tunnel or production deployment's network routing.

Slack reads back the outbound message with the bot token and replies with the test
user token, explicitly mentioning the app. The peer must be human-authored because
the adapter ignores bot events. If Slack marks the token's messages as bot-authored,
use `CHAT_LIVE_SLACK_MANUAL_REPLY=true task test:chat:live -- slack` and reply in the
new test thread with the printed marker while mentioning the app. GitHub reads back the App's comment and replies with the test PAT.
AgentMail creates a temporary peer inbox and a webhook limited to the test inbox,
then verifies delivery and sends a real email reply. Cleanup deletes Slack test
messages and temporary AgentMail inbox/webhook resources, and closes the GitHub
fixture issue, even when assertions fail. Phone messages and the primary email
inbox's test conversation remain as test artifacts.

Linq, Meta WhatsApp and Telnyx WhatsApp need a test recipient to reply with the
unique marker printed by the test within 180 seconds. For WhatsApp, that recipient
must message the business line before the run to open the messaging window. These
are assisted live tests, not simulated peer responses. Missing prerequisites fail
before sending; never point these tests at production recipients.

Set missing values in ignored `.env.test.local`:

| Provider | Additional two-way prerequisites |
| --- | --- |
| All | `HOOKDECK_API_KEY`, `CHAT_LIVE_<PROVIDER>_SOURCE_ID` with real provider webhook delivery enabled |
| Slack | `E2E_CHATKIT_SLACK_USER_TOKEN` with `chat:write`; existing bot token needs channel history/read and write scopes |
| GitHub | `E2E_CHATKIT_GITHUB_PAT` with issue/comment access to the test repo; existing GitHub App credentials and webhook secret |
| AgentMail | `CHAT_LIVE_AGENTMAIL_SOURCE_URL` for the dedicated Hookdeck source; API key must create/delete temporary inboxes and webhooks |
| Linq | Valid `E2E_MCP_LINQ_API_TOKEN`, `CHAT_LIVE_LINQ_PHONE_NUMBER`, `E2E_LINQ_WEBHOOK_SIGNING_SECRET`, and an available peer in `E2E_MCP_LINQ_CHAT_ID` |
| Meta WhatsApp | `CHAT_LIVE_WHATSAPP_RECIPIENT`, Meta webhook subscription to its source, existing Meta credentials |
| Telnyx WhatsApp | `CHAT_LIVE_TELNYX_RECIPIENT`, Telnyx webhook subscription to its source, existing Telnyx credentials/public key |

The original Slack, GitHub, Linq and development WhatsApp source IDs are in
`.env.test`, alongside a dedicated AgentMail capture source. Capture sources are
retained for repeatable runs; the AgentMail provider subscription is temporary. Native API tools
continue to run in `task test:chat`; the live suite also exercises the ConnectRPC
boundary on every provider send.

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

Application SQL lives in `queries/{domain}/*.sql` and is executed with SQLx's
`query_file!` / `query_file_as!` macros. SQLx checks parameter and result types
against Postgres during preparation. All domains share `migrations/`; adding a
domain does not add another migration runner.

After changing SQL or adding a central migration, refresh the metadata:

```sh
cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres
pnpm --dir web build
task sqlx:prepare
```

The task starts disposable Postgres, applies the central migrations, checks all
Rust targets/features and writes `.sqlx/`. Commit the SQL files and `.sqlx/`
together. `.cargo/config.toml` enables offline compilation by default; preparation
explicitly enables the database connection. Query preparation compiles Rust;
**Protobuf client generation remains independent of Rust and Postgres**.

CI checks metadata freshness against a newly migrated database:

```sh
task sqlx:prepare -- --check
```

## License

Apache-2.0. See LICENSE.

## AWS KMS

KMS uses the copied `client-aws-config`, `client-aws-sigv4` and `client-aws-kms`
primitives. Configure `ENGINE_ENCRYPTION_BACKEND=aws-kms`, `ENGINE_KMS_KEY_ID`, and
`AWS_REGION` (or `AWS_DEFAULT_REGION`); unset `ENGINE_ENCRYPTION_KEY`. Supply AWS
credentials through environment variables, an ECS role or a shared credentials
file. See [credential sources and limitations](crates/client-aws-config/README.md).
Seed mode needs no AWS configuration or network calls.


See the [chat domain](CONTEXT.md#chat) and the [TypeScript SDK and example agent](sdk/ts/README.md).

## Serverless agent execution

Agents may export the SDK's `createAgentHandler` from a Node serverless route,
including a Vercel deployment. The gateway invokes the deployment URL once; the
running request opens an outbound invocation control stream to its callback URL.
Steering, cancellation and suspension reach that exact execution through the stream,
so subsequent requests do not need to land on the same host instance.

The same protocol runs against a local sidecar. The SDK must establish its control
subscription before user code executes. Suspension uses a framework checkpoint hook
and ends execution; resume starts a new invocation. See the
[SDK serverless setup](sdk/ts/README.md#serverless-invocation-controls).

## Sidecar deployments

Gateway-only deployments need nothing beyond the gateway. To run an agent beside a
sidecar, choose **Sidecar** in its **Deployment** tab, select the failure policy, and
issue a deployment token. Use the same token for every replica of that agent. Pause an
existing agent before changing its mode.

The release archive and container include `tilde` and `tilde-sidecar`. Run the sidecar
next to your SDK-hosted agent process:

```bash
export ENGINE_SIDECAR_GATEWAY_URL=https://tilde.example.com
export ENGINE_SIDECAR_AGENT_TOKENS='<deployment-token>'
export ENGINE_SIDECAR_AGENT_ENDPOINTS='<agent-id>=http://127.0.0.1:3000'
tilde-sidecar
```

For multiple agents, separate tokens and `agent-id=endpoint` entries with commas. The
sidecar keeps no state on disk. It dials the gateway's `sidecar` route group over one
connection, receives configuration, credentials and ownership from that stream, and
publishes typed events back for projection into Postgres. The gateway never connects to
a sidecar, so replicas may sit behind NAT.

The sidecar binds two listeners: `ENGINE_SIDECAR_RUNTIME_LISTEN` (default
`127.0.0.1:8081`, loopback only) for the agent process, and `ENGINE_SIDECAR_LISTEN`
(default `0.0.0.0:8082`) for provider webhooks and native ingress. Agent routes are
prefixed with `/agents/<agent-id>`; provider webhooks use
`/agents/<agent-id>/connections/webhooks/<connection-id>`. Set `ENGINE_SIDECAR_PUBLIC_URL`
to the externally reachable origin of that listener and put a load balancer in front of
the replicas.

The replica that first receives work for a conversation owns it: turn state lives in its
memory, the agent process is invoked over loopback, and channel replies use the
replicated credentials. Registry calls the agent makes with its invocation token are
verified locally and relayed to the gateway, which checks the same token against its
own ownership record and capability ceiling before answering. Requests that land on another replica, or on the gateway, are
executed by the owner and answered through the gateway. The gateway serves reads of
sidecar conversations from its projection, and completed messages in rooms with several
sidecar agents are relayed to each owner.

Heartbeats travel with every publish; an owner unheard from for fifteen seconds loses its
conversations. Under **Assign to new node** the gateway hands active runs to another live
replica, which restarts them from their objective. Under **Stop** the runs fail. A
replica that cannot reach the gateway stops executing after thirty seconds, so a
partitioned owner never keeps running beside its replacement. Writes are acknowledged
from replica memory and reach Postgres when the replica publishes them, so a replica
that dies loses the events it had not yet published. External effects should therefore
use idempotency keys.

Attachments use bounded sidecar memory (128 MiB per upload, 256 MiB total) until
uploaded to gateway S3 storage; configure the existing `ENGINE_S3_*` settings on the
gateway. Use `task dev:sidecar` for local development; `task test:sidecar` runs two
in-process replicas against Postgres and exercises ownership, projection, forwarding and
failover.

## Route groups and authentication

One listener (`ENGINE_LISTEN` / `--listen`, default `127.0.0.1:8080`) serves every
route group. `ENGINE_SERVE` / `--serve` selects the groups a process mounts:

| Group | Routes | Authentication |
| --- | --- | --- |
| `management` | Management RPCs, OIDC, connection setup, embedded UI | User bearer session from OIDC |
| `runtime` | Agent runtime RPCs, invocation controls, OTLP uploads | Signed invocation connect token |
| `ingress` | Provider webhooks and native conversation ingress | Provider signatures or scoped ingress tokens |
| `sidecar` | The sidecar protocol | Agent deployment tokens |

The default is `all`. Operators who want network isolation run separate processes with
different `ENGINE_SERVE` values rather than separate ports. Binding outside loopback
requires `--allow-network`.

Every user admitted by the configured OIDC provider has unrestricted management
access. There is no role model or API-key support. Configure the provider's login
admission policy accordingly. Agent runtimes never receive user tokens.

Configure `ENGINE_OIDC_ISSUER`, `ENGINE_OIDC_CLIENT_ID` and
`ENGINE_OIDC_CLIENT_SECRET`. Register `<ENGINE_PUBLIC_URL>/auth/callback` at the
provider. The public URL is the browser-facing origin, including Vite's port in
development. Production requires HTTPS; `ENGINE_OIDC_ALLOW_HTTP=true` is for local
development. The initial OIDC implementation validates RS256 ID tokens.

`ENGINE_PUBLIC_URL` is also the default origin for provider webhooks and agent
callbacks. Set `ENGINE_INGRESS_PUBLIC_URL` when providers reach the engine through a
different origin, and `ENGINE_RUNTIME_PUBLIC_URL` when agent hosts do; the latter is
the callback URL delivered on invocation.

The browser stores its eight-hour user token in local storage and sends it through
`Authorization: Bearer`; no authentication cookies are issued or accepted. Logout
revokes the session. OIDC login/exchange endpoints are on the management API only.
Provider OAuth callbacks and connection-brokering methods retain their own narrow
protocol credentials on the management API.

### Local Dex login

`pnpm dev` starts Dex from the Compose `dev` profile and supplies its local OIDC
configuration when none is specified. Docker is required for this development
login provider. Sign in with **`dev@tilde.local` / `password`**. Its client secret
is the development-only `tilde-local-dev-secret`. The configuration is in
`dev/dex.yaml`; it uses in-memory storage and defaults to loopback port 5556.
A production Compose engine should use an externally reachable identity provider.

Task configures Dex redirects from the selected address and API/Vite ports.
Localhost redirects for the default ports are also retained.
To start only Dex manually: `POSTGRES_PASSWORD=unused-local-dev docker compose
--profile dev up -d dex`. Stop it with the same Compose profile and `stop dex`.

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

Connect tokens expire after 15 minutes. The SDK renews them every five minutes
while the invocation is active, retaining or narrowing its original authority.
Capability edits affect newly issued tokens; an existing token retains its
snapshot until expiry. Cancellation or lease expiry immediately denies further
requests and renewal. Agent updates that redirect endpoints cannot be used to
capture a more privileged agent's credentials.

The SDK exposes invocation-authenticated `ctx.agents` registry methods and
`ctx.invokeAgent({agentId, objective})` for agents already participating in the
current thread. Management clients accept an OIDC-derived `accessToken` option.

### Optional management routes and React serving

Set these independently in the process environment or development `.env`:

```dotenv
ENGINE_SERVE=all
ENGINE_WEB_ENABLED=true
```

`ENGINE_SERVE=ingress,runtime,sidecar` removes management, OIDC and
connection-brokering routes and makes OIDC configuration optional. Agent-facing routes
and background workers continue running. The dev launcher exposes this switch as
`ENGINE_MANAGEMENT_ENABLED=false`.

`ENGINE_WEB_ENABLED=false` disables embedded React assets in packaged builds
and prevents Vite from starting under `task dev`. It does not disable management
RPCs or OIDC endpoints. Development starts Dex only when management is enabled.

In packaged builds the embedded UI is served by the same listener as the API. A
separately served UI needs its API/auth paths proxied to an enabled management group;
dev Vite supports `ENGINE_DEV_URL` for that upstream. When web is disabled, the dev
public URL defaults to the API port rather than Vite's port.

### Local provider webhooks with ngrok

Install the [ngrok CLI](https://ngrok.com/download), then add to `.env.local`:

```dotenv
NGROK_ENABLED=true
NGROK_DOMAIN=your-domain.ngrok-free.app
NGROK_AUTHTOKEN=your-token
```

`task secrets:load` also loads `ngrok_authtoken` from SOPS into the private dev
dotenv file. Run `task dev`; ngrok forwards to the engine port at `ADDRESS:API_PORT`.
The launcher sets `ENGINE_INGRESS_PUBLIC_URL=https://NGROK_DOMAIN`, overriding any
configured ingress public URL while ngrok is enabled. This also updates the webhook
URLs displayed and copied in connection setup iframes. Management and runtime routes on
the tunnelled port still require their own credentials.

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
task dev ADDRESS=100.102.116.12
```

`ADDRESS` is the machine's IPv4 bind/public address; it defaults to `127.0.0.1`.
Open `http://100.102.116.12:5173` from your browser. Task binds the management API
(port 8080), web UI (5173), provider setup UI/HMR (5174), and local Dex (5556) to
that address. It configures public URLs, accepted browser origins, API proxying
and Dex's issuer/redirects together, and enables the engine's network binding.
The agent runtime API stays on loopback by default; Postgres configuration is
unchanged. The existing management/web enable switches still apply.

Ports remain configurable independently:

```sh
task dev ADDRESS=100.102.116.12 WEB_PORT=15173 API_PORT=18080 CONNECTION_UI_PORT=15174
```

A configured external OIDC issuer is preserved; Task only relocates the default
local Dex provider. The local Dex account remains `dev@tilde.local` / `password`.

`task dev` starts the project's persistent Compose Postgres and waits for it to
be healthy when `DATABASE_URL` uses the default local engine database
(`engine` user/database on localhost or 127.0.0.1 port 5432). Set
`POSTGRES_PASSWORD` to the same password used in `DATABASE_URL`; `.env.example`
provides matching development defaults. Port 5432 is published only on loopback.
Custom/external database URLs remain deployment-managed. `task dev:postgres` can
also start the default local database separately.

## Invocation tracing

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

Agent creation requires an HTTP(S) endpoint. Custom avatar uploads use the `ENGINE_S3_*`
settings in `.env.example`; `task dev` starts MinIO and creates the private avatar bucket.
`task dev:s3` starts only storage, and `task test:avatars` checks real S3 upload/read/replace.
When setting `ADDRESS` for Tailscale, `task dev` uses that address for signed avatar URLs.
For other deployments, set `ENGINE_S3_PUBLIC_ENDPOINT` if the browser-facing storage URL
is different from `ENGINE_S3_ENDPOINT`; Docker Compose also accepts `ENGINE_S3_CONTAINER_ENDPOINT`.

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
