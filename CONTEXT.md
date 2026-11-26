
## Release version

The Rust workspace is released at one Changie version: `VERSION` and the Cargo workspace/lock
entries must agree. Changie records release intent and updates manifests; it does not infer
dependency impact. Release PRs batch that intent. Tags identify the checked commit; a partially
published release is retried at that commit.

Open-source Tilde's TypeScript and Python SDK packages share its release version: `VERSION` and
`sdk/ts/packages/*` versions must agree. Internal SDK dependencies use unversioned pnpm workspace
protocols; pnpm resolves published ranges, while a shared release bumps every SDK package.
Private frontend and example packages are not independently released. A manual release of the
merged default-branch commit checks and builds it before tagging, publishes the SDK to npm and
PyPI and the binary image to GHCR, and exposes GitHub release artifacts only when publishing
succeeds.

## Connections

For local provider callbacks, `task dev` optionally runs an ngrok CLI tunnel
(`NGROK_ENABLED`, `NGROK_DOMAIN`, `NGROK_AUTHTOKEN`) to the event ingress API.
Its HTTPS domain overrides `ENGINE_PUBLIC_EVENT_INGRESS_PUBLIC_URL` while enabled, including
explicit local URL settings, for all connection
webhooks and provider manifests. Management owns setup links and OAuth callbacks
on its own origin; ngrok is started only by the development launcher.
Task owns tunnel shutdown.

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

Connection usage: a connection in use is never disconnected or deleted. Its uses
(`queries/connections/usages.sql`, `Connections::usages`; `Connection.usages` on
GetConnection and ListConnections) are agent assignments, the agents a personal connection allows
by name, agents' and sandbox blueprints' tool sources, routines on its signals, sandbox blueprints
and their egress grants, sandboxes left on it by a blueprint that no longer names it, the
memory embeddings and memory banks' synthesis.
`Disconnect` (and deleting a tool host, whose instances go with it) refuses with
`failed_precondition` naming every use; the person removes each first.
Disconnect holds the connection's row `FOR UPDATE` while it looks, and the `connection_usable`
trigger on every using table refuses a disconnected connection after waiting on that row, so a
racing bind and disconnect never both commit. Foreign keys from users to `connections` restrict
deletes; deleting an agent drops its tool sources, routines and personal allowances (its row
stays), and with its routines their run history.
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
A host's `Theme` (`@trytilde/sdk/theme`: the spaces preset's tokens plus a scheme) styles every
embedded UI. Chat derives its palette from it on its `.tilde-chat` root. For setup it travels two
hops: the embedding page answers the broker's `tilde.connection.ready` with
`tilde.connection.theme`, and the broker hands it to the provider frame with `tilde.setup.port`
and, on change, over that port; each page applies it to its shadcn variables. Without one the
provider frame follows the broker's scheme.
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
Setup prefill (`prefill.ts` in connection-ui, the same contract as Tilde Cloud's console): a
setup URL may carry `type`, `name` and `field.<key>` values, all still editable. The brokering
page forwards `name` and `field.*` to the provider iframe; `useSetup` uses `name` only over the
provider's placeholder name and `CredentialForm` fills schema fields under saved drafts. Secrets
never come from a URL: secret-looking keys are dropped and write-only fields are skipped.
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


Capabilities are `channel`, `inference`, `tool`, `signal` and `egress` (granted by sandbox blueprints). A connection type declares which it offers;
an assignment grants one capability of one connection to one agent. Chat keeps a single
owner per connection; an inference connection may be assigned to any number of agents.
Every connection has a slug, `provider_id/name` (for example `linq/daniel@trytilde.ai`
or `anthropic/prod`), where the name is the account name collected during setup. Names
are limited to URL-path-safe characters and are unique per provider among ready
assignments within each agent and capability, so agents route by slug. Different agents
may use the same provider/account name for separate connections. Management resolves
chat provider IDs with an explicit agent ID; runtime resolves them from the agent token.
Inference providers label the name "API key name", "Project name" or "Resource name"; the account label stays the provider's own
account identity. An inference assignment may carry an alias, unique per agent and never a
provider id; the agent can call `ctx.inference("fast")` instead of the slug, and the route
resolves the alias from the same in-memory map (gateway loader or sidecar configuration).

Mints (`connections::mints`) turn a connection's stored credential into what a consumer (`tool`,
`chat`, later `egress`) sends upstream. A connection type mints `Stored` (the credential as it is:
an OAuth access token the connection keeps refreshed, else a `token` or `api_key`; the scope cannot
narrow it; sent in the headers its provider takes: a bearer token by default, a Linear API key
bare, PagerDuty's `Token token=`, Jira's Basic `email:token`, Datadog's `DD-API-KEY` and
`DD-APPLICATION-KEY`, New Relic's `API-Key`, Honeycomb's `X-Honeycomb-Team`, nothing for a
Prometheus without a token) or `Custom` (provider code). Custom mints: a GitHub App installation token limited to a
`MintScope` of `owner/repo` resources of the installation's account and App permissions (an
unscoped GitHub consumer gets the installation token the connection keeps), AWS STS sessions in
the scope's account, and Entire service account tokens (the connection's secret exchanged at its
region's auth host for a bearer token that lives at most ten minutes; no scope narrows it). Network mints are cached in memory by connection, a fingerprint of its stored
values and the scope digest until five minutes before expiry, single-flight per key, emptied with
the credential cache on every `tilde_sidecar_configuration` notification, and traced as
`credential.mint` spans. Which strategy a type uses is built-in catalog code (`mints::strategy`).

The Connections page lists every connection, split by what it is for into top tabs: Tools &
chat (tool or channel capability), Inference & embedding, Sandboxes (E2B and Modal), and Other
when any remain (ingestion sources, say), classified from each connection's provider and
capabilities. Tools & chat keeps the All/Catalog/MCP/Tilde tool server source tabs. Inference
and sandbox connections are added from their tab, with no agent first (`NewConnectionDialog`:
choose a provider, then its methods of that capability); tools come from the Tilde Catalog or
a remote server. Each row lists the agents assigned the connection or using it as a tool
source.


### OIDC issuer

The registry has one OpenID Connect issuer and one signing key set (`crate::oidc`), independent
of any provider; components mint tokens from it for a relying party by subject and audience. Its
consumer is workload federation from connections (subject `connection:<id>`).
The issuer URL is `ENGINE_TOKEN_ISSUER_URL`, by default the event ingress origin plus `/oidc`;
the ingress serves `/oidc/.well-known/openid-configuration` and `/oidc/jwks`, so a custom issuer
must route to those paths over HTTPS. Tokens are RS256 JWTs valid for five minutes with `iss` the
issuer, `aud` the relying party (`sts.amazonaws.com` for AWS) and `sub` chosen by the consumer
(`connection:<connection id>` for connections). Signing keys are 2048-bit RSA (aws-lc-rs), generated on first use, sealed by the encryption
crate (`oidc_signing_keys`, binding `oidc_signing_key`/kid/`private_key`); the newest signs, a key
older than 30 days is replaced at the next signing (`Issuer::rotate` forces it), and superseded
keys stay in the JWKS until they are 37 days old.

For an `aws`/`oidc` connection the operator, once per account: adds an IAM OpenID Connect
provider with the issuer URL and audience `sts.amazonaws.com` (AWS trusts the issuer's public CA;
the console's Get thumbprint fills a thumbprint if asked), then gives the connection's role this
trust policy (the setup page shows it with the real issuer and connection ID):

```json
{"Version":"2012-10-17","Statement":[{"Effect":"Allow",
 "Principal":{"Federated":"arn:aws:iam::<ACCOUNT_ID>:oidc-provider/<issuer host and path>"},
 "Action":"sts:AssumeRoleWithWebIdentity",
 "Condition":{"StringEquals":{"<issuer host and path>:aud":"sts.amazonaws.com",
  "<issuer host and path>:sub":"connection:<connection id>"}}}]}
```

Roles in other accounts (`assume_role_arns`) trust the connection's role (or IAM user) as an AWS
principal for `sts:AssumeRole`, with `sts:ExternalId` when `external_id` is set.

## Tools

- Tool capability: a connection type's declaration that its provider ships tools. Unlike
  `channel` and `inference` it is never assigned through `connection_agents`.
- Managed tool provider: Tilde-shipped tools backed by a connection's credentials
  (`tools/providers/<id>`). It owns definitions, schemas and upstream calls; names are
  provider-local. Most are tables of one-call REST tools (`tools/providers/rest`); polling
  tools (Tavily research, Firecrawl crawl) wait for the upstream job inside the call. Shipped:
  Tavily, Firecrawl, PostHog, Stripe (refunds), Payload CMS; the Google Workspace products
  (Mail, Calendar, Drive, Docs, Sheets, Search Console, Analytics), each its own provider with
  one `oauth` type; Sentry (auth token or OAuth), whose tools are assembled once from a
  generated catalog (hand-written issue tools, official Sentry MCP tools mapped onto REST, DSN
  ingestion and one tool per remaining OpenAPI operation) and whose `regionUrl` inputs must be
  HTTPS sentry.io origins so the token never leaves Sentry; GitHub (App installation, personal
  access token or OAuth app) and Slack (the Slack app's bot token), whose chat app types
  declare both `channel` and `tool`, so one connection serves chat and tools, as do Linq,
  AgentMail and WhatsApp (Meta and Telnyx); E2B sandboxes, whose `apply_patch` format is
  parsed by a shared module (`sandbox_patch`) Modal also uses; Modal, which manages sandboxes
  over its gRPC API and works inside one through its task command router; AWS, which offers a
  fixed snapshot of the official AWS MCP Server's tools and calls it over MCP with every
  request SigV4-signed and the connection's default region in `_meta.AWS_REGION`. An AWS connection
  is IAM access keys (`iam`) or a role federated with Tilde's OIDC issuer (`oidc`, no keys:
  `AssumeRoleWithWebIdentity` into `role_arn`). Either may list roles in other accounts
  (`assume_role_arns`, optional `external_id`): every AWS tool takes an optional `account_id`, and
  a call naming a listed role's account runs with `sts:AssumeRole` credentials (session
  `tilde-<connection id>`), cached until five minutes before they expire; an unlisted account is
  refused, and `aws_list_accounts` names the reachable ones. Several AWS connections (one per
  account) are simply several tool sources (`aws_prod.*`, `aws_staging.*`); Linear
  (personal API key or OAuth app) over its GraphQL API: `get_issue`, `search_issues`,
  `create_comment` and `update_issue_state` (by state name or ID within the issue's team).
  The software factory's providers are tables of REST tools too (`rest::Tool`: a call, its path
  placeholders and its fields; fields beside the listed ones pass through), mostly read-only:
  GitLab (`token`: a personal, group or project access token, with `base_url` for a
  self-managed instance; `oauth`: an OAuth app on gitlab.com) for projects, issues, merge
  requests, pipelines and job logs, a project named by ID or path; Jira Cloud (`api_token`:
  site URL, email and API token as Basic; `oauth`: 3LO, whose calls go to
  `api.atlassian.com/ex/jira/{cloud id}` with the cloud ID read from accessible-resources) over
  REST v2, whose text fields are plain text; Entire (`service_account`: the secret and the
  cluster host) for projects, repositories (list, get, create) on its Core and branches, commits
  and files on the region API of the repository's processing placement; Datadog (`api`: site,
  API key, application key) for monitors, metric queries and event and log search; Grafana
  (`api`: URL and service account token) for dashboards, alert rules and their state and data
  source queries; Prometheus (`api`: URL and optional bearer token) for instant and range
  queries, alerts, rules, targets and series; PagerDuty (`api_token` or `oauth`, US or EU) for
  incidents, their alerts and notes, services and on-calls; New Relic (`api`: account ID, user
  API key, region) over NerdGraph for NRQL, alert incidents (`NrAiIncident`) and policies and
  entity search; Honeycomb (`api`: API key, region) for datasets, columns, triggers, SLOs and
  `run_query`, which creates the query and polls its result; Terraform (`api`: organization,
  team or user token, optional Terraform Enterprise hostname) for workspaces, runs, plans (JSON
  output read through its redirect without the token) and state version metadata. A URL a
  connection configures must be https. Atlassian, GitLab, Datadog, New Relic and Honeycomb also
  have their hosted MCP servers as `mcp_oauth` types of the same providers.
- Tool host: a customer-hosted tool backend. Like an agent host it is never dialed. A
  connected host opens `tilde.tool_host.v1.ToolHostService.Watch` with its token, publishes its
  tools and answers calls through `Respond`; a Lambda host is its function ARN, invoked
  synchronously with the gateway's AWS credentials and asked for its tools on registration or
  refresh. Calls to a connected host cross gateway processes through the transient
  `tool_host_calls` queue with commit-time notifications; `chat_tool_calls` stays the record.
  A connected host whose stream has been quiet for thirty seconds stops being offered.
  The UI calls tool hosts remote servers. The agent health sweep also samples every host's
  availability (`tool_host_health`), which the Remote servers page shows as twelve hourly
  buckets beside the host's tool count and its provider's auth method names.
- Tool host provider and instances: a host whose tools need credentials publishes a provider
  definition beside its tools (static schema or OAuth with the person's own client over HTTPS;
  no custom setup UI, no managed or dynamic client). Its token endpoint is called like a
  discovered one: public addresses only, DNS pinned. The provider belongs
  to the host (`connection_providers.tool_host_id`): only it can redefine it, it is the one
  registered provider allowed the `tool` capability, and deleting the host deletes the provider
  and its connections. Each connection of that provider is an instance of the host, set up on the
  standard setup page like any connection. Before an instance is ready the host verifies its
  credentials (`VerifyRequest`); a refusal returns the setup to its form with the host's
  message, and the host may name the account (`account_label`). Every call on an instance
  carries its connection ID and credentials, resolved (and OAuth-refreshed) by Tilde per call;
  in the queue they are sealed to the row, and the call names the method the instance was set
  up with (`connection_type`). A host with a provider is used only through its instances,
  which agents and personal connections use as they do managed providers.
- MCP server: a connection type may serve its tools from the MCP server at a fixed URL
  (`ConnectionType.mcp`). Its `McpCredential` says how the connection's secret reaches the
  server: none, bearer (OAuth access token or `api_key`), `api_key` in a named header with a
  prefix, or `api_key` as a query parameter. The curated servers carried over from trytilde/api
  (Notion, Linear, HubSpot, Vercel, Neon, Salesforce, documentation servers and others) are
  built-in providers in `tools/mcp_catalog.rs`, one connection type per way to reach the server;
  a server of a service Tilde already integrates (Slack, AgentMail, Stripe, Linear) joins that
  provider as `mcp_*` types. Any other server is added by URL as a registered (configured) provider of its
  own: the MCP server is the adapter that lets a registered type declare the tool capability, so
  it is registered like any provider. It is listed on the Remote servers page beside
  tool hosts, not in the catalog, with the same 12-hour health history: every five minutes one
  replica sends each such server an unauthenticated `initialize`, and an answer below 500 (a 401
  is a live server asking for credentials) is healthy, a timeout, refusal or 5xx a failed check
  (`mcp_server_health`, kept 14 days like tool host samples). Before a given connection lists its tools, a server's panel
  shows what the installation (non-personal) connections discovered (a
  personal connection's discovery can name its owner's account, so it stays theirs), else what the server lists without
  credentials (probed, cached ten minutes, failures included), else, for a curated server only,
  the snapshot shipped in `mcp_catalog_tools.json`, which can lag behind the server. Agents only
  ever get a connection's discovered list. There is no generic "MCP server" provider
  whose accounts each name a server. Tools are discovered over streamable HTTP (JSON or event-stream responses) into
  `connection_tools`; a hash of the described tools makes an unchanged rediscovery a no-op, and
  a tool the server drops leaves the catalog. Discovery happens on first listing and on
  `RefreshConnectionTools`; there is no background rediscovery or session cache yet. Only
  public addresses are dialed, with DNS pinned per request. A tool-level `isError` is the
  tool's failure with the server's text; `structuredContent` is preferred as the output.
- Tool list search and filters are the server's, in SQL before the keyset page; the web only
  re-queries (search debounced). `ListProviders` takes `search` (name, id or instructions),
  `capability`, `category` and `source` (CATALOG: neither an MCP server added by URL nor published
  by a tool host; MCP_SERVER: the Remote servers page's MCP servers) and returns `categories`, the
  category menu for that capability and source. `ListConnections` takes `search` (name, account
  label or provider name), `provider_id` and `status`; `ListToolHosts` takes `search` (name) and
  `with_provider`. `ListProviderTools` takes `search` over name and description, applied in the
  handler because tools come from code, hosts, snapshots or live MCP servers, and `tool_host_id`
  for a host's own tools. Pickers leave out the few sources an agent already has on the client.
- OAuth client: an OAuth type's client comes from the setup form (the person's own app) or
  from dynamic registration (`dynamic`, MCP-served types only): setup discovers the server's
  authorization server (RFC 9728 metadata, RFC 8414/OpenID discovery), registers a public client
  (RFC 7591) and runs the code flow with PKCE and the server as `resource` (RFC 8707). The
  discovered endpoints, scope and resource stay with the connection for refresh. Every URL
  discovery follows, and the discovered token endpoint at each exchange, is dialed over HTTPS
  to public addresses only, resolved once and pinned (`Http::discovered`).
- Tool source: an agent's use of one installation-owned tool-capable connection, or of one tool
  host that needs no credentials. Which tools an agent gets is the agent's configuration:
  connections and hosts hold credentials and describe their tools, and two agents may use one
  connection with different tools. A source has a slug, derived from the connection's provider
  and name (or the host's name) when it is added, unique within the agent and then fixed.
  Adding the source is the grant.
- Agent tool: a tool the agent uses from one of its sources, by the source's own tool name, with
  the agent's settings for it: whether it runs in the background (async), an optional summary and
  description replacing the tool's own in the agent's catalog, and its display. Tools are not
  renamed. Summaries are at most 256 and descriptions 4096 characters (not bytes) wherever a
  tool is defined, overridden or audited (`tools::SUMMARY_CHARS`/`DESCRIPTION_CHARS`).
- Display: how an agent tool's calls show in end-user chats (the Tilde chat provider's activity
  and watch): full (input and output), summary only, or hidden. Fixed on the tool-call record
  when the call starts, like the summary; traces and management views keep the full call.
- Catalog name: `{source slug}.{tool name}`, the tool name made catalog-safe. The agent's
  `tools_invoke` capability still filters these names. Slugs that would prefix built-in names
  (`tools`, `agents`, `thread`, `user`, ...) are suffixed.
- Tool mode: one setting per agent, direct (the default) or dynamic, covering all its tools;
  never mixed. A dynamic agent's tools are deferred: absent from the listed
  catalog, found with `tools.search` (lexical ranking over name, summary and description; no
  model or embedding store), described by `tools.schemas` and called through `tools.execute`.
  The registry rewrites `tools.execute` into the inner call, so capability filtering, schema
  validation, summary, audit record, span and async behavior are those of a listed tool. The
  256-tool ceiling applies to listed tools only.
- Bundled tool: a tool bundled with the agent in its source code and run in its process. The
  developer writes it the framework's own way (Vercel `tool()`, LangChain `tool`/`@tool`,
  Mastra `createTool`, OpenAI Agents `tool()`/`@function_tool`, Pydantic AI `Tool`, Agno
  functions/toolkits, CrewAI `BaseTool`) and passes it to the adapter's
  `withTildeTools(ctx, tools, options?)` / `with_tilde_tools(ctx, tools, options=)`, which returns
  the framework's own tool collection: the channel's tools, the agent's other Tilde tools and the
  native tools wrapped for auditing. Name, description and schemas are read from the native tool;
  summary and display from its metadata (`metadata.tilde` where the framework has it) or
  `options`. Each adapter documents its framework's caveats in code. The SDK registers exactly
  that set (description, summary, input and output schemas, annotations, display) with
  `RegisterBundledTools` at the start of every invocation, possibly empty, and on each change;
  rows live per invocation (`invocation_bundled_tools`), so `tools.search` and `tools.schemas`
  describe them beside server tools (provider `bundled`). Tilde never executes them: the SDK
  routes a `tools.execute` naming one back to the process and audits each call through
  `ReportToolCall` with the tool's summary. The SDK's lifecycle helpers (`stop`, `goals.*`,
  `tasks.*`) are not bundled tools. `ToolService.ListBundledTools` returns the set of the agent's
  latest invocation that registered any (with its ID, start time and deployment), shown
  read-only as the "Bundled" group of the agent's Tools tab: they are configured in code.
  Sidecar-hosted agents keep no registrations.
