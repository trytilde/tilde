
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

Capabilities are `channel`, `inference`, `tool` and `signal`. A connection type declares which it offers;
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
  request SigV4-signed and the connection's default region in `_meta.AWS_REGION`.
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
  a server of a service Tilde already integrates (Slack, AgentMail, Stripe) joins that provider
  as `mcp_*` types. Any other server is added by URL as a registered (configured) provider of its
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
- Summary: a short label for a call, fixed on the tool-call record when the call starts
  (the agent's override, else the tool's own). Agent-local reports may supply their own.
- Annotations and output schema: optional hints on a tool definition (read-only, destructive,
  idempotent, open-world); hints for frameworks, never authorization.
- Built-in tools: offered only when the agent's existing capabilities make them usable.
  `agents.list` (agents.read), `agents.message` and `agents.wait` (agents.invoke: the target
  joins the thread if needed and runs with the message as its objective), `thread.participants`
  and `thread.search` (thread.read), `tools.result` (when the agent has async tools).
- Personal tool federation: a person connects their own account through a setup link the agent
  obtains with `user.connect_tool`; the connection is owned by that chat user
  (`connections.owner_user_id`) and can never be an agent's tool source. An agent holding
  `tools.personal` for the provider sees its tools as `user.{provider}.{account}.{tool}` only in
  invocations acting for that user, or for another identity under the same root. The user of an
  invocation is the run's source identity, else the thread's only active human; an unverified,
  unattested channel identity is a claim and unlocks nothing.

The invocation catalog lists the agent's tools on sources whose connection is ready or whose
host can take a call; invoke rechecks the same catalog, validates input against
the tool schema, resolves credentials and calls the provider or host. The catalog is built once
per invocation (first ListTools or InvokeTool) and held in memory until the invocation ends, at
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
call delivers that failure. Agent tools, tool hosts, built-in and personal tools
are gateway-only. Sidecar-hosted agents still see channel tools alone: serving them there
needs a provider-level relay to the gateway (credentials stay central) that executes without
auditing, because the replica holding the thread owns its tool-call records and activity order.

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
  secret) and Firecrawl connections (signed with the account's webhook secret) have no chat
  adapter and are verified by `signals::verify`. Their secrets are optional setup fields; without
  one, deliveries are refused. Signal-capable setups show the connection's webhook URL. Bot
  senders on GitHub and Slack emit nothing, so a routine cannot trigger itself.
- Routine: one agent, a name, a prompt, a thread title, an enabled flag (on at creation, toggled
  from the table) and exactly one trigger:
  a five-field cron schedule evaluated in UTC, or one signal type of one signal-capable
  connection. Prompt and title are `{{ key }}` templates: a signal's context is its data plus
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
  read_file, write_file, edit_file, apply_patch, list_dir, glob, grep), all enabled; they are
  configured like other agent tools, but the source goes only with the setting. Every invocation
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
  (`127.0.0.1:4790/v1/tools`) or `tilde sandbox call`, which forwards to `InvokeTool` with the
  session; tool hosts receive the call's `sandbox_id` (and the agent and thread when the reuse mode
  keys by them). Each call is an `execute_tool` span of the agent's invocation, like its other tool
  calls: commands an exec runs see the operation as `TILDE_SANDBOX_OPERATION`, which
  `tilde sandbox call` passes back, so the call nests under that `sandbox.exec`; a call outside any
  operation (a background job's) belongs to the sandbox's latest invocation. Modal snapshots are
  images kept for 31 days, past the longest terminate-after. The sweeper also renews a running
  E2B VM's TTL before E2B would pause it, and puts a Modal VM to sleep before its 24-hour
  lifetime ends; it never sleeps a sandbox with operations in flight, which keep it in use.

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
`InferenceUsage` frames, shed first under backpressure.

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
write/sync`. Sidecars relay the RPCs to the gateway.

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
authenticated layout, and public brokering lives outside it. Agent routes live in
`_app/agent/`, with the shared editor and its tabs grouped in `$agentId/`.
Standalone Connections and Chat pages are not exposed. Agent rows open `/agent/{id}/capabilities`;
`/agent/{id}/chat-providers` and `/agent/{id}/iam` are sibling tab routes under a shared
agent editor. Agent details are fetched by ID, so direct URLs and refreshes do not depend on previously loading the registry. The
shared editor preserves local state across tab navigation and browser Back/Forward. The dashboard header
shows Agent Registry / agent-name breadcrumbs, updated from loaded and saved route data.
Pages can contribute a DashboardNavigation slot beside the breadcrumbs; agent tabs use a
React portal so their panel context and keyboard behavior remain intact.
`/agent/new` creates an agent; the registry remains `/`. Connection brokering has a
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

SuspendInvocation transitions the run through `suspending`; the SDK checkpoint hook
must quiesce and persist framework state before returning. Execution then exits into
`waiting`. ResumeRun starts a new invocation and carries unconsumed input forward.
Framework code restores its checkpoint using the stable conversation ID. Hosts expose
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

Chat provider tools use an invocation-scoped ConnectRPC catalog and streaming
InvokeTool transport. The SDK registers their server-authored descriptions and
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

## Generated contracts

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

`crates/tilde-cli` is the user-facing `tilde` command: `dev`, `deploy` and `doctor`. The engine
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
service binds and URL hosts in `scripts/dev.py` before starting Task's services;
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
All six chat adapters have isolated outbound/inbound fixture coverage.


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
separate from the query pool and close with the service/pool.

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
ListTools, InvokeTool and first-page ListMessages calls are answered from memory. The
catalog and credential caches empty on `tilde_sidecar_configuration`; history pages empty
on `tilde_chat_activity` and on any local message write to the thread; short TTLs bound
staleness and the invocation's end evicts its entries. Credentials live only in
`Connections`, as `SecretString` values that zeroize on eviction. Sidecars already hold
their threads and configuration in memory and need none of this.

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
deployment, the gateway rejects registration otherwise, and local agents never federate
into a deployed registry. Retiring a deployment invalidates its token. Policy
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
gateway-deployed agents in the same room are routed from the projection. Reads (thread,
run, message, activity and thread listings) are answered from the projection wherever they land,
since only the projection sees every replica's conversations; a replica forwards
listings and cursor pages, and serves the current window of a thread it holds.
Gateway provider WatchThread subscriptions read projected activity with opaque replay
cursors, so clients do not need a management API or direct sidecar URL for streaming.
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

The sidebar logo row has a right-aligned light/dark toggle. The app retains the shared paper/espresso palettes, with a black main content card
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
