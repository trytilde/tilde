
## Release version

The Rust workspace and TypeScript SDK packages share one Changie release version.
`VERSION`, Cargo workspace/package lock entries, and `sdk/ts/packages/*` versions
must agree. Internal SDK dependencies use unversioned pnpm workspace protocols;
pnpm resolves published ranges, while a shared release bumps every SDK package.
Private frontend and example packages are not independently released. Changie
records release intent and updates manifests; it does not infer dependency impact.
Release PRs batch that intent. A manual release of the merged default-branch commit
checks and builds it before tagging, publishes the SDK to npm and the binary image
to GHCR, and exposes GitHub release artifacts only when publishing succeeds. Tags
identify the checked commit; a partially published release is retried at that commit.

## Connections

For local provider callbacks, `task dev` optionally runs an ngrok CLI tunnel
(`NGROK_ENABLED`, `NGROK_DOMAIN`, `NGROK_AUTHTOKEN`) to the event ingress API.
Its HTTPS domain overrides `ENGINE_PUBLIC_EVENT_INGRESS_PUBLIC_URL` while enabled, including
explicit local URL settings, for all connection
webhooks and provider manifests. Management owns setup links and OAuth callbacks
on its own origin; ngrok is started only by the development launcher.
Task owns tunnel shutdown, and the SOPS loader exports its token only for dev.

Connections own self-managed credential lifecycle, encrypted Postgres storage and
short-lived connection setup tokens. Providers have a BuiltIn, Configured or Remote kind and one current catalog definition;
custom providers can declare static fields or standard OAuth flows at runtime.
Capabilities belong to the provider and connection type; the only current capability
is `channel`. Chat owns channel binding, subscriptions and message delivery.

Built-in provider definitions, React iframe pages, app manifests, verification and refresh
live under `connections/catalog/<provider>/`. Catalog adapters implement the setup
runtime boundary. The shared broker owns claims, expiry, encrypted staging, atomic
completion and recovery; it does not interpret provider callback fields or phases.
Generic OAuth implements protocols, while provider-specific assertions stay local.
Custom provider UI lives in `catalog/<provider>/ui.tsx`, built from a shared HTML
template and served at `/catalog/<provider>/ui`. Static and OAuth methods use the
shared `/catalog/_standard/ui` page and a canonical JSON Schema for rendering and
server validation. Rust serves the setup host/assets and proxies Vite during development.
Opaque-origin frames use a scoped MessagePort bridge; only the host holds the setup token.
The agent's chat-provider pills list catalog connection types with the channel capability.
Each menu offers an available existing connection or a new setup method. The new-connection
dialog opens the hosted brokering iframe immediately. Creation assigns chat to the agent
atomically using a temporary provider title; the iframe collects the actual account name
alongside credentials. Providers own `account_name_label`. SetConnectionName authenticates
the setup token and rotates its action before credential submission, keeping display names
out of arbitrary credential fields. Setup pages share the sidebar's Tilde wordmark, followed
by × and the provider icon, with “Connect {provider} to Tilde”, instructions, then fields.
There is no user-facing Save draft control.

Connection setup overviews remain provider catalog `instructions`. Ordered
`setup_instructions` come from `Runtime::instructions(&ConnectionType)` on the
adapter selected by provider and connection type. The iframe renders them below
its overview. Channel-capable setups expose a read-only Webhook URL below the
account name with a copy action, rather than generic webhook guidance appended
to every setup. The field is not a credential or submitted form value.
`Runtime::account_name_field(&ConnectionType)` optionally binds the account name
to a credential field. The shared broker omits that field from the projected form,
rejects client-supplied overrides, then injects the saved account name before full
schema validation and encrypted staging. The canonical schema retains the field
and its constraints. AgentMail binds `inbox_id`; Linq and Telnyx bind `phone_number`.
Unmapped connection types keep account names separate from credentials.
Assigned cards show that account name, provider branding, and a status-colored badge. Provider `icon_url`
and plain-text `instructions` are persisted catalog metadata and included in broker state.
Pills, cards and setup iframes use that icon URL; the iframe renders instructions directly
beneath its title. The modal has no header or close icon and dismisses on an outside click,
including while its initial setup request is in flight. Dismissal never cancels credentials.
Built-in icons use theSVG's jsDelivr URLs where available and official provider favicons
otherwise. Missing images receive a neutral frontend fallback.
The outer frame is the trusted broker; provider code remains in its opaque-origin child.
Completion/cancellation notifications are checked against the frame window, origin and
connection ID. Embedded authorization redirects and manifest posts open a separate window;
the original provider UI polls setup state through its existing bridge to follow callbacks.
A connection setup token authorizes only its setup session. Browser clients open the
returned brokering URL; the setup client does not send management bearer credentials.
OAuth callback state is separate from the connection setup token.