- Declared bundled tool: a bundled tool `tilde deploy` found in the agent's code, stored with the
  deployment (`DeploymentDeclarations.tools`, `deployment_tools`) and fixed like its prompts and
  skills; `GetDeploymentContents` returns them. Core `defineTools(tools, options)` /
  `define_tools(tools, options=)` marks a native tool collection (also passed per invocation as
  `bundled`); core never converts native tools: each framework adapter's discover describes the
  tools of a `defineTools` value of its framework and the static tools of framework agents it
  recognises, with the same function its `withTildeTools` / `with_tilde_tools` publishes them
  with. Tools declared twice under one name with different definitions fail the deploy. The
  engine validates them as `RegisterBundledTools` does (`tools::bundled_valid`). Invocations
  still register what they run; a deployment's declared set is what its code shipped.
- Async (background) tool: returns a ticket (the call ID) at once while the provider works in the
  background. The durable tool call stays `running` past the invocation's end (`detached`), and
  the outcome wakes the agent: queued as input on the agent's live invocation in the thread,
  which reads it or, once it ends, starts a fresh invocation from it; with none live, as a new
  run in the same thread; keyed by the ticket either way. It is queued, never steered: steering
  applies the concurrency policy, and `interrupt` would cancel the work that made the call.
  `tools.result` reads it. A detached call abandoned by a crashed process is aborted after an
  hour.
