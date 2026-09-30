# Tilde TypeScript SDK

A separate pnpm workspace containing `@trytilde/sdk` and `example-agent`. The SDK
keeps Dispatch's grouped client and invocation-bound context pattern, but uses
ConnectRPC throughout. It does not expose REST clients, teams,
inboxes, inbox instances, or alternative agent response modes.

```sh
pnpm --dir sdk/ts install --frozen-lockfile
pnpm generate
pnpm --dir sdk/ts build
```

Protobuf contracts live in the repository root. Buf generates the Rust server and,
through the `@trytilde/contracts` package in this workspace, the TypeScript messages
and service descriptors that the SDK and the web app share. That package builds its
`gen/` directory from the protos and does not commit it. The SDK build emits
JavaScript and declarations into each package's `dist` directory.

## Run the example

Start Tilde with `task dev`, or the compiled `tilde` binary with the database and
encryption configuration from the root README. Tilde never calls an agent: the agent
host dials out to Tilde with a deployment token and receives wakes on that connection.
Create the agent and register a Gateway deployment for it from an application depending
on `@trytilde/sdk`:

```ts
import { createManagementClient, createTildeChatClient, management, BinaryPermission, TargetSelection } from '@trytilde/sdk';
import { randomUUID } from 'node:crypto';

// Open-source Tilde needs no credential; on Tilde Cloud pass accessToken: process.env.TILDE_API_KEY.
const tilde = createManagementClient({ baseUrl: 'http://127.0.0.1:8080' });
const { agent } = await tilde.agents.createAgent({
  capabilities: {
    toolsInvoke: { mode: TargetSelection.SELECTED, ids: ['sendMessage'] },
    workRead: BinaryPermission.YES,
    workWrite: BinaryPermission.YES,
    runUpdate: BinaryPermission.YES,
  },
  name: 'Example',
});
// The token is returned once. Start the example with it (see below) before posting.
const { token } = await tilde.deployments.registerDeployment({
  agentId: agent!.id,
  source: management.DeploymentSource.MANUAL,
  target: management.DeploymentTarget.GATEWAY,
});
console.log(token);
// Management provisions credentials; chat operations use the Tilde provider.
const { apiKey } = await tilde.tildeChat.getCredentials({ agentId: agent!.id });
const chat = createTildeChatClient({
  baseUrl: 'http://127.0.0.1:8080', // ingress origin when served separately
  agentId: agent!.id,
  apiKey,
  identity: 'alice', // Resolve from your authenticated application user on the server.
});
const { user } = await chat.getIdentity({});
const { thread } = await chat.createThread({
  title: 'Example thread',
  primaryAgentId: agent!.id,
  participants: [{ userId: user!.id }, { agentId: agent!.id }],
});
const participant = thread!.participants.find(p => p.userId === user!.id)!;
await chat.postMessage({
  id: randomUUID(), threadId: thread!.id, participantId: participant.id,
  text: 'Show me a streamed reply',
});
for await (const { activity, cursor } of chat.watchThread({ threadId: thread!.id })) {
  console.log(activity?.kind, activity?.textDelta, cursor);
}
```

Start the example with that token; it logs once the gateway has registered it:

```sh
export TILDE_GATEWAY_URL=http://127.0.0.1:8080
export TILDE_DEPLOYMENT_TOKEN=<token>
task agent:example
```

The example is deterministic and needs no model API key. Replace its `run`
function with your framework's reasoning/tool loop. It demonstrates goal/task
management, streamed visible output and explicit completion; it is not an LLM
implementation.

## Agent host

```ts
import { connectAgent } from '@trytilde/sdk';

// gatewayUrl and deploymentToken default to TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN.
const host = connectAgent({
  async run(context) {
    // Pass context.tools and context.signal into your framework.
    // Supply its ordinary model output to context.reason(), never auto-send it.
    await context.reason('Inspecting the request.');
    const goal = await context.goals.create({ objective: context.objective });
    const task = await context.tasks.create({ title: 'Reply', goalId: goal.id });
    await context.sendNativeMessage((async function* () {
      yield 'Hello ';
      yield 'there.';
    })());
    await context.tasks.update({ id: task.id, status: 'completed' });
    await context.goals.update({ id: goal.id, status: 'completed' });
    await context.setRunStatus('completed');
    context.stop();
  },
});
for (const signal of ['SIGINT', 'SIGTERM'] as const)
  process.once(signal, () => void host.close());
```