Setup exposes GetSetup, SaveDraft, StartOAuth, SaveCredentials,
ExecuteProviderAction and CancelSetup through ConnectRPC. Drafts are encrypted in a
separate relation from staged credentials and are erased at completion/cancellation.
OAuth state and PKCE remain server-owned; the generic callback accepts query redirects
and form_post without management authentication, while verifying the setup nonce.
Remote providers declare a backend and UI distribution base URL. Dedicated backend
credentials are encrypted and never returned by catalog reads. UI assets are proxied
without credentials into the same opaque-origin iframe. Exceptional remote provisioning
uses ConnectionProviderService.HandleSetup, returning await-input or credentials;
callbacks carry draft context and rotate their nonce between remote actions. Generic
static/OAuth definitions need no remote state machine. The browser SDK subpath
`@trytilde/sdk/connection-setup` supplies the scoped iframe client, and
`@trytilde/sdk/connection-provider` supplies the authenticated remote server helper.

`@trytilde/connection-ui` contains the React setup hooks, Tailwind theme and small
shadcn-style control set. Provider credential forms use React Hook Form; providers
still author their own TSX and interactions. No per-provider HTML source is required.
Connection setup tokens expire after ten minutes. Startup cleanup and a periodic
Tokio task purge expired setup rows, including completed sessions, and cascade-delete
their drafts and staged values without touching working connection credentials.

Root Vite+ checks cover authored JS/TS/TSX, provider pages, SDK and scripts; formatting
also covers HTML/CSS/config files. Generated contracts and build outputs are excluded
from formatting and linting and remain checked by generation and compilation.

Provider categories use the original MCP string identifiers (email, chat,
developer_tools, etc.). Capabilities are declared explicitly beside each connection
method. Each method keeps its id and display name and has a CredentialSource enum:
Static (one JSON Schema), OAuth (grant/configuration and optional additional static
credential schema), or Custom. Static and OAuth sources use the shared standard page;
only Custom sources select provider-owned UI and local/remote setup handlers. Schemas
are flat objects with strings, booleans, numbers, enums and basic validation; unsupported
keywords and remote references are rejected. The old field-description table is migrated
into JSON Schema and removed. OAuth input schemas are derived from the grant, and extra
fields cannot override protocol-owned credentials. Hosting remains independent of source:
remote standard providers need no custom UI or setup-handler credential.
Provider revisions and expected-revision parameters are removed. Registered definitions
update atomically, and builtin definitions reconcile at startup. In-use methods cannot
be deleted through registration. Remote authorization has its own opaque encryption
identity; the migration retains existing ciphertext bindings without versioned providers.

## Chat

- User: a human conversation identity, independent of a login or provider address.
- Thread: one conversation with a primary agent and human/agent participants. Native
  threads and external threads share the audit model; each external thread has one channel binding.
- Participant: one user or registered agent participating in a thread.
- Message: explicit visible content; streamed messages are streaming, complete or aborted.
- Activity: ordered per-thread lifecycle facts and reasoning, separate from the transcript.
- Goal: one agent's desired outcome in one thread.
- Task: one agent's work item in one thread, optionally linked to a goal and existing dependencies.
- Run: a durable objective that can wait across invocations.
- Invocation: one active reasoning loop for an agent in a thread; stop ends only this loop.
- Steering input: durable additional input delivered idempotently to an invocation.
- Agent runtime: an agent-hosted ConnectRPC service for Invoke, Steer, Cancel, Stop and Healthz.
- Agent session: capability-scoped ConnectRPC callbacks to Tilde for message streaming and work tools.

Provider tools are a dynamic catalog. Stop is always available in the SDK, and
ordinary reasoning/return values never automatically become messages. Child jobs, compaction and budgets remain outside the current runtime.

## Shared frontend theme

The web app and bundled connection/identity-verification iframes import the same Tailwind
foundation from `sdk/ts/packages/connection-ui/src/style.css`. Its light palette follows
`tilde-marketing/campaign-manager/src/app/{globals,enterprise-home}.css`: paper, espresso,
blue, butter, forest, purple haze and sienna. Semantic status colors map to those tokens.
Default body, main-card and iframe surfaces are white; deep beige is scoped to the sidebar
and the outer canvas behind the main content card.
Geist, Geist Mono and Bricolage Grotesque fonts ship locally with connection-ui, so embedded
forms and the main application share typography without third-party font requests.

The sidebar reuses the marketing result cards' deep beige GrainGradient, with a static
PaperTexture and frosted-glass overlay. Shader rendering is bounded, pauses offscreen or
when the page is hidden, and respects reduced motion. A CSS background remains without WebGL.

## Frontend routing