- Pending call: any server-executed tool call of a caller that can suspend its loop
  (`InvokeToolRequest.suspendable`, set only on calls the SDKs' framework adapters' tool wrappers
  make, never on the agent code's own) that is still running
  after the threshold (`TILDE_TOOL_PENDING_AFTER_MS`, a minute), or at its invocation's deadline,
  answers `pending` (`InvokeToolResponse.pending`) and goes on in the background: its record is
  marked detached so the invocation's end leaves it running, and its outcome is that record. Its
  process heartbeats it (`heartbeat_at`, every 20 s); one whose process is gone (a restart) is
  aborted a minute after its last heartbeat, and its run resumes with that. The
  adapter then suspends the run with the call pending (see Suspended run). Streamed-input calls,
  background tools and sidecar invocations never answer pending; a caller that cannot suspend
  waits as before.
- Summary: a short label for a call, fixed on the tool-call record when the call starts
  (the agent's override, else the tool's own). Agent-local reports may supply their own.
- Annotations and output schema: optional hints on a tool definition (read-only, destructive,
  idempotent, open-world); hints for frameworks, never authorization.
- Built-in tools: offered only when the agent's existing capabilities make them usable.
  `agents.list` (agents.read; each agent's ID, key and name), `agents.message` and `agents.wait`
  (agents.invoke: the target joins the thread if needed and runs with the message as its
  objective), `thread.participants` and `thread.search` (thread.read), `tools.result` (when the
  agent has async tools), `thread.wait_for_reply` (thread.read: answers pending at once, and its
  output is the next message routed to the agent in the thread, which then starts no run of its
  own, written in the transaction that claims the message (a reply that was queued on the
  invocation before it called the tool answers it when the invocation suspends); a wait is answered only while its
  invocation runs or a checkpoint names it, and one left behind otherwise is aborted; a caller
  that cannot suspend is refused), the skill tools (`skills::tools`: `skills.write` with skills.edit,
  `skills.assign` with agents.edit_skills and skills.read or skills.edit, `skills.unassign` with
  agents.edit_skills; see Skills) and the wiki link tools (`wiki.link`, `wiki.unlink`; see
  Wikis). A tool acting on another agent takes its `agent_id` and checks the capability on it,
  as the runtime RPC does.
- Personal tool federation: a person connects their own account through a setup link the agent
  obtains with `user.connect_tool`; the connection is owned by that chat user
  (`connections.owner_user_id`) and can never be an agent's tool source. An agent holding
  `tools.personal` for the provider sees its tools as `user.{provider}.{account}.{tool}` only in
  invocations acting for that user, or for another identity under the same root. The user of an
  invocation is the run's source identity, else the thread's only active human; an unverified,
  unattested channel identity is a claim and unlocks nothing.
  An application's user does the same without a conversation: the native provider's
  `ListConnectableProviders`, `StartConnection`, `ListConnections`, `ReconnectConnection` and
  `DeleteConnection` (agent ingress, the agent's chat key or an embed key naming it, and
  `x-tilde-identity`) start the same hosted setup for a provider the agent's `tools.personal`
  grants, owned by that identity's chat user, and list, reconnect or disconnect only that user's
  connections. The setup page announces completion to its opener or parent window
  (`tilde.connection.complete` with the connection id), which `@trytilde/embed`'s
  `mountConnectionSetup` listens for.
- Delegation: the owner chooses which agents may use a personal connection: every agent whose
  `tools.personal` covers the provider (`connections.all_permitted_agents`, the default for new
  connections and for every connection made before owners could choose) or only the agents
  listed in `personal_connection_agents`. An agent uses it only when both allow; the catalog and
  every call check both, so a change applies from the agent's next invocation (a call is refused
  at once). Each personal tool call records the connection it ran on
  (`chat_tool_calls.personal_connection_id`), whose owner is fixed at setup.

The invocation catalog lists the agent's tools on sources whose connection is ready or whose
host can take a call; invoke rechecks the same catalog, validates input against
the tool schema, resolves credentials and calls the provider or host. The catalog is built once
per invocation (first ListTools or InvokeToolStream) and held in memory until the invocation ends, at
most 15 minutes: tools added or removed, a mode change or a source going unready take effect from
the agent's next invocation. Credentials, provider state and a connected host's liveness are
still checked on every call, so an offline host is refused at once. Every server-executed tool
runs inside an `execute_tool {name}` span (GenAI conventions) that the trace store records as a tool
observation with bounded input and output. `tools.search`, `tools.schemas` and `tools.execute`
get a span of their own; the tool `tools.execute` runs is a child span beneath it. A background
tool's span (`tilde.tool.background`) ends with its ticket, and a `background {name}` child span
covers the provider's work until the outcome is delivered. A tool's output is at most 1 MiB
(`tools::MAX_OUTPUT`, shared by REST providers, tool host answers and the audit record); a
larger one fails the call with a terminal "output larger than 1 MiB" record, and a background
call delivers that failure.

A sidecar-hosted agent gets its tool sources too, offered as `{source slug}.{tool name}` exactly
as on the gateway and filtered by the same `tools_invoke` ceiling; the replica's tool registry
audits every call, because the replica holding the thread owns its tool-call records and activity
order. Where a call runs depends on the source (`tools::Callee::on_sidecar`):
- Managed provider tools on a connection (GitHub, Tavily, ...) and tools of MCP-served
  connections run on the sidecar. The replicated configuration carries them as `SidecarTool`s
  (the catalog definition with the agent's wording and tool mode, the provider's tool name and,
  for MCP, the server URL and credential placement), built by `Tools::sidecar_tools` from the
  same rows as the gateway's catalog, and carries their connections' decrypted credentials as
  `ConnectionConfiguration`s with the TOOL capability (a connection already replicated as a
  channel or for inference is not repeated). Changes to tool sources, picks, MCP discoveries,
  tool mode, connections and credentials notify `tilde_sidecar_configuration`, so a rotation or
  removed tool reaches the replica with the next configuration frame, not the next invocation.
- Tool hosts (credential-less hosts and instances of a host's provider; hosts dial the gateway),
  sandbox tools and AWS OIDC (it signs with the gateway's OIDC issuer) stay on the gateway.
  The replica lists and calls them through the relayed
  `tilde.runtime.v1.GatewayToolService` (`tools::relay`), as it does memory and wikis; the
  gateway rechecks the token, lease, ceiling and its own catalog, and runs the call without a
  second audit record. A gateway the replica cannot reach hides those tools from that listing.

Built-in (`tools.*` gateway extras) and personal tools remain gateway-only.

Both SDKs expose `ctx.toolSource(slug)` / `ctx.tool_source(slug)`, tool `summary`/`annotations`/
`outputSchema`/`background` and bundled tools; TypeScript also has `ManagementClient.tools`/`toolHosts`, and `@trytilde/sdk/tool-host` with
`createToolHost` (dial-in) and `createToolLambdaHandler`. Tools declare zod `inputSchema` and
`outputSchema`, which type `run(input, ctx)`; `defineAuth` declares the provider, its methods (zod
credential schemas or OAuth) and `verify`, and its `auth.tool` gives `run` the instance's typed
`ctx.auth`. The Python SDK's `tilde.tool_hosts` mirrors this with pydantic models (`Auth`,
`Method`, `@auth.tool`/`@tool`, `create_tool_host`, `create_tool_lambda_handler`). Each SDK ships
an example tool server that `tilde dev-agent` registers and starts beside the example agents.
Once both servers are up, `dev-agent` gives every example agent the Python server's tools and a
demo "Acme" workspace of the TypeScript CRM (Example Agent 1 in dynamic mode), leaving sources an
agent already has alone. Each example writes two bundled tools (`local_time`, `roll_dice`) as
native framework tools in a module-level `defineTools` / `define_tools` value, which `tilde deploy`
declares with the deployment, and gives its model `withTildeTools` / `with_tilde_tools`: the channel's
tools, every other Tilde tool of the agent (`ctx.agentTools` / `ctx.agent_tools`, which leave
bundled tools out) and its bundled tools.

## Routines

- Signal: a typed event a connection's provider emits from an authenticated webhook delivery,
  named `{provider}.{object}.{action}` (`github.issue.opened`), with a summary and data. The
  `signal` capability belongs to the connection type; like `tool` it is never assigned through
  `connection_agents`, and personal connections are not signal sources. `signals::Source`
  (`crates/tilde/src/signals/`, ported from trytilde/api's signal providers) lists a provider's
  signal types (each with a default thread title template) and template variables and
  normalizes deliveries. A built-in type declares the `signal` capability exactly when it has a
  source; registration refuses a mismatch. GitHub App (issues, pull requests, comments, reviews, CI check runs), Slack,
  AgentMail, Linq, WhatsApp (Meta and Telnyx) connections reuse their chat webhook and its
  verification; Sentry auth-token connections (signed with an internal integration's client
  secret), Firecrawl connections (signed with the account's webhook secret), Linear connections
  (`Linear-Signature`, an HMAC of the body with the webhook's signing secret, and a
  `webhookTimestamp` at most a minute old) and AWS connections (CloudWatch alarms through an SNS
  HTTPS subscription) have no chat adapter and are verified by `signals::verify`. Their secrets
  (for AWS, the accepted topic ARNs) are optional setup fields; without one, deliveries are
  refused. SNS messages are checked against the certificate at `SigningCertURL`, fetched only
  from `https://sns.<topic region>.amazonaws.com`, with the RSA signature over SNS's canonical
  string (SHA-1 for SignatureVersion 1, SHA-256 for 2); a valid signature proves SNS, not the
  customer, so only listed topics are accepted, and their subscription confirmation is answered
  by visiting its SubscribeURL on the same origin. The exact signal names are listed under
  "Signal catalog" below. Signal-capable setups show the connection's webhook URL. Bot
  senders on GitHub and Slack emit nothing, so a routine cannot trigger itself.
- Routine: one agent, a name, a prompt, a thread title, an enabled flag (on at creation, toggled
  from the table) and exactly one trigger:
  a five-field cron schedule evaluated in UTC, or one signal type of one signal-capable
  connection, narrowed by up to 20 filters (`routine_filters`): each names a context path and
  the values it may hold, and all must match (for a list value, any element may; scalars compare
  as their text, a missing path never matches). Prompt and title are `{{ key }}` templates: a signal's context is its data plus
  `provider_delivery_id`, `signal_type` and `summary`; a cron routine's is `scheduled_at`. A
  missing key renders empty; an empty title uses the routine's name. A deleted (soft-deleted)
  agent's routines no longer fire.
- Routine run: one firing, recorded once per fire key (the scheduled time, or the signal type and
  provider delivery ID) in `routine_runs`, so redeliveries and replicas never start a second run.
  Every firing creates a new thread with the rendered title and starts a run whose objective is
  the rendered prompt, followed for a signal by its type, connection slug, summary and data
  (each cut to fit the objective limit). The thread is the record; the run row keeps its thread
  and any error from starting it.
- Scheduling: a worker on every gateway process claims due cron routines with
  `FOR UPDATE SKIP LOCKED` every 15 seconds and moves each to its next time after now in the
  same transaction before firing (at most once; missed times are not backfilled). Signals fire
  synchronously from the webhook ingress after the delivery is authenticated, before any chat
  ingestion.
- Management: `RoutineService` lists, creates, updates and deletes an agent's routines and lists
  a connection's signal types and variables. The agent's Routines tab starts new routines from two
  pills above a table of the agent's routines: "Tilde catalog", which searches either the ready
  signal-capable connections or the catalog of providers that emit signals (the new connection's
  setup then opens the routine on it), and "Scheduled routine". The routine dialog has the
  form on the left and the template variables on the right; template fields show variables as
  inline badges (`components/template-input.tsx`).

### Signal catalog

Exact signal types per provider/connection type, as `ListSignalTypes` returns them. Routine
templates and filters name paths of a signal's context: its data below plus
`provider_delivery_id`, `signal_type` and `summary`.

- Linear (`linear`, types `api_key` and `oauth`; webhook secret field `webhook_secret`):
  `linear.issue.created`, `linear.issue.updated`, `linear.issue.removed`,
  `linear.comment.created`, `linear.comment.updated`, `linear.comment.removed`. Data:
  `action`, `url`, `organization_id`, `actor {id,name,email}`, `issue {id, identifier, title,
  description, url, priority, priority_label, state {name,type}, team {key,name}, labels [names],
  assignee {id,name,email}}`, `comment {id, body, url, user}` (comment events, else null) and
  `changed` (updatedFrom keys). Changes by apps or integrations emit nothing.
- Sentry (`sentry`, type `token`; field `client_secret`): `sentry.issue.created`,
  `sentry.issue.assigned`, `sentry.issue.resolved`, `sentry.issue.unresolved`,
  `sentry.issue.archived` (also Sentry's older `ignored`), `sentry.issue.regressed` (an
  unresolve whose `data.issue.substatus` is `regressed`, emitted beside it),
  `sentry.alert.triggered` (issue alert rules, resource `event_alert`) and
  `sentry.metric_alert.critical|warning|resolved`. Data is Sentry's payload as sent
  (`data.issue.*`, `data.event.*`, `data.triggered_rule`, `data.metric_alert`,
  `data.description_title`, `data.web_url`).
- AWS (`aws`, types `iam` and `oidc`; field `sns_topic_arns`, comma-separated, any number of
  accounts and regions in one connection): `aws.cloudwatch.alarm.triggered`
  (new state ALARM), `aws.cloudwatch.alarm.resolved` (OK) and
  `aws.cloudwatch.alarm.insufficient_data`. Data: `topic_arn`, `message_id` and `alarm {name,
  description, state, previous_state, reason, changed_at, arn, account_id, region, namespace,
  metric_name, statistic, comparison, threshold, period, evaluation_periods, dimensions {Name:
  Value}, ecs_service, ecs_cluster, load_balancer, target_group}`; the last four lift the
  ServiceName, ClusterName, LoadBalancer and TargetGroup dimensions (metric-math alarms report
  their first metric).
- Also: `github.*` (GitHub App), `slack.app_mention` and `slack.message.posted`, AgentMail, Linq,
  WhatsApp and Firecrawl types in `signals/`.

## Sandboxes

- Sandbox blueprint: the configuration of the VMs an agent works in. It names an installation
  E2B or Modal connection (the provider Tilde launches, sleeps, wakes and terminates VMs with),
  a template (E2B template, Modal registry image or `im-…` image; the image only needs the
  `tilde` CLI on its PATH), a reuse mode, encrypted environment variables (write-only, sealed per
  variable, exported in every shell of its VMs) and tool sources of its own, managed like an
  agent's (`agent_tool_sources.sandbox_blueprint_id` owner). Provisioning beyond the template is
  the customer's image; Tilde ships no credential helpers.
- Agent sandbox: an agent's setting naming one blueprint (`agent_sandboxes`, at most one). It
  adds the agent's `sandbox` tool source of the fixed sandbox tools (exec, exec_output,
  read_file, write_file, edit_file, apply_patch, list_dir, glob, grep), all enabled and fixed:
  like bundled tools they cannot be switched off or edited, and the source goes only with the
  setting. Every invocation
  of such an agent runs once its sandbox is up: the runtime calls `Sandboxes::ensure` before
  waking the agent, and a sandbox tool call ensures it again (waking it if it slept).
- Reuse mode (on the blueprint, so agents sharing it agree): thread (one per agent and thread,
  the default), agent, agent_identity (agent and person), global (one for every agent using the
  blueprint) and global_identity (one per person across those agents). The person is resolved as
  personal tools resolve it, rolled up to the root identity; without a verified person the
  person-keyed modes fall back to one per agent and thread. All modes but thread let data cross
  threads; the global modes also cross agents.
- Sandbox: one VM, keyed by blueprint, reuse mode and the agent, thread and person that mode
  distinguishes, and launched with a recorded connection. Status starting, running, sleeping or
  failed. Transitions (launch, wake, sleep, terminate) are made by the gateway process holding the
  row's lease. E2B VMs pause and resume with memory, so their process reconnects by itself; a Modal
  VM sleeps as a filesystem snapshot and wakes as a new VM from it. The blueprint's timings say
  when running sandboxes sleep (default ten minutes unused) and when any is terminated (default
  seven days unused), and how long a launch or wake waits for the process to connect (default two
  minutes; past it the invocation fails). Sandboxes whose blueprint reuse or
  connection changed, or whose agent no longer has the blueprint, are terminated by the sweeper
  (rows are deleted only after their VM is). Terminate is also a management action.
- Sandbox process: `tilde sandbox connect` in the VM (tilde-cli, Unix only). Tilde starts it with
  `TILDE_URL` (the runtime URL) and a single-use enrollment token, which its first
  `SandboxService.Connect` exchanges for a session token held in memory; reconnects use the
  session. It is never dialed: operations reach it as Connect frames through the transient
  `sandbox_calls` queue (as tool host calls do) and it answers with Respond. Each registration
  carries the blueprint's environment, written to `~/.tilde/sandbox.env` and sourced by shell
  profiles. Processes in the VM call the blueprint's tools through its loopback API
  (`127.0.0.1:4790/v1/tools`) or `tilde sandbox call`, which forwards to `InvokeToolStream` with
  the session; tool hosts receive the call's `sandbox_id` (and the agent and thread when the reuse mode
  keys by them). Each call is an `execute_tool` span of the agent's invocation, like its other tool
  calls: commands an exec runs see the operation as `TILDE_SANDBOX_OPERATION`, which
  `tilde sandbox call` passes back, so the call nests under that `sandbox.exec`; a call outside any
  operation (a background job's) belongs to the sandbox's latest invocation. Modal snapshots are
  images kept for 31 days, past the longest terminate-after. The sweeper also renews a running
  E2B VM's TTL before E2B would pause it, and puts a Modal VM to sleep before its 24-hour
  lifetime ends; it never sleeps a sandbox with operations in flight, which keep it in use.
- Egress grant: a blueprint's permission for its sandboxes to reach a connection's upstream
  through the gateway with the connection's credential, which never enters the VM
  (`sandbox_blueprint_egress`, managed with SetBlueprintEgress/DeleteBlueprintEgress; off until
  enabled). Its slug (`^[a-z][a-z0-9_]{0,31}$`) names it in the route; a blueprint grants a
  connection once. The connection must be ready, installation-owned and of a type with the
  `egress` capability, which a provider offers when it declares egress (`connections::egress`,
  built-in catalog code: the git forges GitHub, GitLab and Entire, the API providers in
  `catalog::apis` and AWS). A provider's egress is its protocols (HTTP with
  deny-by-default `METHOD /path` allow and deny rules, deny winning; git smart-HTTP with its
  services, how the upstream takes the token (Basic as a provider's user, or bearer) and whether
  pushes may delete refs, rewrite them or touch the default branch), its
  upstreams (fixed, or read from the connection by the hook: a self-managed GitLab's `base_url`,
  an Entire `cluster` host; never from a grant), and the JSON Schema of a grant's config. A grant's config is validated against
  that schema and stored with its defaults (`config_version` records the schema it was saved
  under); it can only narrow the rules. The provider's `EgressHook` checks each request against
  the rules and the config and names the narrowest credential to mint (`MintScope`). Setup queries
  (RunSetupQuery, e.g. `github.list_repositories`) fill config choices marked
  `x-tilde-options`. GitHub grants narrow to repositories, push branches (globs, default
  `factory/*`; empty: no push) and pull request writes; App connections mint tokens limited to
  them, personal access and OAuth tokens rely on the gateway's checks alone. Forge grants share
  their narrowing (`egress::forge`). GitLab grants narrow to projects (`group/project`, nested
  groups included), push branches and merge request writes; its REST allowlist is under
  `/api/v4/projects/{project}`, where a grant limited to projects only admits a project named
  by its encoded path (`group%2Fproject`), never by ID. Entire grants narrow to repositories
  (`project/repo`, served under `et/`) and push branches; pushes never delete or rewrite refs,
  and the default branch is pushable when the grant names it, since a tenant's repository is
  the agent's own. Entire has no HTTP protocol: its API is reached with tools.
- API egress (`catalog::apis`): HTTP egress to the providers agents read from sandboxes, each
  an allowlist of what agents need, its upstream the provider's or the connection's (never a
  grant's), its credential the connection type's stored mint (an OAuth access token refreshed by
  the connection as usual). None can narrow a credential per request, so the rules and the
  grant are the narrowing. Linear (`POST /graphql`): queries, and the mutations the grant names
  (default `issueCreate`, `issueUpdate`, `commentCreate`), read from the document by a small
  GraphQL scanner (`egress::graphql`: aliases, fragments, arguments; batches, subscriptions and
  unparseable bodies refused; a provider's denied fields refused at any depth). GraphQL requests
  carry no query string. New Relic: NerdGraph queries only, by region, never `apiAccess` (its
  API keys). Sentry (sentry.io or `base_url`): reads of the organization, its issues, events,
  releases, projects, teams, environments, tags, traces, replays, monitors and stats (and a
  project's), issue updates when `issue_writes` (default on); a grant narrowed to
  `organizations` refuses paths naming another or none. Jira (API token type only; the OAuth
  type's cloud-ID lookup leaves it tools-only): reads and JQL search on the connection's site,
  issue writes when `writes`. PagerDuty (by region): reads of incidents, services (never their
  integrations, by path or `include[]`; no `;` in the query), on-calls and schedules, incident notes (`notes`, default
  on), acknowledging and resolving (`incident_updates`, default off). HCP Terraform (`hostname`,
  else app.terraform.io): organizations, projects, workspaces, a run's events, redacted plans and
  applies (never runs themselves, which carry their variables' values); no `include` of state,
  outputs or runs, no dotted path segments (`.json`), no `;` in the query. Grafana (the
  connection's URL): dashboards, folders, data sources and their queries, annotations, alert
  rules, and an alertmanager's alerts and silences (never its config). Datadog (`api.<site>`),
  Prometheus (the connection's URL), Honeycomb (by region): read-only allowlists.
- AWS egress (`catalog::aws::egress`): the sandbox's SDK or CLI points `AWS_ENDPOINT_URL` at the
  route and signs with the egress token as its access key ID (any secret). The gateway reads the
  token, and the region and service from that signature's scope (the client's signature is not
  checked), sends the request to `https://<service>.<region>.amazonaws.com`, and signs it again
  with the connection's credentials in the grant's `account` (its own by default, else a role it
  lists). Only vetted services are supported, each with its protocol (`SERVICES`): RPC `logs`,
  `monitoring`, `ecs`, `ec2`, `rds`, `cloudtrail`, `dynamodb`, `sqs`, `sns`, `cloudformation`,
  `elasticloadbalancing`, `autoscaling`, `sts`, `ssm`, and REST `s3` and `lambda`; a grant's
  `services` is an enum of them (default `logs` and `monitoring`) and a request for any other is
  refused. A grant also names `regions` (empty: any) and `read_only` (default on). An RPC
  request goes to `/` (or exactly `/service/<name>/operation/<action>` for RPC v2), names its
  action only where its service's protocol carries it (`X-Amz-Target`, `Action` in the query or
  a form body, the RPC v2 path), once or agreeing, with one target, one content type and an
  unencoded body; read-only, the action is named Get, List, Describe, Filter, Search, Lookup,
  BatchGet, Query or Scan followed by a word, or Logs' StartQuery or StopQuery. A REST request
  names no action anywhere; read-only, it is a GET or HEAD. Paths with empty, `.` or `..`
  segments, encoded or not, are refused; a Query service's body is a form. Under any grant these
  reads are refused: STS other than GetCallerIdentity; SSM parameter reads,
  `GetCommandInvocation`, `ListCommandInvocations`; CloudFormation `GetTemplate`; EC2
  `DescribeInstanceAttribute`, `DescribeLaunchTemplateVersions`, `GetLaunchTemplateData`,
  `GetConsoleOutput`, `DescribeVpnConnections`, `GetVpnConnectionDeviceSampleConfiguration`,
  `GetFlowLogsIntegrationTemplate`; ECS `DescribeTaskDefinition`; Auto Scaling
  `DescribeLaunchConfigurations`; names with `Token`, `Credential`, `SecretValue`, `AccessKey`,
  `Password` or `ApiKey`. Anything else stored in AWS that a read returns (stack outputs, a
  Lambda function's environment, an S3 object) is the role's IAM policy's to guard. The
  refusals only mean something under a read-only grant: a grant that writes can reach the same
  secrets another way (an SSM `SendCommand`).
- Egress route: `{runtime}/sandboxes/egress/{slug}/{*path}` on the runtime listener
  (`sandboxes::gateway`). A sandbox process's egress token (issued with its session at
  enrollment, digest in `sandboxes.egress_hash`, valid while the row lives; a re-enrolled process
  gets a new one) authenticates as Bearer, `token` or a Basic password (git). The token's
  sandbox's blueprint's enabled grant `slug` names the connection (still ready,
  installation-owned and egress-capable); a grant saved under an older config version is
  refused. The token also comes the ways API clients send their keys, and none of them travels
  upstream: GitLab's `PRIVATE-TOKEN`, Datadog's `DD-API-KEY`, New Relic's `API-Key`, Honeycomb's
  `X-Honeycomb-Team`, PagerDuty's `Authorization: Token token=`, a bare `Authorization` (Linear
  API keys) and the access key ID of a SigV4 signature (AWS SDKs). The upstream's root (`/`) is a
  path like any other. Git smart-HTTP paths (`<repository>[.git]/info/refs|git-upload-pack|git-receive-pack`,
  the repository any number of plain segments) are git, anything else HTTP; the path is checked
  and forwarded as sent, still percent-encoded, except that escapes of unreserved characters
  are decoded first (`%76ariables` is `variables` to the rules too) and a `%` that starts no
  escape is refused. A push is read whole first (at most 100 MiB): its pkt-line ref
  updates, whether its pack proves each update a fast-forward (commits walked from new to old
  through the pack, offset deltas resolved; anything unprovable is not), and, for a protocol that
  protects it, the default branch
  from upstream's upload-pack advertisement (unknown refuses the push). The hook decides; the
  client's credentials, cookies and hop-by-hop headers are dropped, the scoped credential minted
  (git sends it as Basic `x-access-token` to GitHub, Basic `oauth2` to GitLab and as a bearer
  to Entire; HTTP in the provider's headers; AWS credentials sign the request with SigV4 for the
  service and region the client signed for), and the request streams to the provider's
  upstream (for AWS, the service's endpoint in that region). Redirects are not followed: a same-host `Location` is rewritten to the route. The
  exception is a git upstream with replicas (Entire's entry host redirects to nodes under its
  own host): the gateway buffers the request and follows up to three such redirects itself,
  with the credential, to the upstream's origin or a host under it only. 20
  requests a second per sandbox (burst 200). Each request is a `sandbox.egress {slug}` span on the
  sandbox's latest invocation (slug, protocol, method and path or git service and repository,
  decision and reason, status, ref updates as events). Registration carries `egress_token` (with
  the session only), `egress_env` (`TILDE_EGRESS_<SLUG>_URL`, `TILDE_EGRESS_<SLUG>_TOKEN` and the
  provider's HTTP templates for its standard clients, e.g. `LINEAR_API_URL`, `SENTRY_URL` and
  `SENTRY_AUTH_TOKEN`, `DD_API_KEY`, `GRAFANA_URL`, `HONEYCOMB_API_ENDPOINT`, `AWS_ENDPOINT_URL`
  and `AWS_ACCESS_KEY_ID`; `{token}` filled in by the process) and `git` rewrites of each git upstream to the route
  (the connection's upstream; for Entire both `entire://<cluster>/` and `https://<cluster>/`, so
  `git clone entire://...` works in a sandbox without Entire's remote helper);
  `tilde sandbox connect` writes them to the shell environment and `/etc/gitconfig` with
  `tilde sandbox credential` as the gateway's only credential helper.

## Memory

- Memory bank: a named container of memories (`memory_banks`). Shared, or an agent's own: created
  with the agent's id, linked to it read-write and never linked to another agent. Agent deletion
  is a soft delete, so `Lifecycle::delete` deletes the agent's own banks (their memories go
  with them) and its links in the same transaction; a deleted agent gets no new banks or
  links. A bank's scope is shared (agents see every memory) or per_person.
- Agent memory bank: a link giving an agent a bank, read or read_write (`agent_memory_banks`).
  Links are the only way an agent reaches memory; an agent with none has no memory tools. A
  read-write link may `auto_retain`: the agent's conversations then feed the bank's synthesis (a
  link no longer read-write stops retaining).
- Memory: one fact (`memories`): a kind (profile, preferences, entities, events, cases,
  patterns, tools, skills), a path (slash-separated segments organising the bank, '' for its
  root), title, content, an optional time it holds (`valid_during`: [t,t] for a moment, [start,
  end) for a period that ended, [start,) for an ongoing one) and importance. Its origin is explicit (an agent wrote it), owner (a person
  wrote or edited it through management) or synthesized (Tilde derived it from the bank's
  sources). The writer is a `memory::Actor` (person, agent or synthesizer); the `Memory` domain
  refuses anyone but a person superseding an owner memory, and an agent forgetting one, whoever
  calls it. In a per-person bank a memory agents write
  belongs to the verified person the invocation acts for (the root identity, as personal tools
  and sandboxes resolve it); agents see that person's memories and bank-wide ones people wrote
  without a person, and without a verified person cannot write there; only per-person banks
  keep memories for a person. A memory an agent writes records its thread and the latest message
  a person sent there (`thread_id`, `message_id`), left unset for a sidecar conversation the
  gateway has not projected yet. The origin follows the writer: an agent's memory is explicit, a
  person's owner.
- Supersession: a new memory naming the one it replaces (`supersedes_id`, unique, so a memory is
  replaced at most once; bank counts are of current memories). A superseded memory stays, with its history, but is no longer recalled,
  listed or browsed, even if its successor is forgotten; agents are told to recall the current
  one instead of changing it. Forgetting is a soft delete (`deleted_at`); an agent forgetting
  what it already forgot succeeds, so retries are safe.
- Revision: every create, supersede, update and forget appends the memory as it then is (kind,
  path, title, content, validity, importance, origin, person) to `memory_revisions`, with its
  actor (person, agent or synthesizer), the agent when an agent made it and the batch when the
  synthesizer did (`batch_id`), so a batch's forgets can be found as well as its writes.
- Recall: reciprocal rank fusion (k=60) of up to two ranked lists of 50 candidates: full text
  (any of the query's words: `plainto_tsquery`'s terms joined with OR as they are, without
  stemming them again, over a generated English `tsvector` weighting title words above content
  words, ranked by `ts_rank_cd`) and, once embedding settings exist,
  cosine similarity over pgvector `halfvec` embeddings computed under the current settings. The
  vector query runs in its own transaction with `hnsw.iterative_scan = relaxed_order` and
  `hnsw.ef_search = 100`, so filters still leave enough neighbours (pgvector 0.8 or later).
  Importance only orders memories with equal scores. Each result says which list found it
  (`keywords`, `meaning`). Filters: kind, path and below, and a period the memory's validity
  overlaps (timeless memories are left out when one is given; a period ending before it starts is
  refused). Without a
  query, recall lists the filtered memories newest first. The query is embedded with a 10-second
  timeout and no pooled connection held; recall answers from full text alone when the provider
  fails.
- Embedding settings: one installation setting (`memory_settings`): an inference connection
  (OpenAI, Azure OpenAI's v1 API, Vercel AI Gateway, Baseten, Voyage or Cohere), a model and 1024
  or 1536 dimensions. Setting them embeds a probe first, takes a new version from a sequence and
  asks for every live, current memory's embedding again; until a memory is re-embedded only full
  text finds it. The migration refuses pgvector older than 0.8.
  The column is untyped `halfvec`, with a partial HNSW index per supported dimension, so changing
  models needs no migration. `inference::prices::embedding_models` lists the vendored sheet's
  embedding models (`embedding_vector_size`) as suggestions.
- Embedding queue: `memory_embedding_jobs`, one row per memory waiting. A write, an edit or a
  settings change (re)requests it, stamping `requested_at`. The worker (`Memory::run`, in every
  process with chat tools) claims due jobs with `FOR UPDATE SKIP LOCKED`, leasing them by pushing
  `run_after` out, embeds them in one call through an `inference::Upstream` it builds from the
  decrypted connection (cached per credential version) and stores the vector only if the job was
  not requested again meanwhile. When the provider refuses a batch's content (400, 413, 422) each
  memory is tried alone and only those it refuses back off; otherwise (network, timeouts, 401,
  403, 404, 429, 5xx) the whole batch backs off, exponentially up to an hour. These calls, the
  settings probe and recall from the console are the organization's own spend in
  `inference_requests` (no agent, no bank); a recall query an agent's memory tool embeds is
  recorded in that invocation, so it is the agent's usage and counts toward its budgets.

- Synthesis: a bank's own chat model turning its sources into memories (`memory_synthesis`, on
  while the row exists: an OpenAI-compatible or Anthropic inference connection, a model,
  instructions of at most 2000 characters; saving probes the model; clearing it keeps the
  watermarks). Sources are the conversations of agents linked read-write with `auto_retain`,
  counted from their newest message when retaining starts (no history backfill; a conversation
  another agent already retains into the bank keeps its watermark), and the pages of wikis linked
  to the bank (`memory_bank_wikis`; unlinking one drops its pages' watermarks, and a batch finishing
  after the unlink sets none, so memories already synthesized from them stay). The synthesis worker, beside the embedding
  worker, claims the bank that waited longest (`FOR UPDATE SKIP LOCKED`, leased ten minutes) and
  takes one source part:
  - a conversation's complete messages after the bank's `(created_at, id)` watermark for it
    (`memory_synthesis_threads`) and before any message still streaming, at most about 16k tokens
    (each message cut to 8000 characters), once the conversation is idle (no invocation of the
    agent pending or running there and its newest message twenty minutes old) or, in a conversation
    the gateway runs, once about 4k tokens wait; conversations a sidecar runs (`thread_leases`)
    wait until idle. The last few messages before the watermark are shown as context, without
    evidence keys (in a per-person bank only agents' and the person's);
  - or a page's current content (cut to 64k characters) once it has been unchanged for five
    minutes and its revision is past the bank's watermark for it (`memory_synthesis_pages`). A
    page synthesized before comes with a line diff from that revision as the batch read it (cut
    the same way), at most 16k characters (a larger change sends the model to the page whole),
    and the model is told to record only what changed; one whose title and content did not change
    only moves the watermark, with no model call. A page deleted since is handled first: its synthesized memories whose whole
    supersession chain has evidence and all of it from that page are forgotten, without a model
    call, as a batch of their own, and its watermark goes.
  Evidence, batches and recall's `source` name a page by its current wiki and path, so a moved
  page still links; recall gives `source` only while that wiki is linked to the memory's bank.
  The batch's id is derived from the bank, the source and where the part ends. The model gets the
  time, the bank's name and description, its instructions, the kinds (`memory::kind_guide`, shared with the tools), the
  memories derived from the page (for a page) and related memories (the person's in a per-person
  bank) recalled from a conversation's first, middle and last items, or a page diff's added lines,
  or the page's title and windows at its start, middle and end, and the source keyed E1, E2…
  with each message's time. It is told the source is data, not instructions; that a person's
  statements are their claims and an agent's low-trust unless confirmed or an outcome; and that
  memories are self-contained, and to keep from a page only what the bank's agents need to recall
  rather than restating it. It answers in JSON with `write`, `supersede`, `forget` and `noop`
  operations (decoded as `synthesis::Op`; told to keep at most 20, the most durable; at most 20
  taken, output capped at 16000 tokens, an answer cut at the cap failing the batch). Tilde drops an operation it cannot read, citing no or
  foreign evidence, an invalid memory, a supersede of a memory it was not shown, a person's, a
  replaced one or another person's, or a forget of anything but a synthesized memory derived from
  the page being read whose whole supersession chain is from that page alone, noting why; it applies the rest (origin `synthesized`, actor
  `synthesizer`, evidence rows naming the batch, embedding requests) through the same
  `memory::insert` agents' writes use, with the batch row (`memory_synthesis_batches`: applied
  with counts and notes, including truncation, failed with the error, or skipped) and the
  watermark in one transaction. A batch found done only moves the watermark. In a per-person bank
  a conversation feeds only its verified person (its only active human, verified or attested):
  other people's messages are left out (noted), and a batch with none of the person's messages, or
  a conversation with no such person, is recorded as skipped without a model call. Wiki-derived
  memories are bank-wide. A failure shows its error on the bank. Only a failure the batch's
  content caused counts against it: an answer that is not JSON or not operations, an answer cut at
  the output cap, a 413 or 422, or a 400 whose error names the content (context length, content
  filter; `PostError::content`). Content failures are counted per source start (the bank, the
  source and its watermark), and the third passes the part over: its batch is recorded skipped, the
  watermark moves past it and the bank carries on, its failure count kept. Anything else (a connection not ready, network
  errors and timeouts, 401, 403, 404, 408, 429, 5xx, other 400s, recall or embedding failures)
  backs the bank off exponentially (`memory::backoff`, shared with embeddings) and passes over
  nothing, so an outage loses no conversation. Synthesis calls (and the recall they run) are
  recorded in `inference_requests` with the bank's `memory_bank_id` and no agent.
- Memory evidence: what a synthesized memory was derived from and in which batch
  (`memory_evidence`): exactly one of a message of a conversation or a page revision (kept by page
  id, since revisions outlive their page). Memories agents write keep their conversation on
  `memories` instead. Recall gives agents a wiki-derived memory's `source` (wiki and path), so
  they can read the page.

Agents use the built-in tools `memory.recall`, `memory.browse` (the paths directly below a path
with counts, grouped in SQL, and the id, kind, title, date and a 200-character preview of the
memories at it, each list capped and saying so when cut), `memory.write` and `memory.supersede`
(omitted fields, content included, keep the old memory's values; `valid_from` alone is a moment,
with `valid_to` a period, with `ongoing` an open one) and `memory.forget` (provider `tilde`, catalog prefix
`memory` reserved). Agents see each memory's kind, path, title, content, importance, origin and,
when set, validity (`ongoing` for an open period), not who wrote it or where, and are told owner memories are authoritative and to tell the
person when one looks wrong. Read links offer recall and browse only. Banks are chosen by name,
never id, and only when the agent has several. The recall, browse and write descriptions list the
agent's banks (name, shared or private to the person, the agent's own or not, read-only or
read-write, description), so an agent with one bank knows it too. One function builds the tools
from the agent's links, for listing and calling alike. A write's id is the tool call id, so a
retried call writes once; an existing id with another bank, writer or superseded memory is
refused. People manage banks, links, memories, settings and synthesis (settings, sources,
recent batches, run now) through `MemoryService`;
listing a bank with a query ranks it as recall does, with every person's memories. Recalled
memories enter a conversation only through these tools: prompts live in agent code, so Tilde
injects nothing.

The memory pages: **Memory** in the sidebar lists banks (owner agent or shared, scope, current
memory count, memories waiting for an embedding, linked agents) with New bank and, for
administrators, Embedding settings (an inference connection, a model from `ListEmbeddingModels` or
typed, and dimensions; saving runs the probe). A bank page has two tabs. Memories, master-detail:
search, kind, origin and (per-person banks) person filters and Add fact sit in the header's tab row;
a collapsible folder tree from the bank's paths (`ListMemoryPaths`, counts summed up the tree;
paths are the only grouping memories have), a dense table of the memories under the chosen folder
newest first a page at a time (`ListMemories` without a query pages by the last memory's id), or
ranked by a search with how each was found, and the chosen memory's panel; filters by kind, origin
and, in a per-person bank, person (`ListMemoryPeople`: each identity with memories there, named
after one of its users). A memory's panel renders its content as Markdown and shows its validity,
origin, writer, the conversation it came from, what it replaced or what replaced it
(`superseded_by`, by title), other memories in its folder, and its revisions; people add facts and
edit any memory in that panel with the rich Markdown editor (`markdown-editor.tsx`: tiptap with a
toolbar, a selection bubble menu, slash commands, task lists, tables, links, a raw Markdown view
and ⌘S/Ctrl+S to save) and delete any memory (editing an agent's memory is "Take over and edit": it
becomes the person's); a synthesized
memory's panel lists its evidence, linking to the conversation (`/agent/$agentId/sessions`) or
the wiki page, and `?memory=` opens a memory's panel. Settings: name,
description, linked agents and their access (carried on the bank's `links`), Synthesis
(`memory-synthesis.tsx`: its state, last error and the conversations and pages waiting (also
while off), the chat
model and instructions with Run now and Turn off, a note that each batch is a model call billed to
the connection, a switch per read-write linked agent for learning from its conversations, the
wikis it learns from (linking one first confirms its page count: every page is a model call), and
the latest ten batches linking to their conversation or page (`SourceLink`, shared with the
evidence panel) and the memories they wrote, listing those they forgot, with what they wrote,
replaced, forgot and refused and why), and
delete. Every listing reads the
`current_memories` view (live and not replaced) and adds its own person filter. An agent's
Capabilities have a Memory section: on creation a bank of its own (none, one pool or separate per
person, named after the agent unless renamed; created for the agent and deleted with it, with a
name another bank has refused up front) and other banks with read or read-write access, saved
right after the agent is created; afterwards the same choices change at once.

Memory always lives in the gateway's Postgres, never in a sidecar. A sidecar offers its agent the
memory and wiki tools by relaying their listing and every call to the runtime `KnowledgeService`
through the registry relay (`tilde.runtime.v1.KnowledgeService` is one of the relayed services
and is served only there; `knowledge.rs`). Calls go under the agent's own invocation token. The listing happens inside the sidecar's
tool registry, which holds only the verified scope, so the sidecar mints a minute-long token for
that scope with the agent's signing key (lease epoch unset, so it cannot be renewed). The gateway
checks either like the agent's own: signature, generation, thread lease, projected invocation and
capability ceiling, and checks `tools_invoke` again. A gateway the sidecar cannot reach hides the
memory and wiki tools from that listing only.

## Inference gateway

Agents call model providers through `/inference/{provider}/{account}/{*rest}` on the
runtime listener (gateway or sidecar) in each provider's native wire format: an agent
points its OpenAI, Anthropic or Vercel AI SDK client at the slug via
`ctx.inference("openai/prod")` and keeps its code. The hot path verifies the invocation
token like every runtime RPC, checks the connection id is in the token's `inference`
claim, drops the SDK's placeholder credential and hop-by-hop headers, injects the real
header or SigV4 signature from an in-memory map, and relays bytes both ways without
decoding them. Request bodies are buffered (SigV4 and retries need them); responses
stream through with each frame shared into a bounded capture. Supported providers are
OpenAI, Anthropic, Azure OpenAI, Azure AI Foundry, Google AI Studio, Amazon Bedrock,
Baseten, Cerebras, Vercel AI Gateway, Cohere and Voyage; each is a static-credential
catalog entry whose setup validates that its fields form an upstream. Paths are
allowlisted by final segment into kinds: chat, embeddings, rerank, count_tokens,
image, transcription and speech; anything else is refused. Realtime stays a chat
provider concern.

The gateway keeps every ready inference connection decrypted in `inference::Upstreams`,
rebuilt on the `tilde_sidecar_configuration` notification and a slow timer; a sidecar
derives the same map from its replicated configuration, which now carries each
connection's capability. The `inference` claim is computed at token issue and renewal,
so unassignment and future budget exhaustion apply within one renewal without a
per-request lookup. Usage is accounted off the request path: the capture is parsed by a
worker (model from path or body, tokens from the provider's usage object, including SSE
streams) into `inference_requests`; a sidecar ships records through its outbox as
`InferenceUsage` frames, shed first under backpressure. Calls Tilde makes itself
(`Upstream::post_json`, for memory) are recorded as they answer, billed to an
`inference::Payer`: an invocation, a memory bank, or the organization, so their rows may have
no agent or invocation.

Cost is priced by Postgres at insert time from `inference_prices`, in micro-dollars per
million tokens: cached input is a subset of input for OpenAI-style usage and additive for
Anthropic and Bedrock; an unknown model leaves `cost_micros` NULL. The table is reconciled
at startup from `crates/tilde/src/inference/prices.json`, a trimmed copy of LiteLLM's
community price sheet refreshed by `scripts/update-inference-prices.py`; rows with source
`manual` are operator overrides and survive reconciliation. Budgets (`inference_budgets`)
cap spend per agent or per chat identity (`chat_runs.source_identity_id`) over a day, month
or in total, with action block or flag. A worker re-sums costs every 30 seconds and sets
`exhausted_until`; only exhaustion changes fire the configuration notification. Enforcement
happens at token issue and renewal: a blocked scope empties the token's `inference` claim,
on the gateway through SQL and on a sidecar through `inference_blocked` and
`inference_blocked_identities` in its configuration. The request path never reads a budget.
An agent budget may name one connection to cap only that connection's spend; the token then
withholds just that connection. Identity budgets target root identities and roll a run's
sender up through `chat_users.root_identity_id`. Management exposes usage per connection, a
daily spend series over a window, the agent's all-time total, and budget CRUD through
`InferenceService`. The Inference tab draws the month's daily spend per connection (shadcn
chart, month-by-month navigation, total and month-to-date beside the title) above the table,
which shows spend and an inline monthly budget per connection.
Inference security middleware builds on the same records and claims.

## Prompts

Prompts live in agent code and are registered with a deployment, never edited in the UI.
`tilde deploy` (TS `@trytilde/sdk` bin, Python `python -m tilde deploy`) imports the agent's
entry with `TILDE_DISCOVERY=1` (hosts become no-ops), finds what the code declares and sends it
as `RegisterDeploymentRequest.declarations`; `--dry-run` prints those declarations as JSON, which
is how `tilde dev` registers the examples. Declarations come from two places:

- framework adapters, which read the framework's own objects so agents are written as its docs
  show: Mastra agents' instructions and skills (`@trytilde/sdk-mastra-node`), CrewAI `@CrewBase`
  YAML read before interpolation (`tilde-crewai`, entry point group `tilde.discover`). Adapters
  are found by object type among the entry's exports (TS) or the globals of project modules
  (Python), so agents are defined at module scope and `run(ctx)` calls them;
- `definePrompt`/`define_prompt`, `defineSkills`, `defineSkill` for code without a framework.

A prompt version has a format: plain, mustache (`{{var}}`, `{{> section}}` partials stored per
version in `prompt_sections`), braces (`{var}`), or dynamic (a function; the template is its
source and the text exists only at runtime). Every format hashes under one framed SHA-256 rule
shared by `prompts::content_hash` and both SDKs (config is canonical JSON, `{}` by default), and
records its origin (`config/agents.yaml#responder.goal`). Registration upserts a version per
(prompt, hash) and links it to the deployment (`deployment_prompts`); `prompt_versions.
deployment_id` is the first deployment that shipped it. A repeated external id ignores
declarations: a deployment's contents never change.

The gateway links every inference call to the versions that produced it
(`inference_request_prompts`), off the request path in the audit worker: the request's
system, developer and user text is matched against the invocation deployment's plain, mustache
and braces versions (each literal segment in order within one message, at least 16 bytes of
literal text), and `x-tilde-prompt: name@hash, …` stamps name versions the text cannot, dynamic
ones in particular. SDK framework helpers stamp dynamic instructions (Mastra's per-call
processor); `ctx.prompt(def)`/`render()` stamp definePrompt prompts. Request bodies never leave a
sidecar: its Watch snapshot carries the deployment's text-matchable versions as literal segments
(`Snapshot.prompts`, kept for the replica's life since a deployment's prompts never change), the
replica's audit worker matches each body against them with the same matcher, and the frame ships
matches and header stamps alike as `name@hash` (`InferenceUsage.prompts`) for the gateway to resolve. Module-level `inference(slug)` (TS AsyncLocalStorage, Python
contextvar) resolves the running invocation per request, so a framework model is built once at
import while every call still carries its invocation's token and stamps.

The management `PromptService` is read-only: list, history with usage per version (requests,
tokens, cost, latency). At runtime
an agent lists its own prompts freely and another agent's with `agents.read`. The Prompts tab shows
versions newest first with format, origin and first deployment, files with a line diff, and
usage; a deployment's drawer on the Deployments tab lists the prompt and skill versions it
shipped (`GetDeploymentContents`, which also returns its declared bundled tools). Per-deployment and per-invocation overrides for experiments
are the planned next step; nothing in the version model changes for them.

## Wikis

Wikis hold durable reference pages people and agents both read: runbooks, decisions, how things
work. Memory banks hold the short facts an agent recalls; a wiki is where longer, structured
knowledge is kept up to date (ADR-0014).

- Wiki: a named tree of Markdown pages (`wikis`). Shared, or an agent's own (at most one per
  agent, as with memory banks; a second is refused): created with the agent's id, linked to it read-write, never linked to another agent, and deleted with it in
  `Lifecycle::delete` (with its pages and their history).
- Agent wiki: a link giving an agent a wiki, read or read_write (`agent_wikis`). An agent with no
  link has no wiki tools.
- Wiki page: a path unique in its wiki (slash-separated segments), a title, Markdown content and
  its current revision (`wiki_pages`). A generated English `tsvector` weights title words above
  content words.
- Page revision: the page after each create, update, move or delete (`wiki_page_revisions`), with
  the writing agent and the conversation it wrote in (`thread_id`, unset for a person or a sidecar
  conversation not yet projected), and the write's `call_id` (unique): an agent's tool call id, or
  a fresh id for a person's change. Deleting a page removes its row; its revisions stay in
  the database, with the last content in the delete revision, though nothing shows a deleted
  page's history.
- Stale write: an update, move or delete names the revision it read (`expected_revision`, 0 to
  create); when the page has moved on it is refused (ABORTED) naming the page's current revision,
  read again at that point, and the writer reads the page again.
- Retried write: a write whose `call_id` already has a revision returns that write's result (the
  page, or success for a delete) without writing again, found with one lookup, when it is the same
  write: the recorded page, wiki, operation (and path, for a create or a delete by path) match,
  and for an update, move or delete the recorded revision is the one after the revision it read.
  Otherwise the call id was reused for a different write and is refused (INVALID_ARGUMENT). A call
  racing its own retry loses on the unique `call_id` and returns the winner's result. Creates,
  updates, moves and deletes are all retry-safe.
- Edits: an update replaces the whole content or applies edits in order, each replacing an `old`
  text that must occur in the page exactly once (INVALID_ARGUMENT names the edit that does not).
- Search and grep: search joins the query's words with OR (as memory recall does) and ranks by
  `ts_rank_cd` with an excerpt whose matches are bracketed by U+E000 and U+E001 (private-use
  characters, so Markdown in pages cannot be mistaken for them; agents get `**`, the page shows a
  highlight). Grep matches a literal (special characters escaped) or a regular expression line by
  line under a three-second statement timeout, cuts each line to 500 characters and stops at 200
  lines or 128 KiB, so responses stay far below the relay's and management's message limits. An
  invalid or too slow pattern is the caller's to fix (INVALID_ARGUMENT).
- Wiki tools: `wiki.search`, `wiki.grep`, `wiki.read` (a page and its revision, optionally a line
  range, reporting `from`, `to` and `total` lines; a start past the end is INVALID_ARGUMENT),
  `wiki.list` (optionally under a path `prefix`; at most 500 pages, with `more` asking to narrow),
  and with a read-write link `wiki.write`
  (create, or at the revision read change the content by `edits` or replace it) and
  `wiki.delete`; with `agents.edit_wikis` on another agent, `wiki.link` (give it one of the
  caller's linked wikis other than its own, read, or read_write only when the caller's link is,
  linking again sets the access) and `wiki.unlink` (take it away), never on the caller itself.
  They are `SetAgentWiki` and `RemoveAgentWiki` for agents, the caller's own link standing for
  its access to the wiki. As for memory, a wiki is chosen by name only when there is
  a choice, the tool text describes the linked wikis, listing and calling share one gate, and
  sidecar-hosted agents reach them through the relayed `KnowledgeService`.

People manage wikis through `WikiService` (`wikis::rpc`). The **Wikis** sidebar page lists them;
a wiki's page has search (words, exact text or regex) and New page in the header's tab row, its
collapsible page tree (the open page is in the URL, `?page=<path>`), beside the open page its
outline and the pages linking to it (grep for Markdown links naming its path), the page rendered
from Markdown (`react-markdown` with GitHub's extensions, raw HTML not rendered; `wiki-markdown.tsx`:
a link without a scheme names a page by its path from the wiki root (`/runbooks/deploys`; a
leading `./` and a trailing `.md` are ignored) and opens it here, absolute links open outside the
app, and
images from other origins show as links and are never fetched), editing and creating pages in
place with the rich Markdown editor (the memory panel's; its raw Markdown view covers anything the
rich view does not show) that saves at the revision it opened, asks before discarding unsaved changes (any navigation
through the router's blocker, Cancel, leaving the browser page) and, when someone saved first,
shows their changes and overwrites them with the draft at the latest revision or discards it; a
`?page=` naming no page offers editors to create it, move and delete, a history linking each agent
change to its conversation, where a revision shows what restoring it (title and content) would
change in the current page with a line diff folding long unchanged runs (`line-diff.tsx`), search
and grep, and a settings tab with linked agents (`linked-agents.tsx`, shared with memory banks; wikis and banks
carry each link's access). An agent's Capabilities have a Wikis section beside Memory: on creation
a wiki of its own ("Created for this agent and deleted with it", named in both modes) and other
wikis (`LinkedResources`, shared with the Memory section), saved right after the agent is created,
the own wiki and the links each once even when a save is retried; Create agent waits while the own
bank's or wiki's name is another's. Afterwards each change saves at once.

## Skills

A skill is a directory with a `SKILL.md` (front matter `name:` and `description:`, the latter
being what an agent sees before reading it) plus the files it uses. Skills are grouped into
skill sources, the usual unit agents are given; there is
no registry. A source is one of:

- catalog: an entry of the catalog (`skills::catalog`), either a built-in Tilde group compiled
  into the engine (files under `crates/tilde/src/skills/catalog/`: Tilde runtime, Tilde working
  style, Tilde platform) or a managed provider: a trusted GitHub repository and branch (Anthropic,
  Microsoft, Cloudflare, AWS, Cursor, Notion, Granola, Parallel, Superpowers, Browserbase, Cua,
  Apollo.io, Zoom, Stripe, Neon, Vercel, YC Software QM) narrowed by include scopes (directory
  prefixes, or exact files that contribute no skills) and exclude prefixes; only skills whose
  files lie within them are kept, and `translations/` never is. Enabling an entry creates the
  installation's one source for it. Startup re-syncs enabled built-in
  groups, so an engine upgrade versions the skills it changed; providers sync like git sources
  (on enable, hourly, on request), never at startup. `ListCatalog` returns each entry's category
  and icon (served from `web/public`), a provider's repository and branch, and its skills (name,
  description, repository path): built-ins from the binary, providers once synced.
  `GetCatalogGroup` returns one entry for its panel; a provider not yet enabled is previewed
  live (commit, tree, then only each in-scope SKILL.md, scoped and named as a sync would), stored
  nowhere and cached in memory per provider for an hour.
- git: a GitHub repository read through the REST API (`ENGINE_GITHUB_API_URL`, optional
  `ENGINE_GITHUB_TOKEN` for private repositories and rate limits): the ref resolves to a commit,
  the commit's tree is listed (two API calls), files are downloaded from raw content at that
  commit (`ENGINE_GITHUB_RAW_URL`), and every `SKILL.md` under the optional path is a skill owning
  the files beneath it except those of a nested skill. Syncs run on add, hourly and on request;
  a failure is recorded on the source (`sync_error`) and keeps the last good skills. Each sync
  claims the next `sync_generation` before fetching; one that finishes after a newer sync started
  is dropped, so an older commit never overwrites a newer one.
- editor: a collection authored in the Skills pages, or by an agent holding `skills.edit`. A save
  based on a version that is no longer the latest is refused, and the editor reloads.
- code: one per agent (`skill_sources.agent_id`), filled by `tilde deploy` from skill folders the
  agent's code or framework declares. Its skills can have several live versions at once, one per
  deployment (`deployment_skills`); it is read through its agent and never appears
  in source lists, pickers or assignments. An invocation sees its own deployment's bundled skills,
  which shadow assigned skills of the same name. Files too large or binary to send inline are
  uploaded first (`MissingDeploymentFiles`, `UploadDeploymentFile`, content-addressed).

Bundled skills are pinned per deployment; everything assigned through the registry (sources,
single skills, connection links) is live: it applies to every deployment of the agent at its
latest version on the next invocation, with no redeploy. `ListSkills` marks which skills the
deployment shipped (`deployed`, already on disk beside the code); SDK framework helpers deliver
the rest through the framework's own skill mechanism (`ctx.skills.directory()` materialises them
into a content-addressed cache directory reused across invocations), or, for frameworks without
native skills, as Tilde skill tools plus a summary in the instructions.

Package rules follow Tilde's hosted skills: safe relative paths, no symlinks, 1 MiB for
SKILL.md (read from object storage when over the 256 KiB inline limit), 10 MiB per file, 64 MiB
and 2048 files per skill; names are lowercase, unique within
a source, and repeats are suffixed. Every distinct file set is an immutable version whose hash
covers each file's path, content digest and executable bit. Files are content-addressed objects
(`skills/blobs/sha256/..`) in `ENGINE_SKILLS_S3_BUCKET`; UTF-8 files up to 256 KiB are kept
inline in Postgres instead, and without the bucket only such files are accepted. Management
reads and agents get text inline and other files through 15-minute presigned URLs.

An agent adds a source
(`agent_skill_sources`) switched off. Switched on, it gives the agent every skill in it,
including those a later sync adds, except skills switched off one by one
(`agent_skill_exclusions`); switching it on again clears those, and switching it off drops
every skill of it, single skills given from it (`agent_skills`) included. Removing a source
drops its single skills and exclusions too. Agents are also given skills through a connection:
sources linked to a connection (`connection_skill_sources`) reach every agent assigned that
connection's `skills` capability while the connection is ready. Every connection offers the capability.

The Skills pages call sources groups. The Skills page (sidebar) lists skills grouped by
source, collapsible and searchable, and adds sources from the Tilde catalog (its own page,
`/skills/catalog`, sectioned by category with a panel per catalog group), a Git repository or
the editor. Sources have no page of their
own (old `/skills/sources/<id>` links redirect to `/skills`): each group row syncs
(catalog and git), links connections and deletes. A skill page shows the latest files in a
file tree, markdown through a tiptap editor, and edits editor skills only (catalog, git and
bundled skills are read-only here and in the API); each save is the next version (history is
kept, not shown), unchanged assets are sent back with `keep`, and the name and description are
SKILL.md front matter edited inline, where a new `name:` renames the skill in place. The agent's Skills tab is one table
grouped by source, like its Tools tab: a Use switch per source and, while it is on, per skill, bundled skills
(locked on), connection-given skills marked "via" the connection, and an Add skills menu, as on
the Tools tab: choose an existing source (added switched off),
or go to the Skills page to add new skills.

Every wake carries `InvokeRequest.state` (`InvocationState`): what the agent should hold
locally for that invocation, today every skill it sees at its version (bundled ones marked
`deployed`); more pushed metadata belongs there. The SDKs keep registry skills in one folder
per agent under the OS temp directory (`TILDE_SKILLS_DIR`), remember each skill's version in
memory, and on each invocation download only new or newer versions and delete removed skills.
Agents read skills through the runtime `SkillService`: `ListSkills` returns every skill they
are given with its source slug, description and file list (SDKs use the pushed state instead
for their own agent); `ReadSkillFile` takes `name`, or
`source/name` when two assigned sources share a name. Both take another agent's id under
`agents.read`; such listings omit bundled skills, which are the agent's own implementation
(management still shows them on its Skills tab and deployments). An agent
sees the sources it holds `skills.read` on (targets are source ids; `skills.edit` implies it). Assigning a source (switched on whole) or a skill needs `agents.edit_skills` on the receiving
agent (itself included) plus read on the source; unassigning needs only `agents.edit_skills`. With
`skills.edit` on a source it writes skills into an editor source and re-syncs git and catalog
sources. Runtime file info carries the executable bit, which `ctx.skills.directory()` restores.
The SDK exposes this as `ctx.skills.list/read/summary/materialize/sources/assign/unassign/
write/sync`. Sidecars relay the RPCs to the gateway. The same writes are built-in tools for
models (`skills::tools`, gateway-hosted agents): `skills.write` `{source, name, files, message?}`
(text files, the whole skill; the same files again change nothing), `skills.assign` and
`skills.unassign` `{agent_id, source | skill}` (`skill` as `source/name`), authorized like
`WriteSkill`, `AssignSkill` and `UnassignSkill`.

## Chat

- User: a specific human conversation identity. A channel identity retains its provider
  address and participant ID; native Tilde identities use an application subject.
- Root identity: an optional grouping of conversation identities. Each identity belongs
  to zero or one root. Grouping does not change participants, routing, history, or grants.
- Thread: one conversation with a primary agent and human/agent participants. Native
  threads and external threads share the audit model; each external thread has one channel binding.
- Participant: one user or registered agent participating in a thread.
- Message: explicit visible content; streamed messages are streaming, complete or aborted.
- Activity: ordered per-thread lifecycle facts and reasoning, separate from the transcript.
- Goal: one agent's desired outcome in one thread.
- Task: one agent's work item in one thread, optionally linked to a goal and existing dependencies.
- Run: a durable objective that can wait across invocations.
- Invocation: one active reasoning loop for an agent in a thread; stop ends only this loop.
- Suspended run: a run whose invocation saved its framework state and ended so a later one
  continues it (`chat::suspension`). The SDK adapter saves the conversation in its model's
  message format with the tool calls it waits for (`SuspendRun`, `chat_run_checkpoints`, one per
  run, opaque JSON like converted messages); the run moves to `suspending` and, when the
  invocation ends, to `waiting` without handing its inputs on. Once none of those calls is
  running and the agent runs nothing else in the thread, the worker resumes it (`ResumeRun`'s
  path): the next invocation's wake has `resumes`, and `GetCheckpoint` returns the state and the
  calls' records. The invocation records why it was asked to suspend
  (`chat_invocations.suspended_for`: `manual`, `deadline` or `pending`): only the last two resume
  by themselves; a manual suspension waits for ResumeRun, which continues from a saved checkpoint.
  An invocation asked to suspend for its deadline that saved nothing (its loop finished before
  its next step boundary) ends as any stop does. A checkpoint names only calls of its own run's invocations. A run its channel
  may no longer start is left to ResumeRun. A message that arrives meanwhile starts its own run as any message does; the
  suspended run resumes after it (checked again under the thread lock). An invocation that ends
  without suspending drops the run's checkpoint (a failed one keeps it, so ResumeRun continues
  from it).
- Invocation deadline: Tilde, not the agent, owns the host's runtime limit. A Lambda or hosted
  wake sets `chat_invocations.suspend_at` to Lambda's fifteen minutes less a two-minute margin;
  at it the run moves to `suspending` (activity `invocation.suspend_requested`, `deadline`), the
  control stream delivers a suspend command and calls in flight answer pending. Only an
  invocation that can suspend is asked (`chat_invocations.suspendable`, set by its first
  suspendable call); any other runs to its host's own limit, as before. An invocation whose
  lease expires after it saved a checkpoint it was asked for still resumes. Dial-in hosts
  have no limit.
- Steering input: a durable API event, scheduled into a fresh invocation according to the agent's concurrency policy.
- Agent host: an SDK process that dials in to Tilde (`RunService.Watch`) with a deployment token, receives wakes as frames and opens outbound invocation control subscriptions. Tilde and its sidecars never call an agent.
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
authenticated layout, and public brokering lives outside it. The registry's pages (agents,
skills, tools, sandboxes, memory and wikis) live below `/registry` (`_app/registry/`); the root
route redirects their earlier paths (`/agent/...`, `/skills...` and so on) there, keeping search
and hash. Agent routes live in
`_app/registry/agent/`, with the shared editor and its tabs grouped in `$agentId/`.
Standalone Connections and Chat pages are not exposed in the registry. Agent rows open
`/registry/agent/{id}/capabilities`;
`/registry/agent/{id}/chat-providers` and `/registry/agent/{id}/iam` are sibling tab routes under a shared
agent editor. Agent details are fetched by ID, so direct URLs and refreshes do not depend on previously loading the registry. The
shared editor preserves local state across tab navigation and browser Back/Forward. The dashboard header
shows Agent Registry / agent-name breadcrumbs, updated from loaded and saved route data.
Pages can contribute a DashboardNavigation slot beside the breadcrumbs; agent tabs use a
React portal so their panel context and keyboard behavior remain intact.
`/registry/memory` lists memory banks and `/registry/memory/{id}/memories` and
`/registry/memory/{id}/settings` are a bank's tabs, like a sandbox blueprint's; `/registry/wikis`
and its `{id}/pages` and `{id}/settings` do the same for wikis. `/registry/agent/new` creates an
agent; the registry is `/registry`, which `/` redirects to.
The sidebar collapses to icons (item names become tooltips).
Connection brokering has a
separate public route outside the authenticated app layout. Connection setup and
identity verification host modules live under `web/src/routes/connections/` with
`-` prefixes to exclude them from route generation. Shared connection UI loading
keeps embedded content mounted but hidden until ready, with bouncing dots and
Motion fades that respect reduced motion preferences.

## Agent access and identity verification

Agents have a typed sending identity for each assigned channel, stored in
`agent_channel_identities` and linked by foreign key to `connection_agents`.
It is separate from recipient `chat_channel_identities` and from editable
connection labels. Catalog adapters derive it from the provider account: an
AgentMail inbox email, Linq/Telnyx sending number, Meta's actual display phone
number, Slack bot user ID, or GitHub app bot username. Setup completion and
assignment of a ready connection store it atomically; unassignment removes the
old link. Startup restores missing identities from stored provider data without
sending messages. Legacy Meta channels with only a Graph ID need reconnecting
to obtain the actual phone number. Verification requires a known sending identity.

Connection setup and identity approval share `ProviderPage` from connection-ui:
Tilde wordmark × provider icon, a title, description and body. Approval shows
“Link recipient identity to agent” and explicitly describes incoming and outgoing
messaging using the stored sender and recipient identities, never a display label.

The agent access tab uses management-only AgentAccessService RPCs. Access belongs to the
agent/channel assignment, with private, public, and disabled modes. Existing assignments
migrate as public; new assignments start private (GitHub starts disabled). Public still
records every sender identity; private accepts only verified or management-attested identities explicitly allowed
for that agent; disabled accepts no provider traffic into agent conversations. Blocked
callbacks retain a deduplicated access-decision audit without exposing their content to
agent history. Mode/grant changes invalidate runtime access and cancel affected invocations.
Queued messages and claims recheck current policies under the connection lock.

An identity is a provider-owned string `value` with an `email`, `phone_number`, or `username`
type, unique within the connected account. Tilde never normalizes identity values;
provider adapters construct inbound identities and validate verification recipients.
Verification is independent of an agent's allow flag. Management can attest an identity
when creating it with `skip_verification`; this records `attested_at` separately from
provider verification and never creates an agent allow grant. Management creates a pending request
and the provider privately delivers the secret approval link. Only its hash is stored.
The link expires after ten minutes; resends supersede prior proofs and are throttled.
Reading the page never approves. An explicit public Approve RPC consumes the delivered
proof and grants only the bound identity/account/agent. Management clients never receive
the token or approval URL. Removing the assignment invalidates its proofs and grants.

`/identity/verify/{id}` hosts the public, no-login approval flow and a sandboxed shared React
iframe; only the host holds the identity-verification token. AgentMail, Slack, Linq, Meta
WhatsApp and Telnyx deliver verification invitations through their existing credentials.
Adapters tailor the invitation to the identity/channel: AgentMail supplies an
agent-named subject and an HTML acceptance link with a plain-text fallback; Slack
uses a native link; Linq and WhatsApp use plain-text links. All include the
ten-minute expiry and unsolicited-invitation guidance. Approved WhatsApp template
copy remains provider-managed.
WhatsApp can use an approved one-parameter template outside its customer-service window.
GitHub supports public/disabled only in this pass because it has no private message API.
Native Tilde conversations remain separate from connection-backed provider runs; an outbound
channel binding must not silently revoke a native invocation's existing authority.

## Agent pause and deletion

Management PauseAgent persists `paused`, cancels active invocations and blocks
connect-token renewal. Issued tokens remain valid until their five-minute expiry.
Executions observe cancellation through their own outbound
control subscriptions. Pause reports the durable state change; it does not claim
synchronous host acknowledgement. Pending work waits for ResumeAgent. Each invocation
establishes its control subscription before running user code, so a stopped invocation
receives its stop even when the SDK host starts late on a different serverless instance.

`InvocationControlService` on the runtime listener streams scoped stop and suspend
controls. Message and explicit steering inputs stay in the runtime-owned queue. It allows a narrowly scoped terminal-token window to acknowledge
stops after ordinary token expiry. PostgreSQL LISTEN/NOTIFY at the gateway, or
in-memory change notifications on a sidecar, wake delivery. Input IDs deduplicate
queue insertion, including retries after interruption. Control subscriptions are renewed with current tokens
and the SDK aborts work after a prolonged loss of the control connection.

SuspendInvocation (and an invocation's deadline) transitions the run through `suspending`; the
control stream delivers the suspend command once and stays open, renewing the lease, until the
invocation ends. A framework adapter that can suspend (see SDK framework adapters) brings its
loop to its next step boundary and saves it with `SuspendRun`, or is suspended with its last
boundary's state after a minute; without one, the SDK checkpoint hook persists framework state
and the invocation ends at once. Execution then exits into `waiting`. ResumeRun starts a new
invocation and carries unconsumed input forward; a saved checkpoint resumes by itself. Hosts expose
no service: the inbound Invoke, Healthz, Steer, Cancel and Stop RPCs, per-host stop fences,
synchronous pause receipt and redundant sidecar push jobs are removed. Dial-in hosts and
Lambda handlers use the same SDK lifecycle.
Deletion requires pause, erases grants, removes connection assignments,
and retires the registry entry and thread participation. Conversation and audit history
remain readable; retired IDs cannot be reused. Paused state is independent of health.

Management IdentitiesService creates standalone identities by default. `create_root` creates
and links a root in the same transaction, mutually exclusive with `root_identity_id`.
Native subjects omit the agent/provider selector; channel subjects supply an agent ID
and the chat provider's `provider/account-name`. Root creation can atomically link existing
identity IDs. Linking rejects another root; unlinking supplies the expected current root.
These operations retain the identity's chat user ID and never merge conversation history.

## Agent identity and avatars

An agent is identity and policy only. It has no endpoint and no wake-signing key: nothing
calls an agent, so there is nothing to address or sign. How it runs belongs to its deployments.
Its identity is a name (1-200 characters) and an optional free-text description (at most 500
characters, empty by default), both trimmed, edited inline under the name in the agent header.
Each agent has a stable random avatar seed, derived from its UUID at creation. The
MIT-licensed Dispatch avatar renderer lives in `sdk/ts/packages/agent-avatar` and animates
in the registry. Custom raster images are uploaded through management UploadAgentAvatar,
stored in private S3 objects, and exposed by short-lived signed read URLs. Replacing an
image preserves the generated seed and removes the old object after the database commit.
S3 is configured with ENGINE_S3_*; task dev starts a local MinIO bucket. Agent capability
targets use a stacked-avatar picker backed by management name search before pagination.

## Agent health and registry metrics

`agent_health` stores aggregate deployment readiness every 30 seconds, including
unhealthy samples when no deployment can serve. A transaction advisory lock prevents
overlapping sweeps. Gateway and Sidecar samples use connected-instance heartbeats;
Lambda is considered routable without a probe. Per-replica sidecar samples carry their
`instance_id` and are excluded from registry history; historical HTTP probe rows were deleted.
Startup and hourly cleanup delete checks older than 14 days. Sampling stops with the server.

Registry responses include 12 UTC hourly buckets (current partial hour last),
latest status (Unknown after 90 seconds or with no checks), participating thread
count, completed visible agent replies per thread, and invocation-to-first-visible-
reply latency. `chat_messages.first_content_at` captures the first non-empty
streamed chunk once; existing messages without that measurement are excluded from
response averages. No-data hours and missing response samples are not represented
as healthy observations or zero latency. The registry refreshes every 30 seconds.

ListAgents filters before keyset paging, in SQL: `search` matches name or description
case-insensitively (or an exact UUID), `health` matches the same latest status the
metrics report (Healthy, Degraded, Unhealthy, or Unknown when the newest check is older
than 90 seconds or missing), and `paused` selects paused or active agents. The home page
keeps these as `q`, `health` and `state` URL search params.

Rust domain code is grouped under `agent/{mod,health,rpc}.rs` and
`chat/{mod,rpc,runtime}.rs`. Conversation contracts and storage use Thread,
`thread_id`, and `chat_threads`; registry metrics use `thread_count` and
`average_turns_per_thread`. This is a breaking rename without legacy aliases.

Chat provider tools use an invocation-scoped ConnectRPC catalog and the streaming
`InvokeToolStream` transport (input frames in, heartbeats then the result out). The SDK registers their server-authored descriptions and
schemas as framework tool wrappers before the agent loop. Sending is provider-owned;
there is no SendMessage RPC or shared sending trait. Providers explicitly publish
canonical message events through scoped lifecycle helpers. Native text sending is one provider implementation. Connection-backed adapters expose
provider-specific schemas and handlers through the same catalog. Stop stays SDK-local,
and goal/task helpers call Tilde.

## SDK framework adapters and the Python SDK

Framework adapters keep the core SDK framework-free. Each adapter converts typed history
(`ctx.message.history()`) into the framework's message shape with the same rules: only the
acting agent's messages are assistant messages, incomplete messages are skipped, objectives,
goals and tasks render as live user text, attachments are hydrated through the scoped
attachment API into native image/file parts, completed attachment-free conversions are cached
per agent in bounded batches and reused only when id and role match. Each adapter exposes
`ctx.channel.current` as framework tools with provider schemas unchanged and forwards the
model's tool-call ID to audited channel execution. TypeScript: `sdk-vercel-ai-node`,
`sdk-langchain-node` (`@langchain/core` 1.x), `sdk-openai-agents-node` (`@openai/agents`) and
`sdk-mastra-node` (`@mastra/core`, built on the Vercel adapter). Examples live in
`sdk/ts/examples/*`.

Suspension contract (core `AgentContext`, both SDKs): an adapter that can stop its loop at a
step boundary and continue it from saved state calls `enableSuspension(snapshot)` /
`enable_suspension(snapshot)` before the loop runs; from then on its Tilde tool calls are
suspendable (a long one answers pending). At each step boundary (before every model call) it
checks `shouldSuspend` / `should_suspend` (a call answered pending, or Tilde's deadline asked)
and calls `suspend(state)` with the conversation in its framework's message format, pending
answers included; the core adds the pending calls (model tool-call ID and Tilde's call ID) and
ends the invocation as a stop. A loop that ends with calls pending is suspended with its last
snapshot; one asked to suspend that reaches no boundary within a minute likewise. On the run's
next invocation the core reads the checkpoint, and `takeResumed()` / `take_resumed()` hands out
the state and each pending call's outcome by the model's tool-call ID once (a second loop in the
invocation starts from its own messages); at its first step boundary the adapter replaces
the call's own messages with the saved ones, the outcomes in place of the pending answers, and
continues. Agent code does not change. Only calls the adapter's tool wrappers make are
suspendable (`ToolExecution.suspendable`, Python `execute(..., suspendable=True)`); the code's own
calls always wait. The SDK's older `checkpoint` hook is for code that keeps framework state
itself and is not called when an adapter suspends. Per adapter: Vercel AI SDK, `prepareTildeCall`'s
`prepareStep` (model messages, binary data as base64; `onStepFinish`, from `prepareTildeCall` or
`tildeAiSdk`, records each step's response, so a loop that stops on a step whose calls answered
pending saves that step; `tildeConversation(ctx)` is the conversation the last loop ended with); Mastra, `tildeMastra`'s input processor (its message list, ended as a tripwire; a
loop that stops at `maxSteps` on a pending step saves the boundary before it, so that step runs
again); LangChain, `tildeMiddleware`'s `beforeModel` (stored
messages; restored through `RemoveMessage(REMOVE_ALL_MESSAGES)`); OpenAI Agents,
`callModelInputFilter` (input items); Python LangChain `abefore_model`, OpenAI Agents
`call_model_input_filter`, Pydantic AI `before_model_request` (`ModelMessagesTypeAdapter`;
Pydantic AI keeps the hook's messages as the run's history). Agno has no per-step message hook
and CrewAI runs its loop on worker threads with no hook that can replace its messages: neither
suspends, so their tool calls block as before and a deadline ends the invocation (the run
waits without a checkpoint).

`sdk/py` is a uv workspace. `trytilde` (import `tilde`) is the Python core: generated Connect
stubs (`connectrpc` + Google protobuf over `pyqwest` HTTP/2) under the proto package names,
`connect_agent` (dial-in Watch with heartbeat readiness from an optional `ready` hook, controls
subscription, tool catalog, ordered `RunService.Report`), `run_connected_agent` for standalone
scripts, `create_lambda_handler`, `AgentContext` with the same goals/tasks/agents/attachments/channel
namespaces, cooperative asyncio cancellation (`StopLoop` is a `BaseException`), per-invocation
OTLP trace and log export, `ManagementClient`/`RuntimeClient`, `tilde.chat_proxy` (pure ASGI
port of `@trytilde/chat-proxy`) and `tilde.fastapi` (`agent_lifespan` to dial in from a FastAPI app, `mount_chat_proxy`).
Adapters: `trytilde-langchain` (langchain-core 1.x; tool-call id from the LangChain
`ToolCall`), `trytilde-pydantic-ai` (`Tool.from_schema`, `RunContext.tool_call_id`, message ids
in model-invisible `metadata`), `trytilde-openai-agents` (`FunctionTool` non-strict schemas,
`ToolContext.tool_call_id`), `trytilde-agno` (`Function` with `skip_entrypoint_processing`,
injected `FunctionCall.call_id`; history passed as `agent.arun(input=messages)`), `trytilde-crewai`
(role/content dicts for `Agent.kickoff_async`; tools carry the provider JSON schema through a
field-less pydantic model and run back on the invocation's event loop because CrewAI executes
tools on worker threads; CrewAI forces strict schemas and snake_case tool names and does not
expose the model's tool-call id, so the core generates audit ids). CrewAI pins `openai<3`
while the OpenAI Agents SDK needs `openai>=3`: they are declared as uv conflicts and CrewAI is
synced into its own environment (`sdk/py/.venv-crewai`, see `sdk/py/scripts/sync.sh`). Examples live
in `sdk/py/examples`. `REGISTER_DEV_AGENTS=1 task dev` registers and starts one example per
adapter (stable IDs, one Gateway deployment each, optional `DEV_AGENTS` key filter); Python
examples run with the workspace interpreter directly so stopping the launcher never orphans
them. Python packages share the Changie release version; contracts are generated
by `sdk/py/scripts/generate.py` and not committed.

`serve({ run })` (Python `serve(run=...)`) hosts an agent wherever its entry module runs, so one
entry serves every deploy target: inside AWS Lambda (`AWS_LAMBDA_FUNCTION_NAME`) it returns the
Lambda handler (`export const handler = serve({ run })`); with a deployment token (option or
`TILDE_DEPLOYMENT_TOKEN`) it starts the dial-in host (`connectAgent`) and closes it on SIGINT or
SIGTERM, letting active invocations report before exiting; otherwise (declaration discovery,
tests, a plain import) it does nothing and the returned handler throws if called. Python's blocks
until a signal, so it refuses to run inside an event loop (`connect_agent` is for that).

## Generated contracts

Open-source Tilde only; Tilde Cloud compiles the generated trees into the engine crate
(`crates/tilde/src/generated`) and has no CLI crate.

`crates/tilde-contracts` holds the Rust Protobuf messages (`proto`) and ConnectRPC service
definitions (`services`) generated from `proto/` by `buf.gen.yaml`; `task generate` rewrites both
trees and nothing in them is hand-written. It is its own crate so the engine and the `tilde` CLI
build against one set of contracts: the engine re-exports it as `crate::proto` and
`crate::services`, which keeps every existing path working, and the message module is named
`proto` inside the crate because the service generator refers to it as `crate::proto`
(`buffa_module`). `buffa`, `buffa-types` and `connectrpc` are workspace dependencies so the
generated code and the engine cannot drift apart on the codegen runtime.

Moving the contracts out made conversions between a query row and a contract type orphan impls,
since both sides are now foreign; `queued_input` in the Tilde chat provider is a free function
for that reason rather than a `From`. Pulling the crate in costs about a minute of compile time
on a cold build, which is why the CLI depends on it rather than the engine.

## The tilde CLI

Open-source Tilde only (`crates/tilde-cli`); Tilde Cloud's users install the same released CLI.

`crates/tilde-cli` is the user-facing `tilde` command: `dev`, `deploy`, `doctor` and `local`
(connecting a harness on this machine to its local agent; see Local agents). The engine
crate already owns the binary name `tilde` and two workspace packages producing one binary name
silently overwrite each other in `target/`, so the in-repo target is `tilde-cli` and every
distribution channel installs it as `tilde`. It calls the management API through the
generated clients in `tilde-contracts`, the same crate the engine serves from, so the wire
shapes are checked by the compiler instead of being written out by hand; the default protobuf
codec is used, and `reqwest` remains only for the chat page's opaque body proxying.

Declarations are the split that shapes the whole design. Prompts, skills and bundled tools are
values in the agent's own modules, so only that language's runtime can read them: each SDK
exposes a declarations-only entrypoint (`tilde-declarations`, the `tilde-declarations` bin of
`@trytilde/sdk` and the console script of `trytilde`, also `python -m tilde declarations`) that
imports the entry with `TILDE_DISCOVERY=1` and prints `DeploymentDeclarations` as protobuf JSON.
The CLI resolves that reader by looking for it (`node_modules/.bin`, then the installed module;
`VIRTUAL_ENV`, then the nearest `.venv`, which in a uv workspace sits above the project) rather
than by running something and reading an exit code, so "the SDK is not installed" stays
distinguishable from "the declarations are broken". Everything after discovery — turning inline
`data` into content-addressed uploads and registering the deployment — lives in the CLI, so
registration has one implementation instead of one per SDK. The reader's protobuf JSON
deserialises straight into `DeploymentDeclarations`, so the CLI never restates those fields. `--allow-empty` lets `dev` run an
agent that declares nothing, while `deploy` still refuses an empty release.

A Hosted deployment (`deploy: { target: "hosted" }`, or `--target hosted`; Tilde Cloud only) is code
Tilde runs itself. `tilde deploy` bundles the agent's code with esbuild into the layout the SDK's
Lambda handler runs from (`dist/index.mjs`, which must export `handler = createLambdaHandler(...)`,
with text imports of prompts inlined and the project's own files followed; `prompts/` and
`skills/` beside it; a `package.json` with the SDK's version), folds the bundle's digest into the external ID so changed
code is a new deployment, and once the deployment is registered publishes the bundle
(`PublishHostedDeployment`). An open-source Tilde server refuses to publish.
The zip is reproducible (entries in path order, one fixed time), so the same sources give the same
digest.

`tilde build` is the same build without the deploy, for CI that publishes a prebuilt agent (Tilde
Cloud's Factory Manager). It picks the agent as `tilde deploy` finds them (the entry's specs, one or
`--agent <key>`, each agent's code in `agents/<key>/` or its `code`; an entry without specs with
`--target`), contacts no server, and writes `declarations.json` (the code's declarations as the
reader prints them), `agent.json` (the agent's `AgentSpec` from the Apply request `tilde deploy`
would send, external ID included), a Hosted agent's `bundle.zip`, and with `--version`
`manifest.json` (`{ version, sha256, declarations }`, what Tilde Cloud installs a published bundle
from).

`tilde dev` resolves the agent by exact name (or `--agent-id`) and fails with the gateway's
`/agent/new` link when there is none: it never creates one, because an agent without
capabilities, inference and channel access looks registered and then fails the first invocation
with `permission_denied`. It registers a deployment whose external id carries the declaration digest, and reuses that deployment on an unchanged restart; because
registration returns a token only on creation, it then mints one with `IssueDeploymentToken`,
which invalidates the previous token, so two concurrent `dev` sessions on identical declarations
fight over it. It starts the agent with `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN`, polls
the mtimes of the files the declarations came from, and on a change re-reads them, registers the
next deployment when the digest moved (a deployment's contents are immutable) and restarts the
agent. Ctrl-C and SIGTERM both stop the child, or it outlives `dev` and keeps holding its token.

The local chat page is a single embedded HTML file served with a reverse proxy to the agent's
Tilde chat ingress: the application key stays in the CLI exactly as a real application's server
keeps it, and the browser only talks to 127.0.0.1. The page polls `ListMessages` instead of
streaming `WatchThread`, which is enough for a dev loop and needs no Connect stream framing in
the browser. Every agent owns one built-in `tilde/application` connection whose channel is
private, so `dev` admits the single `dev` identity on it: a refused ingress call still records
the identity, which is what makes it addressable, and `SetIdentityAccess` then allows exactly
that one without touching the channel's mode.

Distribution is driven by `packaging/targets.json`, the single source of truth for each target's
Rust triple, release asset name, npm platform package and Python wheel tags. `.github/workflows/cli.yml`
cross-compiles every target (`cross` for the musl ones, because ring compiles C and assembly),
then `scripts/pack-cli.mjs` writes one `@trytilde/cli-<platform>` package per target and packs
`@trytilde/cli` with those as exact `optionalDependencies`, and `scripts/pack-cli.py` writes one
`trytilde-cli` wheel per tag carrying the binary in `.data/scripts` so pip installs the real
executable with no Python wrapper. The platform packages are generated at pack time, never
committed, so installing this repository never has to resolve a version that is not published
yet. `install.sh` downloads the release archive and verifies it against the release's
`checksums.txt`. Both SDKs declare the CLI at their own version — the TypeScript SDK as a peer
dependency, because package managers install peers at the project root where `npx tilde`
resolves, while a transitive dependency's bin is not linked there. `sdk/py/packages/tilde-cli`
is a development stand-in that runs the binary built by cargo so `uv` can resolve
`trytilde-cli` inside this repository; `task sdk:py:pack` skips it.

Sandbox egress wiring (the process side, `tilde sandbox`): the registration that issues the
session also issues an egress token, kept in memory the same way, for the gateway's egress route.
Each registration's `egress_env` joins the blueprint's environment with `{token}` replaced
(variables naming the token are skipped while the process has none), and its `git` rewrites
replace a marked block in `/etc/gitconfig` (`url.<url>.insteadOf`, and for the gateway origin an
empty helper followed by `!tilde sandbox credential`); no rewrites removes the block.
`tilde sandbox credential get` answers git from the loopback API (`/v1/egress/credential`) only
for the gateway's protocol and host, as `x-access-token` and the egress token.

## Tracing

Tilde is its own trace store. Spans live in one append-only ClickHouse table,
`otel_traces`, in Rotel's OpenTelemetry layout with Tilde's facts materialized from span
attributes: agent, invocation and thread ownership, and the observation type, level and
model. A trace is simply its spans, and session and ownership facts ride on every row, so
reads never join. Retried deliveries of a span collapse on the sorting key. History is kept
for `ENGINE_CLICKHOUSE_RETENTION_DAYS`, or indefinitely when unset; the same setting governs
logs and metrics. Absent configuration disables the viewer and discards valid,
authenticated OTLP uploads. A configured outage is unavailable, not disabled.

Observation: a span as the viewer presents it. The gateway projects `tilde.observation.*`
attributes once, before queueing: type (generation, tool, chain, agent or span), model,
input, output, level and completion start time. A session is a chat thread: spans carry
`tilde.thread.id`, and a snapshot of session facts under `tilde.session.*` captured at
acceptance, never identity display values or message bodies.

Observation cost: `crate::pricing` is the one place list prices become cost, for inference
requests captured at the gateway and for generation spans captured in traces, from the
`inference_prices` table keyed by Tilde provider id and model. A span names its provider
through `gen_ai.provider.name`, `gen_ai.system` or the AI SDK's `ai.model.provider`; without
one, whichever provider lists the model.

Media: a base64 data URI in any string attribute of a span becomes a
content-addressed object in the media bucket (`ENGINE_MEDIA_S3_BUCKET`,
`media/<agent>/<sha256>`) and a
`@@@tildeMedia:type=…|id=…|source=…@@@` token in its place; attributes still over 64 KiB
are stored whole (`payloads/<agent>/<sha256>`) and truncated on the span with an
`<attribute>_ref` naming the copy. This covers the projected `tilde.observation.*` fields
and the AI SDK or GenAI attributes they were copied from, so no full payload stays inline
when the bucket is configured; without it both stay inline. The media bucket is never a
queue bucket, so queue expiry cannot touch what stored spans reference.
Observations name their owning agent; `TracingService.GetTraceObjectUrl` signs a
ten-minute URL for an object whose key names that agent, which is how the inspector
resolves a cooperating agent's media inside a shared trace.

Metrics: `/v1/metrics` on the agent runtime listener accepts OTLP metrics under the same
credentials as logs. Ownership is stamped on every data point, batches queue in
`ENGINE_METRICS_S3_BUCKET`, Rotel writes them to `otel_metrics_<kind>` tables, and
`METRICS_OTLP_*` forwards them. Sidecars mount the same `/v1/metrics`, accept the
deployment token like logs, stamp ownership and relay Metrics frames. There is no metrics
viewer.

Telemetry queue: accepted trace, log and metric batches are written to object storage
(`ENGINE_TRACES_S3_BUCKET`, `ENGINE_LOGS_S3_BUCKET`, `ENGINE_METRICS_S3_BUCKET`) and a pointer row in Postgres
(`telemetry_objects`) before they are acknowledged. Each signal and destination is a queue,
such as `traces/clickhouse` or `logs/external`. Every enqueue writes its own object
(`<queue>/<agent>/<batch>/<enqueue>.pb`) while a unique `(queue, batch_id)` keeps a
retried upload one pending pointer. Admission takes a per-queue advisory lock around the
capacity read and pointer insert. Any gateway replica claims a pointer under a short
lease, delivers the object and removes the pointer; a failed or abandoned delivery
becomes due again (`Spool::drain`). Gateways hold no telemetry state. Delivery is at
least once and bounded per queue and per agent. The engine never deletes a queued
object; the bucket's lifecycle rule (at least one day) expires it. One sweeper
(`crate::telemetry::spool::sweep_worker`) drops pointers older than a day from every
queue. External OTLP forwarding
(`TRACES_OTLP_*`, `LOGS_OTLP_*`) is an independent queue, so a collector outage never
delays history.

Message and invocation rows retain W3C trace context across durable dispatch. Sidecars
accept scoped OTLP, stamp it with the verified invocation scope, and ship it to the
gateway as telemetry frames in their publish stream. The gateway re-stamps the agent from
the authenticated deployment. Storage credentials remain at the gateway. Invocation tokens
authorize agent uploads, including the terminal upload grace period; management tokens
cannot authorize ingestion.

The management-only TracingService reads the span table with a mandatory server-authored
agent predicate; every optional filter is a static parameter, never interpolated SQL.
Filters are conditions on named columns: string columns (name, type, level, model, session,
invocation, status message, input, output) with =, contains, does not contain, starts with,
ends with, any of and none of; number columns (latency, tokens, cost, time to first token)
with =, >, <, >=, <=; and `metadata` conditions on any span attribute. The web query
language expresses these as `field:value`, `field:*text*`, `-field:value`, `field:a|b` and
`field:>n` terms, with plain terms on the basic fields kept in their own URL parameters and
everything else in `where`. A complete trace, which may include
gateway and cooperating-agent spans, is returned only after proving one of its spans
belongs to the caller's agent. Cursors are keyset positions bound to their filter and time
window. Session summaries are page-local; the browser re-aggregates what it has loaded.

Sidecars retain raw OTLP in their bounded in-memory outbox until gateway queue
acceptance, preserve cached configuration through disconnection, and apply terminal
token grace.

Development starts ClickHouse and MinIO with Compose defaults and creates the avatar, log
and trace buckets.
The optional AI SDK dev agent emits model telemetry through Tilde.
The tracing activity chart aggregates the whole filtered time range in one span-store
query, with every filter including input and output search: counts, error counts, and
average latency per interval. Brushing updates the table URL time filter while preserving the chart horizon
and aggregates. Selection edges resize the filter; full-width selection restores
the original range. Explicit date controls replace the horizon. The chart never
aggregates only loaded table rows.

The Tracing tab loads server-filtered 50-observation cursor blocks on scroll and
virtualizes its rows. Filter changes cancel old requests; observation versions are
deduplicated before loaded-session totals are calculated. The compact IBM Plex
Mono toolbar edits existing typed filters with field:value autocomplete and chips
that expand above the table. Valid custom time ranges apply on change. A right
Sheet shows a Microcharts TraceFold waterfall with a shared 500ms grid and an
initially fitted time range. Horizontal pinch zoom preserves fixed row heights
and native vertical scrolling. Full-row hover previews restore the clicked
selection, with details in dense Input/Output/Metadata tabs. Full-screen inspection places the graph and
details in two columns. Session totals describe loaded
results rather than all stored history.


## IAM

- Principal: a registered agent acting under one invocation. In the open-source build management
  callers are not principals: it has no management users, sessions, API keys, roles or groups.
- Management API: the listener serving the browser UI and management RPCs. In the open-source
  build it is unauthenticated and every caller can do everything; operators put their own
  authenticating proxy in front of it. Lists return every row their filters keep, creation grants
  nothing and deletion purges nothing beyond the resource itself.
- Agent runtime API: a separate listener in the same process. It accepts only
  signed agent connect bearer tokens. It exposes agent registry operations,
  current-thread reads and invocation, and agent session callbacks. It has no
  connection administration or browser routes.
- Agent connect token: signed claims for one agent, invocation, run, thread and
  thread-specific participant, with a capability snapshot and a five-minute expiry.
  Ordinary RPC authorization checks the signature and expiry locally. Issued tokens
  remain authoritative until expiry, including after cancellation or permission changes.
  Issuance and renewal check the active run, invocation lease, agent, channel access and
  membership. Gateway renewal uses one query; sidecars check their local runtime state.
  Renewal intersects current grants with the token snapshot and never widens authority.
  The SDK reuses the token and renews at four minutes; denied renewal aborts execution.
  Expiring control streams reconnect using the latest token. Stop notifications still
  stop cooperative SDK work immediately. Data ownership and message-state constraints
  remain enforced by the operation itself. Trace ingestion retains its terminal window.
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
- Granting authority: creation grants no capabilities by default. In the open-source build
  management callers may set any capability. An agent setting grants at runtime needs explicit
  grant_capabilities authority over the target and cannot exceed its invocation capabilities.
  Endpoint changes also require authority at least as broad as the target agent's current
  capabilities.

The installation signing key for agent connect tokens is encrypted through the encryption
module.

`ENGINE_MANAGEMENT_ENABLED` and `ENGINE_WEB_ENABLED` independently control
management routes and React serving, defaulting to true. Packaged React assets share the
management bind; that listener is absent when neither management nor embedded web is enabled.
Development omits Vite when web is disabled.
The agent runtime and event ingress listeners and background workers remain active
in every mode. Event ingress defaults to `127.0.0.1:8082` and mounts only signed
provider webhook routes; management never mounts webhook ingress.
`ENGINE_PUBLIC_EVENT_INGRESS_LISTEN` and `ENGINE_PUBLIC_EVENT_INGRESS_PUBLIC_URL` control its
bind and advertised webhook origin independently of management.

Taskfile owns development startup, build/test sequencing and Cornucopia query generation.
Task loads `.env`, validates Rust configuration before starting services, then runs
the API and optional Vite concurrently with live interleaved output. A service
failure stops its siblings (fail-fast); Ctrl+C also stops all processes. `pnpm dev`/`build`
delegate to Task. Disposable Postgres lifecycle remains in a shell wrapper;
JavaScript remains for substantive generation checks and integration tests.

Dev dotenv files supply bind addresses, public URLs, Vite upstreams, browser
origins and S3 endpoints. `TS_ADDR=<IPv4> task dev` relocates local
service binds and URL hosts in `scripts/dev-env.py` before starting Task's services;
custom external URLs and database settings remain independent.
Without TS_ADDR, dotenv values pass through directly. `.env.example` lists local
defaults. Dev ngrok always overrides the ingress public URL.
Generated dotenv files remain private, ignored local files.

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
WhatsApp, Telnyx WhatsApp and Google Chat. They own dynamic tool definitions and provider wire
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

Chat's activity history captures immutable typed snapshots during mutations and
queues them in a bounded, process-local memory buffer. A background worker checks
that their transactions committed, encodes Protobuf snapshots and inserts batches.
Rolled-back mutations never become history. History is best-effort: crashes or buffer
overflow can lose queued events; canonical messages, tasks and tool-call records remain
durable. Flushers preserve the order of retained events within each thread and allow
gaps after a short wait when an earlier event was lost. Audit reads are eventually
consistent; graceful shutdown drains committed queued events where possible.
Messages, reasoning deltas, joins/leaves, message chunks, typing changes and tool
inputs/results/errors remain distinct. Compact chunk events avoid copying a growing
message on every token. ListActivity paginates the same cursor used by WatchThread.
Participants leave through an active flag, preserving authorship. Typing expires after
ten seconds with a queued not-typing event. Provider calls audit automatically, while
tilde.runtime.v1.ChatService.ReportToolCall and the SDK helper cover agent-local framework tools.
Interrupted calls become aborted and are not automatically repeated across unknown
external-effect outcomes. The Chat UI replays every category and offers raw event view.

Attachments have authenticated upload/download RPCs, metadata in canonical messages,
and encrypted Postgres bytes (128 MiB per file). Provider download references remain
private and are encrypted when they contain URLs; their bytes are fetched on demand,
then encrypted and cached. Public file fetches pin DNS and reject private destinations;
provider bearer credentials are restricted to provider-owned file origins. Email HTML
is rendered inside a script-free sandbox. Attachment-only messages still dispatch to
an agent. New input is scheduled as a fresh invocation whose immutable message
snapshot includes its attachments. Remote references require their source provider until first download.

CacheConvertedMessages and HydrateConvertedMessages retain the original batch cache
semantics. Values are explicitly opaque agent-converted JSON, separate from canonical
messages and audit snapshots, scoped by agent and message. Writes validate the entire
batch before committing, accept completed messages only, and cannot cross the invocation
thread. Incoming edits invalidate conversions. Runtime history includes that agent's cached conversions alongside canonical messages. The TypeScript SDK exposes cache, typing, attachment and local
tool-audit helpers alongside the generated clients over its shared HTTP/2 transport.
Conversion-cache writes validate and upsert a whole batch under the thread lock,
including last-value-wins duplicate IDs. Task creation inserts scoped dependencies
as a set. Complete gateway native sends commit the unique tool-call claim and completed
result together with the message, recipients and attachments in one transaction.
They queue the same typed tool-start/message/chunk/completion snapshots; failed
attempts roll back message writes before recording tool failure. Incremental input
and external provider effects retain a separately committed running claim and
terminal result. Durable tool-call claims prevent duplicate execution independently
of best-effort audit history.
Runtime message pages validate the cursor and include only that page's agent-specific
conversions in one query. Tool catalogs read native eligibility and ready assigned
connections together. Neither read needs an additional invocation-state query.

Development falls back to public `.env.example` defaults; private `.env` and
`.env.local` override them. Tests use `.env.test` and start local storage when
using its default endpoint.
The six original chat adapters have isolated outbound/inbound fixture coverage; Google Chat is
covered end to end (signed request, agent host wake, reply to a fake Chat API).

A Google Chat app (`google_chat`/`app`) is a Chat API app with an HTTP endpoint (not a
Workspace add-on) whose endpoint is the connection's webhook URL. Its credentials are the
JWT-bearer grant of the project's service account (`issuer` = client_email, `private_key`,
scope `chat.bot`), minting Chat API tokens on refresh, and the Cloud `project_number`: Google
signs each request with a JWT from `chat@system.gserviceaccount.com` whose audience is that
number ("Authentication Audience: Project Number"), verified against Chat's published
certificates. Google delivers space messages only when they @mention the app (or run its slash
commands) and every DM; the agent sees `argumentText`, the message without the mention. A DM is
one external thread (`spaces/AAA`), a space thread another (`spaces/AAA/threads/BBB`), which is
also the `conversation` its `sendMessage` tool replies to; `updateMessage` edits a sent
message (there is no automatic streaming edit, as with Slack). Senders are their Google email,
which Google vouches for, so the adapter attests identities: private access allows an added
email at once. The webhook answers `{}` (no synchronous reply). Attachments are not yet
ingested.


## RPC audience boundaries

Protobuf contracts are grouped by audience, then domain, under `proto/tilde/`:
`management/v1`, `runtime/v1`, `agent_host/v1`, `setup/v1`, `provider/v1`, and
`provider/tilde/v1` for the built-in Tilde chat provider.
Shared resource types live in `types/v1`. Management never imports runtime cache types.
Canonical Message has no conversion-cache field. Runtime ListMessages and agent-host
Invoke carry cached conversions separately; the SDK preserves batch cache/hydrate helpers.

Management and runtime expose separate AgentService definitions. Native chat has one
customer-facing ConnectRPC contract, `tilde.provider.tilde.v1.ChatService`, implemented
in `chat/providers/tilde/rpc.rs` and mounted only on agent-scoped ingress. Management
has no ChatService or duplicate conversation commands. It retains agent/provider
configuration, credential issuance, IAM and observability. SDK examples and integration
clients use `createTildeChatClient`; `ManagementClient.chat` has been removed.

The provider reuses the shared Chat application service and canonical types. Gateway
routes use `/agents/{agentId}/tilde.provider.tilde.v1.ChatService/{method}`; sidecars
serve the same contract. Agent/thread-scoped ingress credentials authorize requests;
management login sessions and invocation tokens cannot substitute for them. Runtime
ChatService remains invocation-scoped, inferring the current thread and acting
participant and keeping conversion caches private to the agent. Explicit other-agent
targets still require their existing capabilities.

Connection management and public setup commands are separate complete services. The
ConnectionSetupService and native OAuth callback are composed outside management login;
every setup command validates its connection setup token. Remote provider HandleSetup
belongs to provider/v1; agent_host/v1 holds only the `InvokeRequest` wake message. Invocation controls are outbound runtime subscriptions.
This changes protocol paths and generated clients, with no legacy namespace aliases.
It retains the single binary, common domain implementations and central database migrations.

## Embedded native chat

Native thread creation stays at the gateway. Subsequent work and mutations use the
thread's selected deployment, preserving weighted routing and existing pins. The chat-ui
package retains the recorded Transcript viewer alongside Chat/ListSessions; live styles
use the additional `tc-live` class so they do not override read-only transcript rendering.

The built-in Tilde provider is a catalog provider (`tilde`, connection type
`application`) like every other chat provider. Every agent owns exactly one Tilde
connection, created in the agent's creation transaction and deleted with the agent; `StartConnection`, `Disconnect` and `UnassignCapability`
refuse it, the web hides it from the add-provider picker, and startup backfills one for
agents that predate it. Its single sealed value, `api_key`, is what management
`TildeChatProviderService` reveals or rotates (rotation replaces the value, bumps the
credential version and drops cached credentials). Access policy is the ordinary
`connection_agents.access_mode` (private by default) and identities are ordinary
`chat_channel_identities` of that connection, managed through `AgentAccessService` from
the IAM tab. The Tilde adapter attests identities instead of verifying them: an asserted
identity is recorded and attested on first contact, unknown identities reaching a private
agent are refused until allowed, and `RequestIdentityVerification` records and allows an
identity at once (status `approved`). Ingress checks the key in constant time and admits
the identity with `chat_identity_allowed`, serialized on the agent's deployment-settings lock.

An application API key requires a trusted `x-tilde-identity` assertion (base64url UTF-8).
The native identity is an opaque application string scoped to the agent, mapped to a
canonical chat User. The server proxy resolves it from the host's authenticated session;
React identity props and browser-provided headers never authorize access. Gateway ingress
exchanges this pair for a short-lived signed user/agent scope when forwarding to sidecars.
Operator-issued ingress tokens remain privileged agent/thread credentials for internal
or administrative clients; embedding proxies accept only native application API keys.

Every identity-scoped operation checks active membership, including history, attachments,
search, subscriptions, and queue/run controls. Search and session listing filter membership
before pagination. Posting/typing can only use the caller's own participant; new sessions
contain that caller and the configured agent. Members may explicitly add/remove people.
Agent invocation/control targets remain bounded by the key's agent. Subscriptions recheck
membership and expiry before emitting activity. Rotation rejects new requests immediately;
already open streams expire within the internal token's fifteen-minute delivery window.

Session titles belong to the conversation; read state belongs to `(thread,user)`. Read
acknowledgements carry the last displayed activity sequence and cannot acknowledge future
events. Explicit unread marks do not affect another person's read state. Provider session
snapshots can include the first message page and an opaque replay cursor from the same
SQL snapshot, closing the bootstrap-to-subscription gap without duplicating partial text.

Queue listing/removal/reordering/steering act on the runtime's existing pending inputs.
Mutations serialize with consumption under the thread lock (and the sidecar lease).
Reordering places one item before another or at the end; steering interrupts the active
invocation and dispatches the selected item first under the agent's batching policy.
Removed queued messages remain visibly aborted in the transcript, but agent history excludes
removed and still-pending inputs. Sidecars publish typed queue snapshots into the durable
projection; hydration restores their order and resume publishes their new invocation IDs.

`@trytilde/chat-proxy` owns server credentials, trusted identity resolution, configured agent
selection, and streaming Connect forwarding. It allows only the provider contract and never
forwards browser cookies or credentials upstream. `@trytilde/chat-next` adapts Web requests
and async catch-all route params. `@trytilde/chat-ui` exports Chat and ListSessions, reusing
Dispatch's composer and markdown presentation with native history, search, read state,
attachments, live replay and queue controls. The browser receives no provider key. Components
sharing a proxy and UI identity invalidate each other's lists; unmount aborts streams and
revokes attachment object URLs. These packages build through `task build:chat`, including
SDK pack/publish workflows, and are exercised by `task test:chat-ui`.

## PostgreSQL notifications

Thread subscriptions use a shared PostgreSQL LISTEN connection per Chat service,
with transaction-commit NOTIFY wakeups from `chat_activity`. Readers subscribe
before loading history and drain all cursor pages before waiting. Notifications
contain no event data. Reconnect invalidates subscribers so they catch up from
persisted state without periodic subscription polling. Listener connections are
separate from the query pool and close with the service/pool. They connect to
`DATABASE_DIRECT_URL` (the server itself): `DATABASE_URL` may be a
transaction-pooling PgBouncer, which gives no session to LISTEN on.

## Connection pooling

`DATABASE_URL` is the query pool's address and may be a transaction-pooling PgBouncer;
`DATABASE_DIRECT_URL` is the server itself and defaults to `DATABASE_URL`. Only work that
needs a session uses it: LISTEN (one connection per listening service) and migrations (one, at
startup). Nothing on a pooled connection may rely on session state: settings are `SET LOCAL`
and advisory locks are transaction locks. Prepared statements are fine (PgBouncer's
`max_prepared_statements` re-prepares them on each server session). A PgBouncer that rejects a
`search_path` in the startup `options` needs a deployment that keeps Tilde in a schema of its
own to set it on the login role (`ALTER ROLE <login> SET search_path = <schema>, public`),
never in a pooled URL.
The query pool has 4 connections behind a pooler (`DATABASE_DIRECT_URL` set) and 16 on the
server itself.

Pending message routing and invocation dispatch wake on committed message/work
changes and on freed local execution capacity. Queued API events are dispatched
when their active invocation finishes, or replace it under interrupt policy. Trace forwarding wakes on queue changes, drains batches, and schedules
retries from persisted retry deadlines. Startup/reconnect reads recover missed
notifications. Health probes, lease heartbeats, expiry/retention cleanup and
failed-operation retries remain time-driven. The registry browser currently
refreshes every 30 seconds; it does not yet have a registry subscription API.

## Gateway warm cache

When the gateway claims an invocation it primes, in the background, the channel catalog
for that agent and thread and the decrypted credentials of every connection in it; the
wake's first history page is cached for that invocation as it is loaded. Subsequent
ListTools, InvokeToolStream and first-page ListMessages calls are answered from memory. The
catalog and credential caches empty on `tilde_sidecar_configuration`; history pages empty
on `tilde_chat_activity` and on any local message write to the thread; short TTLs bound
staleness and the invocation's end evicts its entries. Credentials live only in
`Connections`, as `SecretString` values that zeroize on eviction. Sidecars already hold
their threads and configuration in memory and need none of this.

## Secrets

A secret (`crate::secrets`, `SecretService`) is a named value a person sets in the app
(Connections → Secrets, `/connections/secrets`) and specs reference by name. Names follow
environment-variable rules; each secret has an optional description. The value is sealed by
the encryption crate (binding `secret`/id/`value`) when set and never returned: management lists
names, descriptions, timestamps and the blueprints using each. A sandbox blueprint's environment
variable holds its own sealed value or references a secret (`sandbox_blueprint_env.secret_id`);
a connecting sandbox process gets the secret's current value, so rotating it reaches new sandbox
connections without a deploy. A referenced secret cannot be deleted. An apply's blueprint env
takes `secret.ref("NAME")`; a missing one is a required action linking to
`/connections/secrets?add=NAME`, which opens a new row with the name filled in and the value
focused.

## Declarative apply

An agent's registry configuration can be written in code (`@trytilde/iac` in open-source Tilde's
SDK) and applied by `tilde deploy` through `ApplyService.Apply`, in every edition. A request
carries agent specs: the agent's own fields, inference aliases, tool sources, chat channels, a
sandbox blueprint (environment and egress grants), routines and the deployment the customer already deployed (a
Lambda ARN, a version-qualified one recommended, or a dial-in host). `crate::apply` runs it:

- Preflight reads only and lists every required action before anything is written, each with a
  link into the app: a missing connection (a pre-filled create link: provider, name, type), one
  that is not ready, of the wrong type or lacking a capability for its use, a tool or signal the
  connection does not offer (with the nearest name as a suggestion), an invalid spec, and
  missing permissions.
- Channels (`chat::access::apply`): each names a channel connection, assigned to the agent (a
  channel connection has one agent, so another agent's is refused in preflight), or without a
  connection the agent's own Tilde chat, and optionally its access mode (`SetChannelAccess`'s
  private, public or disabled; a new assignment starts in its provider's mode, GitHub's
  disabled). Preflight refuses a mode the provider does not support and two agents of the
  request claiming one connection. A channel left out stays; `delete` unassigns a connection,
  never the Tilde chat, and is a no-op for one gone or moved. Once the apply commits, runs on a
  channel whose access it tightened from identities no longer allowed are stopped.
- Then one transaction for every agent of the request, in dependency order: blueprints, then per
  agent its fields, aliases, tool sources, sandbox link, routines and deployment. A dry run runs
  the same writes and rolls back, so the plan is what an apply does.
- Upsert by key, whoever created the resource: an agent by `key` (`agents.key`, unique among live
  agents; a spec's fixed `id` takes over an existing agent without a key), a blueprint by name, a
  routine by name within its agent (`routines_agent_name`), a tool source by connection, an
  inference alias by alias, an egress grant by slug, a deployment by external ID.
- Nothing is removed for being absent from a spec; an entry with `delete: true` removes it (a
  blueprint environment variable too: variables a spec does not name, such as secrets set in the
  app, stay). A tool source's tools become exactly the spec's (`*` is every tool the connection
  offers). Absent capabilities, concurrency policy and tool mode keep the agent's own. A routine
  without `enabled` is created disabled and its enabled state is then left to people.
- Preflight also blocks an alias that would displace another alias on the same connection, a
  fixed ID that names an agent with a different key, and (cloud) capabilities beyond the caller's
  own reach, as CreateAgent and UpdateAgent do.
- Connections are only ever referenced by slug, never created by an apply.
- Specs are written with `@trytilde/iac` (`sdk/ts/packages/iac`): `defineAgent`, `defineProject`
  (agents applied together, and their team), `connection.ref("provider/name")`, `blueprint`,
  `egress`, `routine`, and deploy targets (`lambda(arn)`, `gateway()`, `sidecar()`). Values are
  plain data marked with `Symbol.for("tilde.iac")` and carry their `AgentSpec` protobuf JSON.
  Built-in catalog providers are typed (`github.ref("acme")`): `src/catalog.ts`, generated by
  `task iac:catalog` from a disposable server's catalog, holds one string-literal union of tool
  names (and one of connection types) per provider. `.tools(...)` takes only those names and `"*"`
  (`ALL`, every offered tool), so a typo does not compile; `.untypedTools(...)` takes any string,
  and both chain in any order. MCP-served tools take plain strings, which preflight checks.
  `connection.ref(slug)` is typed through the `Connections` interface of `@trytilde/iac` (empty
  in the package), which a project fills by module augmentation: `tilde iac types` lists the
  organization's ready connections (MCP servers and tool hosts' instances included) with their
  tools and writes a `.d.ts` (`declare module "@trytilde/iac"`) mapping each slug to its
  connection type and tool names.
  Connections that share a slug merge: one type keeps
  the union of their tools, different types (or unknown tools) leave that part plain strings. A
  generated slug's `.tools(...)` and `type` are checked at compile time; any other slug takes
  plain strings, which preflight checks. No enums or runtime name lists: `ALL` is the only runtime constant.
- Ontologies in a project (Tilde Cloud): `defineProject({ ontologies: { <name>: ontology({
  environments: { <env>: { connections, secrets, live } } }) } })`, each in the repository's
  `ontologies/<name>/` (`tilde deploy` runs at the repository root). A project is a pnpm
  workspace with one root lockfile and no `tilde.toml` (the root `tilde.config.ts` is the project); each ontology is the Tilde
  project of its name. `ontology<typeof connections>()` takes the ontology's own `connections` type
  (a type-only import), so binding a connection of another provider does not compile
  (`ConnectionRef`'s `provider`: a catalog ref's provider, a `connection.ref` slug's prefix);
  `secrets` binds an ontology secret name to a Tilde secret (`secret.ref`); `live` is whether
  Tilde serves the environment's spaces to the organization, set when the deploy creates the
  environment (no RPC changes it later: a difference from an existing environment is reported in
  the plan and left). `tilde deploy` deploys
  them before the agents (`tilde-cli` `ontology.rs`). Nothing changes until everything is
  checked: the ontologies' preflight (connections and secrets exist;
  each missing one is a required action with its link, the secret's naming the project's team; a
  server without the ontology control plane answers "Ontologies need Tilde Cloud") and the
  agents' dry-run `Apply`, reported together (exit 2). A first deploy needs nothing set up by
  hand: a project or environment Tilde does not have is part of the plan and the deploy creates
  it, the project (`ProjectService.CreateProject`) in the deploy's team (`--team` or
  `defineProject({ team })`, a slug resolved with `IamService.ListTeams`; none: the
  organization), each environment (`EnvironmentService.CreateEnvironment`, on Cloudflare) once
  the version is ready, with the storage its IR declares and the spec's `live`. Tilde compiles
  an ontology for its project, so a new project's ontology is built after the project is
  created rather than before anything changes, its tools and signals are not checked against the
  agents' specs on that first deploy, and `--check` refuses a project that does not exist yet. Then each ontology is built here, on the
  client (Tilde never builds a project), with the project's own `@trytilde/ontology` and
  `@trytilde/spaces` (Tilde Cloud's packages, run with `node`): its IR (`tilde-ontology-build ir`),
  sent to `VersionService.CompileOntology`, whose errors fail the deploy and whose contract is
  written into the spaces project as `@tilde/contract`; its handler module (`worker`); its
  spaces (`tilde-spaces-build build`); all as an artifact (a plain tar: `artifact.json` with the
  runtime the package declares, `ontology.ir.json`, `worker.js`, `spaces/`; uncommitted changes
  refused without `--allow-dirty`; over 32 MiB refused). Then, per ontology: a version
  (`VersionService.CreateVersion` with the artifact), a check that it declares exactly the
  connections and secrets the spec binds (else nothing is bound; a connection the ontology
  declares optional, `{ provider, optional: true }`, may stay unbound and is reported as
  skipped: its pipelines do not run until it is bound), then each environment bound and
  rolled out in turn (`--accept-rebuild <model>` passed to `Deploy`); a failed rollout stops and
  names the environments running the version, bound only, and untouched. `--only ontologies|agents` and
  `--ontology <name>` narrow it; `--dry-run` prints the plan. The ontology control plane is
  called as Connect JSON: open-source Tilde depends on no part of its contract. A binding is
  named by its `name`.
- An environment's own connection: Tilde Cloud makes one with every environment,
  `tilde_ontology/<project>-<environment>` (`tildeOntology.environment(project, environment)`
  in `@trytilde/iac`), which serves the ontology's tools and is the only connection that offers
  its signals (`tilde_ontology.<object id>.<created|updated|deleted>`, from the IR's
  `objects[].signals`; `@trytilde/ontology`'s `signal(Object, change)` is a `SignalRef` a
  routine's `signal.type` takes, whose data keys its filters). One `tilde deploy` therefore
  works for a project whose agents use its own ontology before the environment exists:
  `ontology::prepare` returns what the ontologies offer (`Offered`: the `(ontology,
  environment)` pairs the plan creates and, when every ontology of the project was compiled,
  their tools and declared signals), and every dry-run `Apply` before the ontologies execute
  (preflight's, and `--dry-run`'s) sends the pairs as `ApplyRequest.new_ontology_environments`,
  so the server takes a reference to such a connection as satisfied and its uses as written.
  The CLI checks those uses itself (`apply::with_ontologies`): every tool named on a created
  environment's connection, or on any `tilde_ontology/...` connection the server found
  wanting (`ToolNotOffered`, before the new version runs), against the compiled handlers; every
  routine signal on a created environment's connection, or on one of the project's environment
  connections the server found not offering it (`SignalNotOffered`: the running version lacks
  it), against the declared signals, refused with the object and what it declares. A
  connection the server vouched for keeps its word; without compiled ontologies (`--only
  agents`, `--ontology` choosing some, a new project) the server's findings stand. The apply
  after the rollout names no environments: by then they and their connections exist.
- `tilde deploy` runs the SDK's `tilde-declarations` reader with `TILDE_DISCOVERY_SPECS=1`; a
  reader that knows specs prints `{declarations, agents, code, team}` (validating each agent against
  `AgentSpec`; `code` maps an agent's key to its `defineAgent({ code })`, which specs don't
  carry), older readers ignore the variable. With agents, the CLI sends one `Apply`, in which
  every agent declaring `deploy` releases its own code: the reader runs once in that code
  (`code`, an entry module or a folder with its own manifest, else `agents/<key>/` from the
  project's directory) for its declarations (skill files inline), and the deployment gets an
  external ID (`--external-id`, the GitHub run, else a digest of what it deploys, so an unchanged
  rerun registers nothing). An agent with no code here (neither) has its code in another
  repository: its spec is applied without a deployment, with a note. From an agent's folder (the
  entry is code, not `tilde.config.ts`) the one deploying agent without code of its own releases
  the entry's declarations; two are refused. `--check` reads no agent's code. `--team`/`TILDE_TEAM`
  overrides the project's team; `--dry-run` asks for the plan. It prints the plan or every
  required action with its link (relative links joined to the server URL) on stderr, a newly
  registered dial-in deployment's token on stdout (alone when the request has one deploying
  agent, else a `<key> <token>` line each, however many were issued), or with
  `--json` the response. Exit codes: 0 applied or planned, 1 error, 2 blocked. Without agents it
  registers the deployment as before.
- The response lists one `Change` per resource (create, update with before and after per field,
  unchanged, remove; environment values sealed) and, for a registered dial-in deployment, its
  token once.

## Deployments, instances and the run protocol

An agent has many deployments. `agent_deployments` is the declared half of "what is
running": created by CI (`RegisterDeployment`, idempotent on a caller-supplied external
id), with a source and explicit type (`gateway`, `sidecar` or `lambda`), a required
function ARN for Lambda, repository and commit, plus what the
git provider knows about that commit (message, branch, author) so the Deployments tab
reads like a hosting console and links the commit back to GitHub. Registering a
deployment issues its token once; the token is the join between the record and whatever
runs. `agent_instances` is the observed half: every process that dials in with a
deployment token, heartbeating on a short cadence. Every instance belongs to exactly one
deployment, the gateway rejects registration otherwise, and an agent process run locally never
joins a deployed registry by itself (a local agent, see Local agents, has no deployments at all). Retiring a deployment invalidates its token. Policy
(capabilities, connections, IAM), routing and the sidecar failure policy stay on the
agent. Execution type belongs exclusively to each registered deployment: Gateway,
Sidecar, or Lambda. Gateway agent servers and sidecars dial in with deployment tokens;
deployments do not have a public URL. Lambda requires a function ARN. Agent creation never
creates a deployment; deployments are registered explicitly.

The deployments toolbar opens manual registration in a modal and reveals the returned
token once. Registered Gateway/Sidecar rows initially show `Offline (0)` until a ready
connection arrives; Lambda is eligible immediately. Token rotation and retirement
require explicit destructive confirmations in the UI; failed actions retain the dialog.

Routing is per agent: Latest prefers the newest registered deployment across all types
once available, falling back to an available deployment while it is offline;
Weighted distributes new conversations across registered deployments by integer
percentages totaling 100. A weighted deployment must have zero traffic before retirement.
PromoteDeployment allocates 100% to one deployment. Conversation routing locks preserve
one (thread, agent) deployment pin. Changing Latest or weights never changes that pin
while its deployment is registered. Gateways dispatch Gateway/Lambda invocations;
sidecar replicas retain their leases and handle Sidecar conversations. Sidecar projection
pins newly published membership to its lease owner's deployment. Failover stays within
the pinned deployment. Gateway-created threads and first provider events are canonical
before selecting an owner, so concurrent workers cannot select different execution types.

Execution is wake-and-dial. A wake carries the invocation (`InvokeRequest`, now with
the deployment id and trace context); how it is delivered is the only per-target
difference: an instance that dialed in through `tilde.run.v1.RunService.Watch`
receives it as a frame (a durable directive acknowledged by `Report`), and a Lambda
function is invoked asynchronously through
the AWS invoke API with the gateway's own credentials. Every host then reports through
`RunService.Report` with its invocation capability: acceptance, reasoning deltas and
the end of the invocation with any unconsumed inputs and, when the host's run threw,
the error, which the gateway records as a failed invocation rather than a stop. The
gateway uses the authenticated outbound connection for Gateway wakes.
The invocation lease is renewed by the host's open command stream and by reports, so a
host that dies stops renewing and expiry reclaims the run. Registry health is sampled
from deployment readiness; connected instances report health through heartbeats.

## Local agents

- Local agent: an agent that runs in a harness on a person's machine, registered with
  `CreateAgent`'s `harness` (a row of `local_agents`; the harness is fixed). Tilde never runs or
  wakes it, so it has no deployments (registering one is refused, and so is starting a run:
  nothing would serve it), and inference, prompts, chat providers, routines and capabilities do
  not apply. It is still an agent of the registry: it
  is listed with the others, owns its Tilde chat connection, and is given tool sources and
  skills like any agent. The health sweep leaves it unsampled (unknown): nothing is expected
  to serve it. Everything else is a managed agent.
- Harness: where a local agent runs: Claude Code, Codex, Claude Desktop or ChatGPT Desktop
  (`local::Harness`). Each harness's adapter declares the features it supports
  (`Harness::features`, returned as `Agent.local.features`): the coding harnesses keep
  transcripts on the machine and run hooks, so they report sessions (and traces) and take
  skills and tools; the desktop apps keep conversations in the vendor's account and take tools
  only. No harness reports logs yet. The agent's page shows a Setup tab and only the tabs its
  harness's features name.
- Harness credentials: the harness is one more application speaking for the agent, so it
  authenticates with the agent's application key (the `api_key` of its Tilde connection, rotated
  like any). The local endpoints answer only a local agent's key, at
  `/agents/{agent}/tilde.local.v1.LocalAgentService/{method}` and `/agents/{agent}/mcp`.
- Reported session: `LocalAgentService.Ingest` takes a harness session's events (what the
  person said, what the agent said, each finished tool call). A session is one native thread
  between the person and the agent (ids derived from the harness's session and event ids), its
  messages are canonical chat messages at the times they were said, and every event is also a
  span (`tilde.observation.*`, a turn being one trace), which is what the Sessions and Tracing
  tabs show; a prompt is a generation with an input and no output, because the session
  transcript reads messages from generations. Events are recorded once however often they are
  sent. Tool calls are spans only: `chat_tool_calls` records calls Tilde ran.
- Local MCP endpoint: the agent's tools on ready sources as an MCP server (streamable HTTP, JSON
  answers, no session), with catalog names made MCP-safe (`source_tool`). Adding a source is the
  grant; a call runs as the gateway runs an invocation's, with the agent as caller and no
  thread, and is not a `chat_tool_calls` record (the harness's own transcript reports it).
  Sandbox and personal tools are not served.
- Skills: `ListSkills` and `ReadSkillFile` return the skills the agent is given, for the
  harness's own skill folder.
- `tilde local` (the CLI): `install <harness>` stores the gateway URL, agent id and key in
  `~/.tilde/local/<harness>.json` (owner-readable), adds the harness's hooks (Claude Code
  `settings.json`; Codex `hooks.json`, which Codex runs once trusted) and registers `tilde local
  mcp <harness>`, a stdio bridge to the MCP endpoint, as the harness's MCP server (Claude
  Desktop: `claude_desktop_config.json`), so the key never enters the harness's configuration.
  `hook` runs on the harness's hook events: at a session's start it syncs the agent's skills
  into the harness's skill folder (each carries a `.tilde` version marker; skills a person put
  there are never touched), and when a turn or session ends it reads the transcript, parses it
  with the harness's adapter and reports what follows the session's cursor, which moves only
  once Tilde accepted it. ChatGPT Desktop takes remote MCP servers only: its Setup tab gives the
  endpoint to add in ChatGPT.