`context.tools` contains SDK-local `stop`, goal/task wrappers, and provider tools
fetched from invocation-scoped `tilde.runtime.v1.ChatService.ListTools` before `run` starts.
Descriptions, input schemas and optional chunk schemas come from the server.
Provider tools execute over the same reusable Connect transport as other callbacks.

Each provider owns its `sendMessage` tool's description, schema, formats and handler.
There is no common SendMessage RPC or mandatory sending trait method. Providers
publish canonical Chat message events explicitly; a tool result never does so.
Providers without sending capabilities expose no dummy sending tool. The
`context.sendNativeMessage` helper is restricted to the native-thread provider.
The provider interfaces are documented in `crates/tilde/src/chat/tools/`.

Tools receive the framework's `toolCallId`. Creation tools derive stable IDs from
that ID and the invocation. Framework integration should honor `context.signal`
and propagate the stop control exception; cooperative cancellation cannot forcibly
terminate arbitrary synchronous JavaScript. Returning a value from `run` never
publishes a message. Both normal return and `context.stop()` end the invocation;
unfinished runs become waiting unless the agent explicitly changed their status.

Message concurrency is configured on the agent: `queue` (default), `interrupt`,
or `queue_and_batch`. The API schedules fresh invocations and cancels interrupted
ones; handlers do not poll for steering or drain input queues. Pass `context.signal`
to model/tool calls. Each invocation's history is bounded by its triggering event,
plus messages produced by that invocation.
Use `chat.resumeRun` to explicitly resume a waiting/failed run. Do not guess which
of several waiting objectives a new human message should resume.

## Streaming and deployment

Provider tool `execute(input, { toolCallId })` sends completed input over the
client-streaming `tilde.runtime.v1.ChatService.InvokeTool` RPC. For incremental input, call
`tool.stream(input, chunks, { toolCallId })`; its chunk format is declared by the
provider's `chunkSchema`. Framework adapters must supply chunks explicitly:
registering a bundled tool does not automatically stream model tokens.