TanStack Router owns browser navigation through file routes in `web/src/routes`.
The Vite plugin generates `web/src/routeTree.gen.ts`; `_app` is the pathless
authenticated layout, and public brokering lives outside it. Agent routes live in
`_app/agent/`, with the shared editor and its tabs grouped in `$agentId/`.
Standalone Connections and Chat pages are not exposed. Agent rows open `/agent/{id}/capabilities`;
`/agent/{id}/chat-providers` and `/agent/{id}/iam` are sibling tab routes under a shared
agent editor. Agent details are fetched by ID after the management session check,
so direct URLs and refreshes do not depend on previously loading the registry. The
shared editor preserves local state across tab navigation and browser Back/Forward. The dashboard header
shows Agent Registry / agent-name breadcrumbs, updated from loaded and saved route data.
Pages can contribute a DashboardNavigation slot beside the breadcrumbs; agent tabs use a
React portal so their panel context and keyboard behavior remain intact. Sign out lives
in the sidebar footer and notifies the existing authentication boundary after revocation.
`/agent/new` creates an agent; the registry remains `/`. Connection brokering has a
separate public route outside the authenticated app layout. Connection setup and
identity verification host modules live under `web/src/routes/connections/` with
`-` prefixes to exclude them from route generation. Shared connection UI loading
keeps embedded content mounted but hidden until ready, with bouncing dots and
Motion fades that respect reduced motion preferences.

## Agent access and identity verification

The agent IAM tab uses management-only AgentAccessService RPCs. Access belongs to the
agent/channel assignment, with private, public, and disabled modes. Existing assignments
migrate as public; new assignments start private (GitHub starts disabled). Public still
records every sender identity; private accepts only verified identities explicitly allowed
for that agent; disabled accepts no provider traffic into agent conversations. Blocked
callbacks retain a deduplicated access-decision audit without exposing their content to
agent history. Mode/grant changes invalidate runtime access and cancel affected invocations.
Queued messages and claims recheck current policies under the connection lock.

An identity is a provider-owned string `value` with an `email`, `phone_number`, or `username`
type, unique within the connected account. Core IAM never normalizes identity values;
provider adapters construct inbound identities and validate verification recipients.
Verification is independent of an agent's allow flag. Management creates a pending request
and the provider privately delivers the secret approval link. Only its hash is stored.
The link expires after ten minutes; resends supersede prior proofs and are throttled.
Reading the page never approves. An explicit public Approve RPC consumes the delivered
proof and grants only the bound identity/account/agent. Management clients never receive
the token or approval URL. Removing the assignment invalidates its proofs and grants.

`/identity/verify/{id}` hosts the public, no-login approval flow and a sandboxed shared React
iframe; only the host holds the identity-verification token. AgentMail, Slack, Linq, Meta
WhatsApp and Telnyx deliver verification messages through their existing credentials.
WhatsApp can use an approved one-parameter template outside its customer-service window.
GitHub supports public/disabled only in this pass because it has no private message API.
Native/management invocations remain separate from provider-triggered runs; an outbound
channel binding must not silently revoke a native invocation's existing authority.

## Agent pause and deletion

Management PauseAgent persists `paused`, cancels active invocations and revokes
ordinary runtime access. Executions observe cancellation through their own outbound
control subscriptions. Pause reports the durable state change; it does not claim
synchronous host acknowledgement. Pending work waits for ResumeAgent. Each invocation
must establish its control subscription before running user code, so late requests
cannot bypass pause/revocation even on a different serverless instance.

`InvocationControlService` on the runtime listener streams scoped steering, stop and
suspend controls. It allows a narrowly scoped terminal-token window to acknowledge
stops without restoring ordinary RPC access. PostgreSQL LISTEN/NOTIFY at the gateway, or
in-memory change notifications on a sidecar, wake delivery; unacknowledged steering replays after reconnect. SDK
input IDs deduplicate delivery. Control subscriptions are renewed with current tokens
and the SDK aborts work after a prolonged loss of the control connection.

SuspendInvocation transitions the run through `suspending`; the SDK checkpoint hook
must quiesce and persist framework state before returning. Execution then exits into
`waiting`. ResumeRun starts a new invocation and carries unconsumed input forward.
Framework code restores its checkpoint using the stable conversation ID. The host
service now exposes only Invoke and Healthz. The inbound Steer, Cancel and Stop RPCs,
per-host stop fences, synchronous pause receipt and redundant sidecar push jobs are
removed. HTTP/1 serverless handlers and standalone hosts use the same SDK lifecycle.
Deletion requires pause, erases the signing key and grants, removes connection assignments,
and retires the registry entry and thread participation. Conversation and audit history
remain readable; retired IDs cannot be reused. Paused state is independent of health.

## Agent identity and avatars

Creation requires a nonempty HTTP(S) endpoint in Rust and both RPC contracts; updates
cannot clear it. Historical endpointless registrations can be read but need an endpoint
before they can be updated. Retired records may clear their endpoint.
Each agent has a stable random avatar seed, derived from its UUID at creation. The
MIT-licensed Dispatch avatar renderer lives in `sdk/ts/packages/agent-avatar` and animates
in the registry. Custom raster images are uploaded through management UploadAgentAvatar,
stored in private S3 objects, and exposed by short-lived signed read URLs. Replacing an
image preserves the generated seed and removes the old object after the database commit.
S3 is configured with ENGINE_S3_*; task dev starts a local MinIO bucket. Agent capability
targets use a stacked-avatar picker backed by management name search before pagination.