- Not built: logs from a harness's OpenTelemetry export; adapters that read a vendor account
  instead of a machine (claude.ai and ChatGPT conversations, through compliance APIs or
  exports), which would report through the same `Ingest`; policies that give many agents the
  same tools and skills (trytilde/tilde#43).

## Agent deployments and sidecars

Execution type is declared on registration, with no agent-wide mode switch or pause
required to release a different type. `tilde-sidecar` runs replicas that dial in with
a Sidecar deployment token. It accepts comma-separated tokens, one per agent, and keeps
no state on disk. The agent process beside it uses `connectAgent` with the local
sidecar runtime as its gateway URL; a sidecar wake fails until that process is connected. Public sidecar URLs are optional and serve only
provider webhooks and native ingress from outside. Replicas never call each other and the
gateway never calls a replica: work for a conversation another replica holds is sent to the
gateway (`Forward`, for ingress calls and provider events), which directs it to the holder
over that replica's own outbound Watch stream. Every internal connection is agent to gateway,
agent to its local sidecar, or sidecar to gateway.

Connect request bodies may be gzip-compressed. The Connect handlers decompress them; the chat
provider ingress paths (gateway guard, gateway ingress for sidecar agents, sidecar ingress and
the sidecar registry relay) buffer the body to authorize and route it first, so
`deployment::routing::plain_body` decompresses unary and enveloped streaming bodies once at
that edge and strips the encoding headers before anything is parsed or forwarded.

The gateway serves four route groups on one listener: management, runtime, ingress and
sidecar. A sidecar binds a loopback runtime listener for its agent process and one
network listener for provider webhooks and native ingress. Every other interaction is an
outbound connection from the sidecar to the gateway's sidecar group: a held `Watch`
stream that delivers a snapshot, configuration changes, lease changes and directives,
and `Publish` calls that carry heartbeats, typed events, directive results, lease
releases and telemetry. Central IAM, registry changes and credential setup stay at the
gateway. Sidecars enforce replicated permissions and serve assigned connection
credentials from memory.

Gateway and sidecar listeners enable `TCP_NODELAY` on accepted connections so small
RPC writes do not wait for TCP acknowledgements before being sent.

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
after the liveness window. Recovery wakes on registrations and on changes of an instance's
connection or readiness, never on a heartbeat that only refreshes `last_seen_at` (a NOTIFY
serialises its commit behind every notifying commit on the server); dead holders are found
on recovery's interval. Heartbeats commit without waiting for their WAL flush.

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
gateway-deployed agents in the same room are routed from the projection. Reads (thread,
run, message, activity and thread listings) are answered from the projection wherever they land,
since only the projection sees every replica's conversations; a replica forwards
listings and cursor pages, and serves the current window of a thread it holds.
Gateway provider WatchThread subscriptions read projected activity with opaque replay
cursors, so clients do not need a management API or direct sidecar URL for streaming.
Registry RPCs (agents, prompts, skills, and the memory tools the replica's tool registry
offers) from an agent beside a sidecar are verified by the replica, relayed over
`RelayStream`, and re-verified at the gateway: token signature, agent generation, the
thread lease, and the capability ceiling, before the existing registry handlers run.
The projection may lag a young invocation or roster, so only an invocation the gateway
already knows to be over is refused there.

Attachment bytes live in bounded sidecar memory until a separate worker uploads them
to the gateway, which encrypts them into S3; a slow object store never delays
heartbeats. Replicated metadata distinguishes temporary
availability from persistence, and persisted bytes can be downloaded through the
gateway by any replica. Health samples arrive with heartbeats.

## Sidecar telemetry

Telemetry is always on, so sidecar configuration carries no telemetry switches; a
configured sidecar relays every validated batch, and gateway outages keep batches in the
bounded sidecar outbox.
Gateway acceptance places them in the shared telemetry queue, where a batch is identified
by its content so reconnect replay is not queued twice. Platform spans use a process-wide
provider routed to the correct local agent, preserving invocation context and terminal
trace-token grace. No storage credentials reach agents or browsers.

`REGISTER_DEV_AGENTS=1 task dev` runs `vercel-ai-example-agent` as a separate SDK server. It is a private TypeScript member
of the SDK workspace, importing `@trytilde/sdk` normally and using Vercel AI SDK
`generateText` with its OpenAI provider. Dev startup launches its compiled `dist/index.js`.
The debug-only `dev-agent` command registers it through Agents, reusing its fixed
ID. Only thread reading, run updates and provider tools
are granted on creation. Registration does not alter assigned channels or existing
grants. The child receives the OpenAI key from private dev credentials,
its deployment token, gateway URL and model; database/encryption credentials are not forwarded.
Its model receives only the current channel tools and must call a provider tool for visible output.


The TypeScript core SDK exposes an invocation-scoped `ctx.message` facade.
`history()` reads the runtime's authoritative chronological page, derives roles
relative to the acting agent, includes per-agent cached representations, and
represents the current objective as a typed context item. `includeWork` adds
current goal/task state through the existing work.read permission; these are
context projections, not synthetic persisted chat messages. History pagination does not change the inbound channel binding. Delivery is explicit through provider-owned `ctx.channel` tools.

`@trytilde/sdk-vercel-ai-node` is the framework-specific adapter, matching the
Dispatch core/framework package split. `convertToAiSdkMessages` produces Vercel
UI messages for `convertToModelMessages`, with typed callbacks for conversation,
objective, goal and task items and an overridable attachment callback. Images
and PDFs become hydrated file parts; textual files become their actual contents.
Unsupported binary formats retain an explicit description and can be decoded by
a custom callback. Downloads use the invocation's authenticated attachment API.
Canonical completed text conversions may use the existing per-agent cache;
attachment bytes and changing work state are not duplicated into that cache.


Agent `concurrency_policy` is a persisted enum exposed in the Capabilities UI:
`queue` (the default) handles events individually after the active response;
`interrupt` revokes the old invocation and wakes its outbound stop control before dispatching fresh
work; `queue_and_batch` combines pending events into the next response. Scheduling
is per thread/agent. Gateway Postgres and the lease-owning sidecar runtime own
queue ordering, batching, retries and idempotency. Host receipts do not determine
which queued event is dispatched next. Every handler call receives a fresh invocation scope
and cancellation signal. A cancelled scope cannot use runtime tools even if the
external process ignores cancellation. History is fenced at the triggering event,
with the current invocation's own output still visible. Explicit StartRun/ResumeRun
remain durable-work operations; message scheduling never guesses which old goal to resume.


The SDK's `ctx.channel` surface groups only tools published for the invocation.
Built-in provider namespaces have typed callable arguments; dynamic customer
provider keys use `provider(id)` or `call_channel_tool(fullName, serializedArgs)`.
`current` is reserved for the inbound connection (or native conversation), and
never falls back to another enabled provider. Other authorized connections remain
available by provider or explicit connection ID. Tool descriptions can be customized
without changing schemas or authorization. `convertToAiSdkTools` preserves those
schemas, forwards model tool-call IDs, and honors cancellation. Model text is never
implicitly sent; the example exposes only `ctx.channel.current` to Vercel AI SDK.

## Agent log history

OTel logs use Rotel and the same verified invocation scope and terminal grace as
traces. ClickHouse owns log history in a standard OTel Map schema with explicit
agent, invocation, thread, and record identities. Gateway disk queues independently
buffer local history and optional external OTLP delivery; application Postgres
never stores bulk logs. Queue limits, 24-hour pending expiry, and seven-day history
retention bound resource use. Sidecars ship immutable log batches in their publish
stream until durable gateway acceptance and receive enablement only.

The management-only LogsService enforces agent scoping and fixed-window cursor
pagination and full-range count/error histograms. ClickHouse queries follow
HyperDX patterns for interval bucketing, automatic granularity, severity grouping,
and escaped ILIKE search, with Tilde’s mandatory agent scope and stable cursors. The Logs tab shares tracing’s
chip/date controls and resizable histogram selection, with a full-width Time/Level/Messages
virtual table, scroll pagination, right-side record inspection, and native trace
links, and a bounded live view. ClickHouse and the log, trace and metric buckets are
required: the engine refuses to start without them, and task dev always starts a local
ClickHouse from Compose defaults and .env overrides unless ENGINE_CLICKHOUSE_URL names one.
Telemetry intake and delivery use their own small Postgres pool, so a full or slow telemetry
queue cannot starve management requests or agent heartbeats of connections.


The TypeScript host supplies a logger-independent OTel `agentLogProcessor` with
per-invocation batching and renewed connect-token authentication. Ownership is
captured on emission; agent/thread(session)/run/invocation attributes and native
trace/span context travel together even when tracing is unsampled. Host completion
flushes final telemetry. The example's Pino instrumentation bridges into the same
OTel processor. Startup/background logs use the same collector with a deployment
token, supplied automatically by connected hosts and local example bootstrap.
The collector stamps `tilde.log.scope` as deployment or invocation. Deployment logs
carry verified agent/deployment IDs; invocation logs carry verified session/run/
invocation IDs. Reserved client ownership is replaced. The SDK keeps their bounded
queues separate and drops late closed-invocation logs without deployment fallback.
Log table and histogram filters expose scope alongside session and run UUIDs.


## PostgreSQL query ownership

Cornucopia validates the SQL contracts in `queries/` against the central migrations
and generates the committed `tilde-queries` crate. Domain `db` modules expose named,
typed operations over a generated `GenericClient`; services retain ownership of
transactions and lock ordering. Deadpool supplies cached statements and fast
connection recycling; tokio-postgres executes queries. The pool wrapper keeps its
configuration, identity, and shutdown signal together so separate databases and
test schemas never share LISTEN configuration or audit queues. Notification drivers
run continuously, reconnect on failure, and invalidate durable reads after reconnect.
The embedded migrator retains existing SHA-384 migration history and moves the old
ledger to `_tilde_migrations` atomically. Cancellation rolls back migration changes
and releases the transaction-scoped migration lock.

Trace provenance uses `tilde.client_sdk.version` from the installed TypeScript SDK,
`tilde.sidecar.version` stamped by a relaying sidecar, and `tilde.gateway.version`
stamped before the gateway delivery queue. SDK instrumentation stamps every SDK,
framework and RPC span it exports. Ownership normalization preserves only the
client-reported SDK version; server versions are replaced by the owning process.
They are stored with the span and shown in the trace inspector's metadata.
Gateway-only traces have no client SDK version; traces bypassing a sidecar omit its version.
Tracing and Logs share a dense square time-range dropdown with IBM Plex Mono text.

The Deployment tab places autosaving routing settings compactly inline,
with sidecar failure policy when applicable, above a full-width dense deployments
table immediately below the divider, without section headings, a serving summary,
or a refresh button. It reuses capability save feedback, locks controls while a change is pending,
and restores confirmed values on failure. A right-aligned toolbar button opens manual registration in a modal. Retire and
token rotation require destructive confirmations; issued tokens appear once in a dialog.
Log levels display warnings in bold yellow and errors/fatal levels in bold red.

The light/dark toggle is in the account menu (open source: the sidebar footer). The app retains the shared paper/espresso palettes, with a black main content card
and neutral charcoal tabs, table headers, inputs, hover states and portaled menus/dialogs
in dark mode. It defaults to the system theme on initial load and persists
an explicit choice in `tilde.theme`. The HTML shell applies that choice before first
paint so reloads retain the selected palette.

Weighted deployment routing replaces Manual. Active deployments are registered
records across Gateway, Sidecar and Lambda execution types.
Integer `traffic_weight` percentages total 100 across that set. Changing weights is
an atomic management operation with duplicate, scope, status, target and total
validation. New conversations sample the configured distribution; existing pins stay
on their registered deployment even when its weight becomes zero. Weighted targets
with traffic must be set to zero before retirement. New registrations receive zero
unless they are the first serving deployment. Selecting Weighted seeds 100% on the
current deployment; Latest resumes routing to the newest registered deployment.
The table exposes Traffic weight after Deployment only in Weighted mode. Edits show
a Save action in that column header; invalid totals open an error dialog, and valid
changes require confirmation. Canceling or failed saves preserves the draft.

`connectAgent` owns one outbound RunService connection and the host's invocation state,
using explicit options or TILDE_GATEWAY_URL/TILDE_DEPLOYMENT_TOKEN. It is the only host shape
besides the Lambda handler; there is no HTTP server, handler, signing key or Healthz RPC, and
readiness is the `ready` flag of each heartbeat (an optional `ready` hook). Gateway hosts point at the central runtime;
Sidecar-hosted agents point at their local runtime. Registration is followed immediately
by readiness heartbeat. There is no separate HTTP-only deployment category.

Local example bootstrap registers its own idempotent Gateway deployment and passes
that deployment's token to the HTTP server. It does not borrow credentials from the
agent's current serving deployment, which may now be a different execution type.

Deployment traffic eligibility is distinct from registration. Gateway and Sidecar
require an open gateway Watch generation, ready heartbeat within 15 seconds, and
an agent connection (for Sidecar, the local agent-to-sidecar stream). Lambda remains
eligible without a persistent connection. Stream cancellation clears its generation;
late cleanup cannot close a replacement stream, and heartbeat alone cannot reconnect
it. Allocation and promotion reject positive weights on offline deployments. New
routing skips disconnected releases; weighted selection uses the remaining eligible
positive weights. Existing conversation pins remain assigned and await their owner.
Readiness transitions wake queued messages. The deployments table polls availability
without discarding edited weights, and shows single-line Type and Serving (count)
cells, with plain-text deployment types and compact unique commit prefixes.

Registry health samples the agent routing policy every 30 seconds and stores one
aggregate observation per nondeleted agent, including failure when no deployment
can serve. Latest uses its available fallback. Weighted is green when all positive-weight
deployments are ready, orange (Degraded) when some are offline, and red when none
are ready. All traffic is redistributed across the remaining ready deployments. Gateway/Sidecar readiness requires a fresh heartbeat and the
full live connection chain. Lambda is eligible without a persistent connection; this
measures routing availability, not a probe of the Lambda function. Historical HTTP
probes and per-sidecar health events do not count as aggregate observations. Missing
observations remain unknown; an outage recorded in an hourly bucket stays red after
recovery. Healthy history bars are green in light and dark themes; degraded samples
produce orange hourly bars unless that hour also contains a full outage.

Deployment registration has no public URL in the UI, API or database. Gateway wakes
use the connected RunService stream exclusively, without an inbound HTTP fallback.
Instance URLs remain for sidecar ingress and forwarding. Agents have no endpoint at all.

The standalone `.github/actions/register-deployment` Node action registers CI releases
with an optional management token its caller provides; the open-source server ignores it. It
returns a masked same-job deployment-token output and an authenticated deployment-page
link. GitHub run IDs make reruns idempotent; missing tokens on retries fail unless the
caller explicitly opts into rotation. Manual users rotate in the UI to
get a replacement; original tokens cannot be retrieved from their stored hashes.
No bearer credentials are placed in job summaries, artifacts, or public links.

The registration action includes workflows with and without token output. Setting
`output-token: false` omits the secret output, discards the newly returned credential,
and permits idempotent retries without rotation. This mode cannot request token rotation.
Manual setup still uses the authenticated UI to issue a replacement credential.

Sessions is a dedicated agent header tab immediately after IAM, at
`/agent/:agentId/sessions`. It reuses the trace session grouping, filters, activity
chart and inspector. Tracing always shows observations; the old observations/sessions
filter selector and `view` search parameter are removed. Both routes remain full width.

The Sessions inspector renders trace payloads through @trytilde/chat-ui's read-only
Transcript display components brought forward from the chat-ui worktree. Model history
snapshots overlap by sequence, nested generation wrappers are omitted, and tool-call IDs
join calls with recorded results and instrumented tool spans. Recorded reasoning appears
as expandable text; uncaptured content is never inferred. Entry timestamps refer to the
source observation. Individual trace inspection retains its waterfall. Sessions can load
additional cursor pages; the view is not a replacement for the canonical chat store.

Transcript presentation uses Dispatch's original ConversationMessage, ToolChipsBlock and
TraceBlock components with scoped Dispatch chat CSS and palette tokens. Consecutive tool/reasoning entries share the original
compact expandable run UI; trace adapters supply full captured arguments and results.

The Sessions table is a view over existing traces. Before accepting a trace batch into
its existing durable delivery queue, the gateway captures provider/connection IDs and
branding, identity IDs, retained message count, last-turn time, and metadata capture time
for authenticated agent/session pairs. These are `tilde.session.*` span attributes. Identity
values and message bodies are not added by this enrichment. Reserved metadata is replaced
from trusted chat state. A batch is identified without its enrichment, so a retry is not
queued twice while the first copy is pending; once delivered, a retry is enriched afresh
and the newer snapshot wins.

Session summaries select one whole snapshot using its metadata capture time, never sum
counts across spans or use the largest count. Scrolling older pages cannot overwrite a
newer snapshot. PostgreSQL reads for the Sessions view only resolve identity IDs into
display labels; aggregation reads traces. Old traces without these fields stay unknown.
Counts describe the latest captured trace, so changes without a new trace do not update
them. There is no separate session ClickHouse table, projection worker, or session queue.
The cleanup migration removes the earlier experimental PostgreSQL projection objects;
the experimental ClickHouse table is removed with its one-time cleanup SQL.





## Liveness through proxies

Tilde Cloud is reached through a load balancer that closes a connection after 60 s without bytes
in either direction, and speaks HTTP/1.1 to Tilde, so HTTP/2 PINGs never cross it. Work that can
sit silent longer streams its own bytes (`rpc::with_keepalive`, a frame per heartbeat interval,
10 s): `InvokeToolStream` (chat and sandbox), the sidecar's `RelayStream`, `WatchThread` (an empty
frame repeating the cursor) and the other server streams' pings. A non-streamed inference call
still silent after 20 s is answered `200` at once with JSON whitespace every heartbeat until the
provider's body follows. Management RPCs that wait on outside work give up after 45 s
(`rpc::INLINE_LIMIT`) with their own error, or let it finish in the background (skill source
syncs).
