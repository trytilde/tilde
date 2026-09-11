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

## IAM and API listeners

One Tilde process serves three APIs:

| API | Default bind | Authentication |
| --- | --- | --- |
| Management API | `127.0.0.1:8080` | User bearer session from OIDC |
| Agent runtime API | `127.0.0.1:8081` | Signed invocation connect token |
| Event ingress API | `127.0.0.1:8082` | Provider webhook signatures |

Every user admitted by the configured OIDC provider has unrestricted management
access. There is no role model or API-key support. Configure the provider's login
admission policy accordingly. Agent runtimes never receive user tokens.

Configure `ENGINE_OIDC_ISSUER`, `ENGINE_OIDC_CLIENT_ID` and
`ENGINE_OIDC_CLIENT_SECRET`. Register
`<ENGINE_MANAGEMENT_PUBLIC_URL>/auth/callback` at the provider. The management
public URL is the browser-facing origin, including Vite's port in development.
Production requires HTTPS; `ENGINE_OIDC_ALLOW_HTTP=true` is for local development.
The initial OIDC implementation validates RS256 ID tokens.

Use `ENGINE_MANAGEMENT_LISTEN` / `--management-listen` and
`ENGINE_AGENT_RUNTIME_LISTEN` / `--agent-runtime-listen` for bind addresses.
`ENGINE_AGENT_RUNTIME_PUBLIC_URL` must be reachable by agent servers and is the
callback URL delivered on invocation. All listeners require `--allow-network`
when binding outside loopback. These names replace `ENGINE_LISTEN`,
`ENGINE_PUBLIC_URL` and `--listen`.

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

### Optional management API and React serving

Set these independently in the process environment or development `.env`:

```dotenv
ENGINE_MANAGEMENT_ENABLED=true
ENGINE_WEB_ENABLED=true
```

Both default to `true`. `ENGINE_MANAGEMENT_ENABLED=false` removes management,
OIDC and connection-brokering routes and makes OIDC configuration optional.
The agent runtime API and background workers continue running.

`ENGINE_WEB_ENABLED=false` disables embedded React assets in packaged builds
and prevents Vite from starting under `pnpm dev`. It does not disable management
RPCs or OIDC endpoints. Development starts Dex only when management is enabled.

In packaged builds, React shares `ENGINE_MANAGEMENT_LISTEN`: with management off
and web on, that address serves the UI and health endpoints only. With both off,
it is not bound at all. Builds without the `embedded-web` feature do not bind an
extra listener for static assets. A separately served UI needs its API/auth paths
proxied to an enabled management API; dev Vite supports `ENGINE_DEV_URL` for that
upstream. When web is disabled, the dev management public URL defaults to the API
port rather than Vite's port.

Development orchestration lives in `Taskfile.yml`; `pnpm dev` and `pnpm build`
are aliases for `task dev` and `task build`. Task runs the API and Vite in parallel
with live interleaved output after configuration validation and optional Dex startup.
A service failure stops the other development services (fail-fast). Ctrl+C also
stops all processes.
`task sqlx:prepare` owns migration/query preparation; use `task sqlx:prepare --
--check` to verify metadata. Test sequencing also lives in Task; the retained
Postgres shell wrapper owns temporary database allocation and cleanup. JavaScript
scripts remain for code-generator installation, generated-file verification and
protocol/process integration tests.

### Local provider webhooks with ngrok

Install the [ngrok CLI](https://ngrok.com/download), then add to `.env.local`:

```dotenv
NGROK_ENABLED=true
NGROK_DOMAIN=your-domain.ngrok-free.app
NGROK_AUTHTOKEN=your-token
```

`task secrets:load` also loads `ngrok_authtoken` from SOPS into the private dev
dotenv file. Run `task dev`; ngrok forwards only to event ingress at
`ADDRESS:INGRESS_PORT` (default port `8082`). The launcher sets
`ENGINE_EVENT_INGRESS_PUBLIC_URL=https://NGROK_DOMAIN`, overriding any configured
ingress public URL while ngrok is enabled. This also updates the webhook URLs
displayed and copied in connection setup iframes. The dashboard, setup UI and OAuth redirects remain
on management; ngrok never forwards to management or the agent runtime API.

All connection webhook URLs and provider manifests use the event ingress origin.
Existing provider webhook registrations must be updated to the new URL shown on
the connection; starting the listener does not rewrite provider registrations.
`/connections/webhooks/{connection_id}` is served only on event ingress, which
remains active when management and web are disabled. Deployments can expose this
listener while keeping management private. Configure its bind with
`ENGINE_EVENT_INGRESS_LISTEN` or `--event-ingress-listen`, and its externally
reachable origin with `ENGINE_EVENT_INGRESS_PUBLIC_URL`.

Ngrok is disabled by default and belongs only to Task's dev launcher; the packaged
Rust server never starts it. It stops with Ctrl+C or a dev service failure.
`task dev:ngrok` can attach a dev tunnel separately; the running API must already
use the matching event ingress public URL and port.

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

Tilde embeds Rotel to forward platform and agent spans to your collector. Enable
tracing by setting the complete OTLP/HTTP protobuf traces URL:

```sh
ENGINE_TRACING_EXPORT_ENDPOINT=https://collector.example.com/v1/traces
ENGINE_TRACING_EXPORT_HEADERS='Authorization=Bearer your-collector-token'
```

Without an endpoint, tracing is disabled. Agents send OTLP/HTTP to `/v1/traces`
on `ENGINE_AGENT_RUNTIME_PUBLIC_URL` with their connect token. The TypeScript SDK
propagates context across model/tool instrumentation and streamed callbacks.
Webhook/API ancestry survives durable message scheduling and invocation dispatch.

Rotel batches asynchronously through bounded queues. Ingestion success means
acceptance into memory; collector failures retry for up to five minutes per batch.
Overload returns retryable 503 responses. There is no local trace storage or replay,
and queued telemetry may be lost on process failure or exhausted retry budgets.
Final agent uploads remain authorized for five minutes after invocation end.

See [tracing implementation](crates/tilde/src/tracing/README.md) for queue limits,
authentication and delivery semantics. Postgres trace storage is a separate follow-up.

Agent creation requires an HTTP(S) endpoint. Custom avatar uploads use the `ENGINE_S3_*`
settings in `.env.example`; `task dev` starts MinIO and creates the private avatar bucket.
`task dev:s3` starts only storage, and `task test:avatars` checks real S3 upload/read/replace.
When setting `ADDRESS` for Tailscale, `task dev` uses that address for signed avatar URLs.
For other deployments, set `ENGINE_S3_PUBLIC_ENDPOINT` if the browser-facing storage URL
is different from `ENGINE_S3_ENDPOINT`; Docker Compose also accepts `ENGINE_S3_CONTAINER_ENDPOINT`.