## Agent health and registry metrics

`agent_health` records each runtime Healthz RPC observation (endpoint, timestamp,
readiness, latency, bounded failure code). The SDK exposes a `healthz(signal)`
hook behind the Healthz RPC. A bounded Tokio worker checks every 30 seconds
with a five-second deadline and a database advisory lock to avoid overlapping sweeps.
Startup and hourly cleanup delete checks older than 14 days. The worker shares the
server shutdown signal; the old subprocess-oriented process manager is not needed.

Registry responses include 12 UTC hourly buckets (current partial hour last),
latest status (Unknown after 90 seconds or with no checks), participating thread
count, completed visible agent replies per thread, and invocation-to-first-visible-
reply latency. `chat_messages.first_content_at` captures the first non-empty
streamed chunk once; existing messages without that measurement are excluded from
response averages. No-data hours and missing response samples are not represented
as healthy observations or zero latency. The registry refreshes every 30 seconds.

Rust domain code is grouped under `agent/{mod,health,rpc}.rs` and
`chat/{mod,rpc,runtime}.rs`. Conversation contracts and storage use Thread,
`thread_id`, and `chat_threads`; registry metrics use `thread_count` and
`average_turns_per_thread`. This is a breaking rename without legacy aliases.

Chat provider tools use an invocation-scoped ConnectRPC catalog and streaming
InvokeTool transport. The SDK registers their server-authored descriptions and
schemas as local tool wrappers before the agent loop. Sending is provider-owned;
there is no SendMessage RPC or shared sending trait. Providers explicitly publish
canonical message events through scoped lifecycle helpers. Native text sending is one provider implementation. Connection-backed adapters expose
provider-specific schemas and handlers through the same catalog. Stop stays SDK-local,
and goal/task helpers call Tilde.

## Tracing

Tilde embeds the pinned Rotel fork in `vendor/rotel`. Agent OTLP/HTTP ingestion
shares the agent runtime listener and authenticates with agent connect tokens.
`iam::listeners::agent_runtime_router` owns the complete runtime surface, including
OTLP ingestion outside the strict live-RPC guard. The management router has no ingestion
route; management/OIDC sessions cannot authorize uploads. Platform request tracing on
management remains internal instrumentation, separate from accepting agent telemetry.
It accepts telemetry until five minutes after invocation end; expired JWTs only
qualify if they were valid at termination. Other actions and renewal still require
live invocation state and unexpired tokens. No separate telemetry credential exists.

Message and invocation rows retain W3C trace context across durable dispatch.
Langfuse is the trace system of record. Sidecars accept scoped OTLP, stamp it with the
verified invocation scope, and ship it to the gateway as telemetry frames in their
publish stream. The gateway re-stamps the agent from the authenticated deployment and
accepts batches into its bounded delivery queue. It forwards to Langfuse and clears
delivered payloads. Postgres holds only temporary delivery payloads and expiring replay
receipts, never permanent trace history. Delivery wakes through LISTEN/NOTIFY and
uses scheduled retry deadlines. Langfuse credentials remain at the gateway.

Management trace reads query Langfuse. Without Langfuse configuration, tracing is
disabled. Invocation tokens authorize agent uploads, including the terminal upload
grace period; management tokens cannot authorize ingestion. The SDK batches per
invocation and preserves the runtime callback path prefix in its OTLP URL.

`sdk/ts/langsmith-agent` is an isolated AI SDK 6 / LangSmith demo. It loads the
OpenAI key from SOPS, sends synthetic recipe/tool traces to LangSmith for UI
inspection, and can optionally host the same agent through Tilde ConnectRPC.

## IAM

- Principal: an authenticated IAM user or registered agent. An IAM user is linked
  by the OIDC issuer and subject; it is distinct from a ChatKit conversation user.
- Management API: the listener serving OIDC login/exchange, the browser UI and
  management RPCs. Every authenticated IAM user can perform every management
  operation. There are no roles, teams, API keys or local password accounts.
- Agent runtime API: a separate listener in the same process. It accepts only
  signed agent connect bearer tokens. It exposes agent registry operations,
  current-thread reads and invocation, and agent session callbacks. It has no
  OIDC, connection administration or browser routes.
- Agent connect token: signed installation-issued claims for one agent,
  invocation, run and thread, with a capability snapshot and a 15-minute expiry.
  Every agent action also verifies that the invocation is running with a live lease.
  Token renewal requires a still-valid token and intersects its grants with the
  current agent record. Renewal never widens authority. The SDK renews every
  five minutes. Existing tokens retain their snapshot until expiry; cancellation
  revokes agent actions for that invocation immediately. Trace ingestion alone
  retains the five-minute terminal upload window described above.