The agent's bundled tools are the tools it defines in its own code, written the framework's
way (`tool()` from `ai`, `@langchain/core/tools` or `@openai/agents`, Mastra's `createTool`).
Each framework adapter has one helper, `withTildeTools(ctx, tools, options?)`, returning the
framework's own tool collection: the current channel's tools, `context.agentTools` (tool
sources, Tilde's built-in tools such as `tools.search`, and personal tools) and the native
tools wrapped for auditing. It publishes the native tools to Tilde for the invocation, so
`tools.search`/`tools.schemas` describe them (with their summary, JSON schemas and annotations)
and a `tools.execute` naming one runs it in this process. Every call is audited once, with the
model's call ID and the tool's summary; `display` (`"full"`, `"summary"` or `"hidden"`) sets how
calls show in end-user chats, while traces keep full detail. The agent's Tools tab shows the
latest published set read-only. Tilde metadata comes from the framework's own metadata where it
has one (see each adapter's README) and `options`, keyed by tool name, overrides it.

```ts
import { tool } from "ai";
import { z } from "zod";
import { withTildeTools } from "@trytilde/sdk-vercel-ai-node";

const tools = await withTildeTools(context, {
  read_file: tool({
    description: "Read a file from the workspace.",
    inputSchema: z.object({ path: z.string() }),
    metadata: { tilde: { summary: "Read a file" } },
    execute: async ({ path }) => ({ text: await readFile(path, "utf8") }),
  }),
});
await generateText({ model, tools, messages });
```

```ts
const send = context.tools.sendMessage;
await send.execute(providerSpecificInput, { toolCallId });
// When this provider supports streaming:
await send.stream!(initialProviderInput, providerSpecificChunks, { toolCallId });
```

`sendNativeMessage(string)` and `sendNativeMessage(AsyncIterable<string>)` use that
same transport with the native provider's text/textDelta schema. The invocation
and framework tool-call ID determine a stable operation UUID. Frames are ordered
and require an explicit finish followed by EOF. Native partial text is observable
before completion; interrupted streams are aborted. Streams have a 1 MiB input
limit and a 30-second idle deadline. Provider handlers own idempotency and upstream
failure reconciliation; the transport does not automatically replay partial sends.

The SDK uses Node's HTTP/2 transport for every connection to Tilde. `connect-web` in a
browser is not an agent transport. A public deployment must preserve HTTP/2 between the
agent host and Tilde's runtime listener.

The outbound `Watch` connection is authenticated by the deployment token. Each wake
carries a signed five-minute connect token for callbacks, binding the invocation, agent,
run, thread, participant and permissions. It remains valid until expiry; inactive
invocations cannot renew it. Neither token appears in thread history.

## Validation

After building Rust and the SDK:

```sh
ENGINE_TEST_BINARY="$PWD/target/debug/tilde" \
  scripts/with-postgres.sh pnpm --dir sdk/ts test
```

This exercises actual Rust/Node ConnectRPC against disposable Postgres, including
partial streams, reasoning separation, scoped goals/tasks, dependencies,
stop/resume, steering, cancellation and all three thread topologies.

## Management access

Open-source Tilde's management API and web UI are unauthenticated: put your own proxy in front
of them. `accessToken` is therefore optional; it is sent as a bearer token when given, which
Tilde Cloud requires (a management API key):
`createManagementClient({baseUrl: managementUrl, accessToken})`. `ManagementClient` has no chat service. Native conversation operations
use `createTildeChatClient` (also exported from `@trytilde/sdk/tilde-chat`) and the
`tilde.provider.tilde.v1.ChatService` ConnectRPC contract, served at
`/agents/{agentId}/tilde.provider.tilde.v1.ChatService/{method}` on ingress.
Application chat uses an agent-scoped API key from the Chat Providers → Tilde screen
(or `tilde.tildeChat.getCredentials`) and a server-authenticated `identity`. Identity maps
to a canonical User within that agent; Tilde filters session lists/searches and checks
membership on every operation. `CreateUser` with an identity scope resolves the caller,
rather than allocating an unrelated identity. Keys can be rotated with
`tilde.tildeChat.rotateCredentials`. Management credentials and runtime tokens do not authorize
provider calls. `baseUrlOverride` replaces the complete provider service base URL.

`DeploymentService.IssueIngressToken` remains available to privileged operator/internal
clients, optionally restricted to a thread. These tokens are not for browser embedding:
they do not impose end-user identity scope. The embedding proxy accepts application keys
and independently derives identity from host authentication.

For React embedding use [`@trytilde/chat-ui`](packages/chat-ui/README.md),
[`@trytilde/chat-proxy`](packages/chat-proxy/README.md), and the
[`Next.js adapter`](packages/chat-next/README.md). Run `task build:chat` to build all three.
Provider activity uses opaque `afterCursor` / `cursor` receipts. Reconnect using the
last receipt rather than constructing a sequence number. Gateway subscriptions also
stream sidecar events after they reach the durable projection.

Agents receive signed connect tokens automatically during invocation and use
only the agent runtime API callback URL. The SDK reuses each five-minute token and
renews at four minutes while the invocation is live. A denied renewal stops execution;
control streams reconnect with the renewed token when the previous token expires.
Tokens and capabilities cannot be selected by the model.
Configure grants when creating or updating an agent; every capability defaults
to deny. For a native agent that sends messages and manages goals/tasks, grant
`toolsInvoke` (selected `sendMessage`), `workRead`, `workWrite` and `runUpdate`.

`ctx.agents.create/get/list/update/delete` call the registry using the current
invocation token. `ctx.invokeAgent({agentId, objective})` starts work for an
allowed agent already participating in the current thread. The server checks
all grants and target IDs; hiding tools is not the authorization mechanism.

## Agent tracing

`connectAgent` and `createLambdaHandler` initialize OTel tracing by default. It extracts the platform's
W3C context, records the invocation and callback RPCs, and uploads standard
OTLP/HTTP protobuf batches to Tilde with the current agent connect token. Spans
created by framework instrumentation inside `run` are collected under the same
invocation; prompts/tool inputs are recorded only if that instrumentation opts in.
Concurrent invocations have separate bounded export queues even when they share
a trace. Final uploads can continue after cancellation within Tilde's five-minute
telemetry grace window.

If the process already configures an OTel provider, add the exported
`agentSpanProcessor` to that provider's `spanProcessors` and set
`tracing: "existing"` in the host options. Initialize that provider's async
context manager before starting the agent. This avoids replacing the application's
provider while collecting its instrumented spans through Tilde.


## Agent logs

Agent hosts initialize the standard OTel Logs SDK by default. Emit through
`@opentelemetry/api-logs` or bridge your application's logger to OTel. The SDK
adapter is logger-independent: Pino, Winston, and direct OTel emitters all use the
same `agentLogProcessor`.

For an existing LoggerProvider, register `agentLogProcessor` in its `processors`
and set `logging: "existing"` in the agent host options. Set service identity on
your provider's Resource (the default provider honors `OTEL_SERVICE_NAME`).

```ts
import { logs } from "@opentelemetry/api-logs";
import { LoggerProvider } from "@opentelemetry/sdk-logs";
import { agentLogProcessor, connectAgent } from "@trytilde/sdk";

logs.setGlobalLoggerProvider(new LoggerProvider({ processors: [agentLogProcessor] }));
const logger = logs.getLogger("my-agent");
const host = connectAgent({
  logging: "existing",
  async run(ctx) {
    logger.emit({ body: "Agent work started", severityNumber: 9 });
    // Work inside this invocation's async context.
  },
});
```

The adapter captures invocation ownership when a record is emitted, before
batching. It adds `tilde.agent.id`, `tilde.thread.id` (session), `tilde.run.id`, and
`tilde.invocation.id`; standard OTel trace/span correlation and custom attributes
are preserved. Every invocation has its own bounded batch queue and current-token
supplier. Exports go to the invocation callback base plus `/v1/logs`; no static
authorization header or global endpoint override is needed. Final logs are flushed
before the invocation completes, and export failures do not fail agent work.
Logging works independently of trace sampling.

Startup/background logs use the same `/v1/logs` endpoint with a deployment token.
`connectAgent` configures this automatically from its gateway URL and deployment
token. Lambda hosts accept `deploymentLogging: { gatewayUrl, deploymentToken }`.
For logs emitted before constructing the host, call
`configureDeploymentLogging({ gatewayUrl, deploymentToken })` before emitting them
and register `agentLogProcessor` on your OTel LoggerProvider. The default processor
supports one deployment per process; conflicting deployment configuration throws.
Flush your LoggerProvider on shutdown (`connectAgent.close()` flushes automatically).
Without deployment configuration, only invocation logs are exported.

The collector stamps `tilde.log.scope=deployment` plus verified `tilde.agent.id` and
`tilde.deployment.id` for deployment-token uploads. Invocation-token uploads get
`tilde.log.scope=invocation` and their verified session/run/invocation IDs. All
client-supplied `tilde.*` ownership is replaced. Deployment logs have no fabricated
execution IDs. Use `scope:deployment` or `scope:invocation` in Logs to filter both
the table and histogram. Closed invocation contexts never fall back to deployment
authentication. Rotated tokens and retired deployments lose gateway upload access.
Do not put per-run IDs on the process-wide OTel Resource.

The local example uses Pino with `@opentelemetry/instrumentation-pino`, loaded
before Pino itself. Its log-sending bridge runs in the same thread so it retains
OTel async context. The Pino dependency belongs to the example, not the SDK.
Use `task test:sdk:logging` to exercise both ordinary OTel and Pino exports.


Management clients use `createManagementClient(...)`; native chat clients use
`createTildeChatClient(...)`; programmatic invocation clients use `createRuntimeClient(...)`.
Generated contracts are exported separately from `@trytilde/sdk/management`,
`@trytilde/sdk/tilde-chat`, `@trytilde/sdk/runtime`, and `@trytilde/sdk/agent-host`.
Runtime thread reads, attachment operations and typing derive scope from the connect token.
`AgentContext.getMessages()` returns canonical `messages` and a separate `cachedMessages`
collection. Initial conversions are available through `AgentContext.cachedMessages`.
Canonical messages have no cache field, including those returned to provider callers.

## Agent host lifecycle and invocation controls

Tilde never calls an agent host. A long-running host dials out with `connectAgent`;
Lambda deployments use `createLambdaHandler`, where the cloud invoke API delivers the
wake. There is no inbound agent endpoint, signing key or health route.

Each running invocation opens `InvocationControlService.WatchCommands` to its
callback URL before user code starts. Incoming messages and explicit steering are
scheduled by the agent's queue, interrupt or batch policy. Stop aborts its signal. Suspend runs the optional
checkpoint hook, acknowledges it, and ends the request; resume is a fresh invocation
that restores framework state. Already-persisted conversation state remains in Tilde;
the hook owns any additional framework checkpoint. It must not merely copy mutable
state while the framework continues executing.

Queued inputs remain at the runtime and are deduplicated by input ID.
Control connections use the current renewed invocation token. A sustained connection
failure aborts the local execution. Cancellation is cooperative: custom code must
honor the signal.

A wake is not a result channel: every host reports acceptance, reasoning and the end of
the run through `tilde.run.v1.RunService.Report`; see "Connected hosts".

## Connected hosts

A long-running process dials in with a deployment token (from the Deployments tab or `RegisterDeployment`) and
receives wakes as frames on `tilde.run.v1.RunService.Watch`:

```ts
import { connectAgent } from '@trytilde/sdk';

const host = connectAgent({
  gatewayUrl: process.env.TILDE_GATEWAY_URL, // the runtime listener root; this is the default
  deploymentToken: process.env.TILDE_DEPLOYMENT_TOKEN, // likewise the default
  instanceId: process.env.HOSTNAME, // optional; defaults to a random UUID
  async run(context) {
    // Your reasoning/tool loop.
  },
  async checkpoint(context) {
    // Optional: persist framework state before suspension.
  },
  async ready(signal) {
    // Optional: dependency checks. false or a throw reports this instance as not ready.
    return true;
  },
});
for (const signal of ['SIGINT', 'SIGTERM'] as const)
  process.once(signal, () => void host.close());
```

`connectAgent` opens `Watch` with `authorization: Bearer <deploymentToken>` and the
instance ID, records the `registered` frame (`host.registration`, `onRegistered`),
runs each `wake` frame concurrently (virtual-thread exclusivity, controls subscription,
checkpoint/suspension), and ignores pings. It throws when neither the options nor
`TILDE_GATEWAY_URL`/`TILDE_DEPLOYMENT_TOKEN` supply the gateway URL and token. Point the
URL at the central gateway for Gateway deployments, or the local sidecar runtime for
Sidecar deployments.

Readiness is reported only through heartbeats: every 3 seconds, and once on registration,
the host sends `Heartbeat({ instanceId, ready })`, where `ready` is the result of the
optional `ready` hook (`true` without one, `false` when it throws). The host reconnects
after a 1-second backoff whenever the stream ends, until `close()` is called. `close()`
stops watching and heartbeating, stops active invocations and waits for their final
reports.

Every host, connected or Lambda, reports through `RunService.Report` on the
invocation's `callbackUrl` with `authorization: Bearer <capability>` (renewed like the
other runtime RPCs): `accepted { commandId }` once controls are ready, one
`reasoningDelta` per `context.reason()` call, and exactly one `stopped { pendingInputIds }`
when the invocation ends by return, stop or suspension. Report failures are logged and
never crash the host.

### AWS Lambda

`createLambdaHandler` runs one invocation per event, where the event is the
JSON-encoded `InvokeRequest` delivered by the cloud invoke API (which authenticates the
wake). It needs no AWS SDK and resolves once the invocation has ended and been reported:

```ts
import { createLambdaHandler } from '@trytilde/sdk';

export const handler = createLambdaHandler({
  async run(context) {
    // As above.
  },
});
```

## Message history and Vercel AI SDK

Use `ctx.message.history()` for invocation-scoped, paginated context and
`ctx.channel` for provider-owned delivery tools. Core SDK history contains
typed conversation, objective, goal and task items; work state is opt-in with
`includeWork` and uses the existing work.read permission.

`@trytilde/sdk-vercel-ai-node` owns `convertToAiSdkMessages`, default attachment
hydration, and callbacks for custom message/attachment rendering. Convert its UI
messages with Vercel's `convertToModelMessages` before inference. The core SDK has
no Vercel dependency. See the adapter's README and `sdk/ts/examples/vercel-ai-example-agent`.


## Other framework adapters

`@trytilde/sdk-langchain-node`, `@trytilde/sdk-openai-agents-node` and
`@trytilde/sdk-mastra-node` mirror the Vercel adapter for LangChain/LangGraph, the OpenAI
Agents SDK and Mastra: `convertTo<Framework>Messages` renders the same typed history (objectives,
goals, tasks, subjects, scoped attachment hydration, bounded per-agent caching with id/role
validation) into each framework's native message shape, and `convertTo<Framework>Tools`
exposes `ctx.channel.current` with provider descriptions and JSON schemas unchanged while
forwarding the model's tool-call ID to audited channel execution. LangChain reads the ID from the
`ToolCall` that `ToolNode`/`createAgent` pass to `invoke`, the OpenAI Agents SDK from
`details.toolCall.callId`, and Mastra from `context.agent.toolCallId`. OpenAI Agents items carry
no Tilde `id` because the Responses API rejects foreign ids; the id lives only in the cached
representation. Each adapter has a private example under `sdk/ts/examples/<framework>-example-agent` that
mirrors `sdk/ts/examples/vercel-ai-example-agent`.

## Python SDK

`sdk/py` is a uv workspace with the Python counterpart of this SDK: `trytilde` (import `tilde`)
hosts agents over ConnectRPC with the same wake, controls, tools, history, reports, tracing and
logging behaviour, mounts into FastAPI, and includes the chat proxy; `trytilde-langchain`,
`trytilde-pydantic-ai`, `trytilde-openai-agents` and `trytilde-agno` are the framework
adapters, each with an example agent under `sdk/py/examples`. See `sdk/py/README.md`.

`ctx.channel.current` contains only the inbound connection's available tools.
Built-ins have typed namespaces (`slack`, `github`, `agentmail`, `linq`, `whatsapp`,
`telnyxWhatsapp`, `native`), with explicit connection selection when ambiguous.
Call a tool directly, e.g. `ctx.channel.slack.sendMessage({ channelId, text })`,
after checking it is available. Custom tools use `call_channel_tool(name, json)`
or `channel.provider(providerId)`. `convertToAiSdkTools` in the Vercel adapter
turns these provider descriptors into model tools and permits instruction overrides.
There is no generic `message.reply` projection of returned model text.

## Prompts and skills

Prompts and skills ship with a deployment. `tilde deploy` (the `tilde` bin of
`@trytilde/sdk`) imports the agent's entry module with `TILDE_DISCOVERY=1`, under which
`connectAgent` and `createLambdaHandler` start nothing, and registers what its exports
declare: `definePrompt`, `defineSkills` and `defineSkill` values, and framework objects an
installed adapter recognises (`@trytilde/sdk-mastra-node` reads Mastra agents' instructions and
skills, `@trytilde/sdk-vercel-ai-node` AI SDK `ToolLoopAgent` instructions). Exports are scanned
one level into plain objects and arrays. An adapter may also export `discoverProject(dir)` for
frameworks defined by files (Vercel's eve `agent/` folder), called for the entry's package root
and the working directory.

```ts
export const triage = definePrompt("triage", {
  template: "You triage requests for {{product}}.\n{{> tone}}",
  sections: { tone: "Be direct and warm." },
  config: { model: "gpt-5", temperature: 0.2 },
});
export const skills = defineSkills({ dir: new URL("../skills", import.meta.url) });
export const refunds = defineSkill({ name: "refunds", description: "…", instructions: "…" });
```

```sh
TOKEN=$(npx tilde deploy dist/index.js)             # TILDE_URL, TILDE_AGENT_ID (+ TILDE_API_KEY on Tilde Cloud)
npx tilde deploy dist/index.js --dry-run            # the declarations as JSON; contacts nothing
```

Flags: `[ENTRY] --agent-id --url --api-key --target gateway|sidecar|lambda --function-arn
--external-id --label --dry-run --json`. ENTRY defaults to package.json `main`. Point it at
built JavaScript; a `.ts` entry only loads when Node's type stripping can run it as is. Text
files up to 256 KiB travel inline, other skill files are uploaded first by digest. The
same name with different content is an error; stdout carries only the token (`--json`
prints `{deploymentId, token, created}`).

In `run(ctx)`, `ctx.prompt(triage).render(vars)` (or `triage.render(vars)` inside an
invocation) inlines sections, substitutes variables (a missing one throws) and marks the
version active: this invocation's inference calls then carry `x-tilde-prompt:
name@hash[, name@hash…]`, which the gateway strips and records. Pass `{ prompt }` to
`ctx.inference(slug, …)` to stamp one prompt, or `{ prompt: null }` for none.

Model clients built at module scope (framework agents) use `inference(slug)` from
`@trytilde/sdk` in place of `ctx.inference(slug)`: its requests go through the invocation
running when they are made, and fail outside one. `runWithContext(ctx, fn)` enters an
invocation for tests and custom hosts.

Skills are grouped into skill sources (a Tilde catalog group, a GitHub repository or an
editor collection) and an agent is given whole sources, single skills, or the skills linked
to a connection it holds. `ctx.skills.list()` returns each skill's name, source, description
and files; `read(name, path?)` returns a text file inline or a short-lived `downloadUrl` for
anything else (`source/name` when two sources share a name); `summary()` renders a block for
a system prompt and `materialize(dir)` writes every skill under `dir/<name>/` for harnesses
that load skills from disk. Each skill says whether the deployment shipped it (`deployed`, its
files already beside the code). `directory()` keeps the others, those assigned in the registry
that reach running deployments with no redeploy, in `$TILDE_SKILLS_DIR` (the OS temp directory by
default) `/tilde-skills/<agent id>/`: the engine pushes the invocation's skills and versions with
each wake, and only new and newer versions are downloaded and removed skills deleted; framework helpers hand it to the framework's
own skills. For frameworks without skills, `tools()` returns `list_skills` and `read_skill` as
Tilde tools to convert like channel tools, alongside `summary()` in the instructions. With `agents.read` these accept another agent's id. With
`agents.edit_skills` an agent lists every source (`sources()`) and assigns or unassigns a
source or skill (`assign({ source })`, `assign({ skill: "source/name" })`), itself included;
with `skills.edit` on an editor source it writes skills into it (`write(source, name, files)`)
and re-syncs git sources (`sync(source)`). `ctx.prompts.list(agentId?)` lists deployed
prompt histories.

## Deployment execution types

`RegisterDeployment` must specify `DeploymentTarget.GATEWAY`, `SIDECAR`, or `LAMBDA`.
Gateway agent servers and sidecar processes dial the gateway with their deployment
token. Deployments have no public URL: Gateway invocations travel over their
authenticated outbound connection.
Lambda requires `targetReference` containing a function ARN and does not accept a URL.

Execution type belongs to the deployment. Latest can switch from a sidecar release
to a Lambda or Gateway release without pausing or changing the agent. Weighted routing
can split new conversations among all registered types. Each conversation retains its
registered deployment pin; routing edits do not move existing conversations.

### Management identities

On Tilde Cloud, pass a server-side management key in the `accessToken` option. Provider IDs
are `provider/account-name`, scoped to the supplied agent's chat assignments.

```ts
import { ManagementClient, management } from "@trytilde/sdk";

const tilde = new ManagementClient({
  baseUrl: process.env.TILDE_URL!,
  accessToken: process.env.TILDE_API_KEY, // Tilde Cloud only
});

const { identity } = await tilde.identities.createIdentity({
  agentId,
  providerId: "agentmail/support",
  identityType: management.IdentityType.EMAIL,
  value: "dan@example.com",
  skipVerification: true, // Mark verified by management; does not grant agent access.
  createRoot: true,       // Optional; otherwise the root is absent by default.
});

const { identity: native } = await tilde.identities.createIdentity({
  identityType: management.IdentityType.USERNAME,
  value: "my-app-user-42", // Native Tilde subject; no provider connection needed.
});

await tilde.identities.linkIdentity({
  identityId: native!.id,
  rootIdentityId: identity!.rootIdentityId!,
});

await tilde.identities.unlinkIdentity({
  identityId: native!.id,
  rootIdentityId: identity!.rootIdentityId!, // Expected current association.
});
```

`createRoot` and `rootIdentityId` are mutually exclusive. Repeating creation for the
same provider subject reuses its identity and root. Creation without
`skipVerification` leaves the identity unverified; provider delivery is requested
separately through `access.requestIdentityVerification`. Linking never changes a
conversation's participant or address and cannot move an already-associated identity
without an explicit unlink. `createRootIdentity({ id, identityIds })` also supports
atomic grouping of existing identities, with a caller-generated UUID for retries.