- Capability: a known action mapped to No/Yes for agent creation, thread reads,
  work reads/writes and own-run updates; other actions use None/All/Selected target
  IDs. RPCs expose a typed field per capability: BinaryPermission for binary actions
  and TargetPermission with a TargetSelection enum for targeted actions. Stored grants
  and signed claims retain the typed Rust action map (`no`/`yes`, `none`/`all`/`selected`).
  Missing grants deny; unknown enums and target IDs on non-selected modes are rejected.
  The edit UI saves toggles immediately and selected agents on modal confirmation,
  rolls back failed updates, and has no capability Save/Cancel footer. Agent actions are read/create/update/delete/invoke and
  grant_capabilities. Thread reads, work reads/writes, own-run updates and tool
  invocation are separate capabilities. Any thread/work grant still stays inside
  the invocation's thread/agent scope. Tool targets are catalog tool names.
- Granting authority: creation grants no capabilities by default. Setting grants
  requires explicit grant_capabilities authority over the target and cannot
  exceed the caller's invocation capabilities. Endpoint changes also require
  authority at least as broad as the target agent's current capabilities.

OIDC is configured through the environment. Browser login uses authorization code,
PKCE and nonce validation, plus a browser-held verifier for the local one-time
exchange. Management bearer sessions expire after eight hours and are revocable
on logout. Only session hashes are persisted. The frontend keeps the token in
local storage and sends Authorization headers; authentication does not use cookies.
The installation signing key and temporary upstream PKCE verifiers are encrypted
through the encryption module. Dev starts Compose Dex with a seeded local account.

`ENGINE_MANAGEMENT_ENABLED` and `ENGINE_WEB_ENABLED` independently control
management routes and React serving, defaulting to true. Management-off instances
require no OIDC configuration. Packaged React assets share the management bind;
that listener is absent when neither management nor embedded web is enabled.
Development omits Vite when web is disabled and Dex when management is disabled.
The agent runtime and event ingress listeners and background workers remain active
in every mode. Event ingress defaults to `127.0.0.1:8082` and mounts only signed
provider webhook routes; management never mounts webhook ingress.
`ENGINE_PUBLIC_EVENT_INGRESS_LISTEN` and `ENGINE_PUBLIC_EVENT_INGRESS_PUBLIC_URL` control its
bind and advertised webhook origin independently of management.

Taskfile owns development startup, build/test sequencing and SQLx preparation.
Task loads `.env`, validates Rust configuration before starting services, then runs
the API and optional Vite concurrently with live interleaved output. A service
failure stops its siblings (fail-fast); Ctrl+C also stops all processes. `pnpm dev`/`build`
delegate to Task. Disposable Postgres lifecycle remains in a shell wrapper;
JavaScript remains for substantive generation checks and integration tests.

`task dev ADDRESS=<IPv4>` binds browser-facing development services (management,
React/Vite, provider UI/HMR and local Dex) to that address and aligns their public
URLs, browser origins and OIDC redirects. Without ADDRESS they use loopback. Agent
runtime and Postgres settings are unchanged. Browser PKCE hashing and UUID creation
work on remote HTTP origins without requiring secure-context WebCrypto methods.

For the default local engine database, `task dev` starts persistent Compose
Postgres on loopback port 5432 and waits for health before launching the API.
POSTGRES_PASSWORD and DATABASE_URL must agree. Custom database endpoints are
left alone. Docker-backed development data survives application shutdown.

## Connection assignments and conversation audit

`connection_agents` is the capability relationship, not a copy of credentials. A partial
unique index gives the `channel` capability one agent; an agent can own several chat
connections. Future capabilities can have different cardinality. `StartConnection`
accepts assignments and persists them atomically with the setup. `AssignCapability` and
`UnassignCapability` attach existing connections. Removing the relationship preserves
credentials and past conversations. Connection get/list uses one SQL query with a typed
agent-summary aggregate, retaining pagination and optional agent/unassigned filters.
An assignment may be pending credential setup. Tools appear only while the connection
is ready and assigned; invocation IAM grants still filter the catalog.

Chat adapters live under `chat/providers` for Slack, GitHub, AgentMail, Linq, Meta
WhatsApp and Telnyx WhatsApp. They own dynamic tool definitions and provider wire
formats. Tool names include the connection ID to distinguish multiple accounts. Meta
read/typing actions are not advertised for Telnyx, and email has no reaction tool.
Connection credentials resolve through the existing encrypted store and refresh path.
Verified callbacks at `/connections/webhooks/{connection_id}` run only on event ingress
and check provider signatures, timestamp windows where available, and account identity.
Conversation/thread creation, participants, message and callback receipt commit together;
duplicate deliveries cannot create duplicate messages. Historical channel threads retain
their agent identity after reassignment. Native sending is omitted on external threads.
Outbound provider sends create/update the matching external conversation; their tool
execution remains audited in the initiating thread. Accepted upstream effects are recorded
even if cancellation races with the response. GitHub manifests enable their webhook;
Slack manifests declare subscriptions, with Slack's required URL-verification step.
Self-managed providers expose their webhook URL in connection reads and the setup UI.

Chat's ordered activity retains immutable typed Protobuf snapshots and timestamps.
Messages, reasoning deltas, joins/leaves, message chunks, typing changes and tool
inputs/results/errors remain distinct. Compact chunk events avoid copying a growing
message on every token. ListActivity paginates the same cursor used by WatchThread.
Participants leave through an active flag, preserving authorship. Typing expires after
ten seconds with a durable not-typing event. Provider calls audit automatically, while
tilde.runtime.v1.ChatService.ReportToolCall and the SDK helper cover agent-local framework tools.
Interrupted calls become aborted and are not automatically repeated across unknown
external-effect outcomes. The Chat UI replays every category and offers raw event view.

Attachments have authenticated upload/download RPCs, metadata in canonical messages,
and encrypted Postgres bytes (128 MiB per file). Provider download references remain
private and are encrypted when they contain URLs; their bytes are fetched on demand,
then encrypted and cached. Public file fetches pin DNS and reject private destinations;
provider bearer credentials are restricted to provider-owned file origins. Email HTML
is rendered inside a script-free sandbox. Attachment-only messages still dispatch to
an agent. Steering carries the original typed message snapshot, so attachments also reach
an invocation that is already running. Remote references require their source provider until first download.

CacheConvertedMessages and HydrateConvertedMessages retain the original batch cache
semantics. Values are explicitly opaque agent-converted JSON, separate from canonical
messages and audit snapshots, scoped by agent and message. Writes validate the entire
batch before committing, accept completed messages only, and cannot cross the invocation
thread. Incoming edits invalidate conversions. Runtime history includes that agent's cached conversions alongside canonical messages. The TypeScript SDK exposes cache, typing, attachment and local
tool-audit helpers alongside the generated clients over its shared HTTP/2 transport.

Development and test configuration use committed `.env` / `.env.test` defaults.
The original KMS-encrypted `secrets.enc.yaml` retains top-level dev credentials and
its `test` overrides; `scripts/load-secrets.py` extracts only chat credentials into
ignored `.env.secrets` / `.env.secrets.test`. Exported values and ignored local
mode-specific overrides take precedence. Test tasks use disposable Postgres.
All six chat adapters have isolated outbound/inbound fixture coverage. Live account
and two-way suites require the `live-chat-tests` Cargo feature and explicit ignored-test
execution. Two-way tests use the production ConnectRPC tool API, encrypted credential
resolution, real provider delivery and a real peer reply. Hookdeck raw captures retain
the original signatures when replayed into local ingress. Slack/GitHub/AgentMail peers
are automated; phone peers are assisted. Callback deduplication and tool input/output
audit are asserted; temporary remote resources are cleaned up on failure too.


## RPC audience boundaries

Protobuf contracts are grouped by audience, then domain, under `proto/tilde/`:
`management/v1`, `runtime/v1`, `agent_host/v1`, `setup/v1`, and `provider/v1`.
Shared resource types live in `types/v1`. Management never imports runtime cache types.
Canonical Message has no conversion-cache field. Runtime ListMessages and agent-host
Invoke carry cached conversions separately; the SDK preserves batch cache/hydrate helpers.

Management and runtime expose separate AgentService and ChatService definitions, each
mounted in full on its designated listener. No per-method audience allowlist or optional
agent-auth/mode flag selects behavior in shared handlers. The thin adapters live under
`agent/rpc/{management,runtime}.rs` and `chat/rpc/{management,runtime}.rs`, sharing domain
commands, canonical types and projection helpers. Runtime infers the current thread and
acting participant; explicit other-agent targets still require their existing capabilities.
All previously permitted runtime registry operations remain available.

Connection management and public setup commands are separate complete services. The
ConnectionSetupService and native OAuth callback are composed outside management login;
every setup command validates its connection setup token. Remote provider HandleSetup
belongs to provider/v1; agent-host Invoke and Healthz belong to agent_host/v1. Invocation controls are outbound runtime subscriptions.
This changes protocol paths and generated clients, with no legacy namespace aliases.
It retains the single binary, common domain implementations and central database migrations.

## PostgreSQL notifications

Thread subscriptions use a shared PostgreSQL LISTEN connection per Chat service,
with transaction-commit NOTIFY wakeups from `chat_activity`. Readers subscribe
before loading history and drain all cursor pages before waiting. Notifications
contain no event data. Reconnect invalidates subscribers so they catch up from
persisted state without periodic subscription polling. Listener connections are
separate from the query pool and close with the service/pool.

Pending message routing and invocation dispatch wake on committed message/work
changes and on freed local execution capacity. Steering wakes on pending-input
changes. Trace forwarding wakes on queue changes, drains batches, and schedules
retries from persisted retry deadlines. Startup/reconnect reads recover missed
notifications. Health probes, lease heartbeats, expiry/retention cleanup and
failed-operation retries remain time-driven. The registry browser currently
refreshes every 30 seconds; it does not yet have a registry subscription API.

## Deployments, instances and the run protocol

An agent has many deployments. `agent_deployments` is the declared half of "what is
running": created by CI (`RegisterDeployment`, idempotent on a caller-supplied external
id) or by hand in the Deployments tab, with a source, a target (`direct`, `sidecar` or
`aws_lambda`), the wake address or function ARN, repository and commit. Registering a
deployment issues its token once; the token is the join between the record and whatever
runs. `agent_instances` is the observed half: every process that dials in with a
deployment token, heartbeating on a short cadence. Every instance belongs to exactly one
deployment, the gateway rejects registration otherwise, and local agents never federate
into a deployed registry. Retiring a deployment invalidates its token. Policy
(capabilities, connections, IAM) stays on the agent; execution mode (gateway or sidecar)
also stays on the agent for now, so every deployment of an agent shares it and a
deployment's target must match. Creating an agent with an endpoint mints its initial
manual direct deployment, and changing the endpoint mints another; promotion mirrors a
direct deployment's endpoint back onto the agent for legacy readers.

Routing is per agent: `latest` serves each newly registered deployment the agent's
mode can run (direct and Lambda for gateway mode, sidecar for sidecar mode) at once,
`manual` waits for `PromoteDeployment`, and under `manual` the serving deployment
cannot be retired until another is promoted. A (thread, agent) pair is pinned to the
serving deployment on its first invocation (gateway mode) or first lease (sidecar
mode) and stays pinned while that deployment is registered; invocations record their
deployment. The pin is a constraint, not a preference: gateway-originated work goes
only to instances of the pinned deployment, failover stays within it, and a replica of
another deployment is refused the lease. Moving a thread across deployments is an
explicit act, not yet exposed.

Execution is wake-and-dial. A wake carries the invocation (`InvokeRequest`, now with
the deployment id and trace context); how it is delivered is the only per-target
difference: an instance that dialed in through `tilde.run.v1.RunService.Watch`
receives it as a frame (a durable directive acknowledged by `Report`), a direct
endpoint is woken over HTTP, and a Lambda function is invoked asynchronously through
the AWS invoke API with the gateway's own credentials. Every host then reports through
`RunService.Report` with its invocation capability: acceptance, reasoning deltas and
the end of the invocation with any unconsumed inputs and, when the host's run threw,
the error, which the gateway records as a failed invocation rather than a stop. The
gateway no longer depends on the wake call's response; a legacy host that answers on
the Invoke stream still works.
The invocation lease is renewed by the host's open command stream and by reports, so a
host that dies stops renewing and expiry reclaims the run. Health polling remains for
direct endpoints only; connected instances are healthy by heartbeat.

## Agent deployments and sidecars

Agents select gateway or sidecar deployment in the Deployment settings tab. Gateway
mode uses Postgres for everything and wakes direct or Lambda deployments. Sidecar mode
runs replicas that dial in with a sidecar deployment's token. `tilde-sidecar` accepts
comma-separated deployment tokens, one per agent, and keeps no state on disk. The agent
process beside it dials the sidecar's loopback runtime listener with the same token
through the run protocol (`connectAgent` in the SDK); an optional agent-ID-to-endpoint
map remains for agents that must be woken over HTTP instead.

The gateway serves four route groups on one listener: management, runtime, ingress and
sidecar. A sidecar binds a loopback runtime listener for its agent process and one
network listener for provider webhooks and native ingress. Every other interaction is an
outbound connection from the sidecar to the gateway's sidecar group: a held `Watch`
stream that delivers a snapshot, configuration changes, lease changes and directives,
and `Publish` calls that carry heartbeats, typed events, directive results, lease
releases and telemetry. Central IAM, registry changes and credential setup stay at the
gateway. Sidecars enforce replicated permissions and serve assigned connection
credentials from memory.

Postgres is the record. A replica is a cache of the threads it touched, a write-behind
queue, and the executor for the threads it holds a lease on. Execution exclusivity is
one lease per `(thread, agent)` in `thread_leases`, pointing at a replica incarnation;
data carries no ownership. A thread costs one gateway call when a replica first touches
it: `Hydrate` takes the lease, then returns everything the executor needs to resume
(roster, recent messages, runs and their origins, goals, tasks, cached conversions).
Lease frames carry the row's version so a replica can order a Watch frame against the
lease its own Hydrate returned. After that, events on that thread cost no SQL and no
gateway call on the hot path: the holder mutates state under one per-thread lock,
appends typed events with a per-thread sequence, and the shipper drains them
asynchronously in batches. A thread idle past the configured window leaves memory and
its lease is released, so the next replica to touch it takes over cleanly; a per-agent
cache byte budget evicts the least recently active idle threads early. A release is
projected in queue order with the events before it, so a run that completed before its
thread was evicted is recorded before the lease goes. A sidecar is a per-host daemon serving several
agents: each agent has its own runtime, cache, outbox and leases, sharing one instance
id, one public listener and one loopback listener. On a graceful stop each runtime
releases the leases of idle threads, ends its invocations on threads mid-invocation
locally (the agent process's open command stream turns into a stop, as it does when a
replica is fenced or loses a lease), and sends a final not-ready heartbeat; the gateway
treats a not-ready instance as gone and recovery restarts that work at once instead of
after the liveness window.

Lease validity is the holder's liveness: the instance heartbeat, sent every three
seconds independently of the event queue, implicitly renews every lease the replica
holds, and a holder unheard from for the liveness window (`LIVENESS`, fifteen seconds, passed
into every query that judges liveness) is dead. A replica whose publishes
have not been acknowledged for ten seconds stops executing on all its threads, strictly
inside the gateway's dead-detection window, so a partitioned holder never runs beside
its replacement. Only run, invocation and tool-call state is checked against the lease
at projection time; messages, users, thread changes, traces and logs from any replica
of an agent in the thread are accepted. Run state from a replica that no longer holds the thread is answered
with the current lease rather than an error, and the replica fails its local work for
that thread. The gateway projects a batch in one transaction with an idempotency
receipt per event; only a frame the gateway cannot accept (a permission failure or a
constraint violation) is rejected on its own, and only infrastructure failures fail a
batch, which is then resent in order. Acknowledged writes live in replica memory until
the shipper publishes them: a replica that dies with a non-empty outbox loses those
events, and the gateway decides the order of near-simultaneous appends from different
replicas, both accepted risks in exchange for a hot path without round trips. External
effects still need application idempotency keys.

Recovery scans leases whose holder is dead. A lease with no active invocation is simply
dropped. One with interrupted work locks the thread route, fails the old invocations,
re-routes the dead holder's unacknowledged directives, and either moves the lease to a
live replica, which hydrates the thread and restarts the run from its objective, or
fails the run under the stop policy. A hydrate that finds the current holder dead
performs the same takeover with the claimant as the replacement, so a forwarded request
never orphans an interrupted run. Lease changes reach replicas on the Watch stream.

Requests for a thread that land on a replica which does not hold it are handed to the
holder directly, sidecar to sidecar, when the gateway's lease answer named a live one;
when nobody holds it the receiving replica takes the lease and runs the turn itself.
Requests on the gateway's ingress for a sidecar agent are queued as durable directives
for the holder, or for any live replica when the thread is unheld, and the HTTP result
is relayed unchanged. Provider webhooks are handed over the same way without waiting.
Completed messages in rooms with several sidecar agents are relayed to each holder;
gateway-deployed agents in the same room are routed from the projection. Management
writes to sidecar conversations forward the same way. Reads (thread, run, message,
activity and thread listings) are answered from the projection wherever they land,
since only the projection sees every replica's conversations; a replica forwards
listings and cursor pages, and serves the current window of a thread it holds.
Registry RPCs from an agent beside a sidecar are verified by the replica, relayed over
`Relay`, and re-verified at the gateway: token signature, agent generation, the
thread lease, and the capability ceiling, before the existing registry handlers run.
The projection may lag a young invocation or roster, so only an invocation the gateway
already knows to be over is refused there.

Attachment bytes live in bounded sidecar memory until a separate worker uploads them
to the gateway, which encrypts them into S3; a slow object store never delays
heartbeats. Replicated metadata distinguishes temporary
availability from persistence, and persisted bytes can be downloaded through the
gateway by any replica. Health samples arrive with heartbeats.

## Langfuse observability integration

Gateway configuration owns LANGFUSE_BASE_URL, LANGFUSE_PUBLIC_KEY,
LANGFUSE_SECRET_KEY and optional LANGFUSE_PUBLIC_URL. Only tracing_enabled is
included in sidecar configuration. Disabled telemetry is validated and discarded;
gateway outages keep batches in the bounded sidecar outbox. Gateway acceptance places
them in the shared transient telemetry_delivery queue. Stable payload receipts
deduplicate reconnect replay; Langfuse owns all trace history. Platform spans use a process-wide
provider routed to the correct local agent, preserving invocation context and
terminal trace-token grace. The agent Tracing tab queries scoped Langfuse public
APIs through management-only RPCs. No Langfuse credentials reach agents or browsers.

## Agent log history

OTel logs use Rotel and the same verified invocation scope and terminal grace as
traces. ClickHouse owns log history in a standard OTel Map schema with explicit
agent, invocation, thread, and record identities. Gateway disk queues independently
buffer local history and optional external OTLP delivery; application Postgres
never stores bulk logs. Queue limits, 24-hour pending expiry, and seven-day history
retention bound resource use. Sidecars ship immutable log batches in their publish
stream until durable gateway acceptance and receive enablement only.

The management-only LogsService enforces agent scoping and fixed-window cursor
pagination. The agent Logs tab provides filters, record inspection, native trace
links, and a bounded live view. Local ClickHouse is enabled by default for task dev,
uses Compose defaults and .env overrides, and can be opted out independently of
Langfuse with DEV_LOGS_ENABLED=0.
