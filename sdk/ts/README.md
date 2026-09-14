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

Protobuf contracts live in the repository root. Buf generates the Rust server,
web client and SDK types without compiling Rust. The SDK build emits JavaScript
and declarations into each package's `dist` directory.

## Run the example

Start Tilde with `task dev`, or the compiled `tilde` binary with the database and
encryption configuration from the root README. Generate a separate agent key:

```sh
export AGENT_SIGNING_KEY="$(openssl rand -hex 32)"
task agent:example
```

The example listens on HTTP/2 at `http://127.0.0.1:3001`; use `AGENT_PORT` to change
its port. Register that origin using exactly the same signing key. Run the
following from an application depending on `@trytilde/sdk`:

```ts
import { createManagementClient, BinaryPermission, TargetSelection } from '@trytilde/sdk';
import { randomUUID } from 'node:crypto';

// A short-lived user session obtained through OIDC login, not an API key.
const tilde = createManagementClient({ baseUrl: 'http://127.0.0.1:8080', accessToken: process.env.TILDE_ACCESS_TOKEN! });
const { agent } = await tilde.agents.createAgent({
  capabilities: {
    toolsInvoke: { mode: TargetSelection.SELECTED, ids: ['sendMessage'] },
    workRead: BinaryPermission.YES,
    workWrite: BinaryPermission.YES,
    runUpdate: BinaryPermission.YES,
  },
  name: 'Example',
  endpointUrl: 'http://127.0.0.1:3001',
  webhookSigningKey: process.env.AGENT_SIGNING_KEY!,
});
const { user } = await tilde.chat.createUser({ name: 'Alice' });
const { thread } = await tilde.chat.createThread({
  title: 'Example thread',
  primaryAgentId: agent!.id,
  participants: [{ userId: user!.id }, { agentId: agent!.id }],
});
const participant = thread!.participants.find(p => p.userId === user!.id)!;
await tilde.chat.postMessage({
  id: randomUUID(), threadId: thread!.id, participantId: participant.id,
  text: 'Show me a streamed reply',
});
for await (const activity of tilde.chat.watchThread({ threadId: thread!.id })) {
  console.log(activity.kind, activity.textDelta);
}
```

The example is deterministic and needs no model API key. Replace its `run`
function with your framework's reasoning/tool loop. It demonstrates goal/task
management, streamed visible output and explicit completion; it is not an LLM
implementation.

## Agent host

```ts
import { createAgentServer } from '@trytilde/sdk';

const server = createAgentServer({
  signingKey: process.env.AGENT_SIGNING_KEY!,
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
server.listen(3001, '127.0.0.1');
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

`context.takeInputs()` drains accepted steering at a safe checkpoint. Unconsumed
inputs are reported on stream completion and carried into a subsequent invocation.
Use `chat.resumeRun` to explicitly resume a waiting/failed run. Do not guess which
of several waiting objectives a new human message should resume.

## Streaming and deployment

Provider tool `execute(input, { toolCallId })` sends completed input over the
client-streaming `tilde.runtime.v1.ChatService.InvokeTool` RPC. For incremental input, call
`tool.stream(input, chunks, { toolCallId })`; its chunk format is declared by the
provider's `chunkSchema`. Framework adapters must supply chunks explicitly:
registering a local tool does not automatically stream model tokens.

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

Use Node's HTTP/2 transport for agents. `connect-web` in a browser is not the
agent callback transport. A public deployment must preserve HTTP/2 on the callback
path and supply `ENGINE_MANAGEMENT_PUBLIC_URL` as an agent-reachable Tilde origin. Terminate
TLS at the deployment proxy for the example agent, or mount its service behind
an equivalent protected HTTP/2 deployment.

Agent endpoint calls are HMAC-SHA256 signed over
`method.timestamp.hex(sha256(protobuf_request))`. The SDK checks the signature
and a 60-second clock window. Tilde issues a random invocation capability for
callbacks, persists only its hash, and revokes it on terminal state/lease expiry.
Neither capability nor signing key appears in thread history. Management APIs remain
deployment-authenticated as in the rest of this OSS workspace.

## Validation

After building Rust and the SDK:

```sh
ENGINE_TEST_BINARY="$PWD/target/debug/tilde" \
  scripts/with-postgres.sh pnpm --dir sdk/ts test
```

This exercises actual Rust/Node ConnectRPC against disposable Postgres, including
partial streams, reasoning separation, scoped goals/tasks, dependencies,
stop/resume, steering, cancellation and all three thread topologies.

## IAM

Management clients require a bearer token obtained through the installation's
OIDC login: `createManagementClient({baseUrl: managementUrl, accessToken})`. Tilde has no
API keys or role model. The browser UI handles login and stores its token in
local storage.

Agents receive signed connect tokens automatically during invocation and use
only the agent runtime API callback URL. The SDK renews these tokens while the
invocation is live. Tokens and capabilities cannot be selected by the model.
Configure grants when creating or updating an agent; every capability defaults
to deny. For a native agent that sends messages and manages goals/tasks, grant
`toolsInvoke` (selected `sendMessage`), `workRead`, `workWrite` and `runUpdate`.

`ctx.agents.create/get/list/update/delete` call the registry using the current
invocation token. `ctx.invokeAgent({agentId, objective})` starts work for an
allowed agent already participating in the current thread. The server checks
all grants and target IDs; hiding tools is not the authorization mechanism.

## Agent tracing

`createAgentServer` initializes OTel tracing by default. It extracts the platform's
W3C context, records the invocation and callback RPCs, and uploads standard
OTLP/HTTP protobuf batches to Tilde with the current agent connect token. Spans
created by framework instrumentation inside `run` are collected under the same
invocation; prompts/tool inputs are recorded only if that instrumentation opts in.
Concurrent invocations have separate bounded export queues even when they share
a trace. Final uploads can continue after cancellation within Tilde's five-minute
telemetry grace window.

If the process already configures an OTel provider, add the exported
`agentSpanProcessor` to that provider's `spanProcessors` and set
`tracing: "existing"` on `createAgentServer`. Initialize that provider's async
context manager before starting the agent. This avoids replacing the application's
provider while collecting its instrumented spans through Tilde.


Management clients use `createManagementClient(...)`; programmatic invocation clients use
`createRuntimeClient(...)`. Generated contracts are exported separately from
`@trytilde/sdk/management`, `@trytilde/sdk/runtime`, and `@trytilde/sdk/agent-host`.
Runtime thread reads, attachment operations and typing derive scope from the connect token.
`AgentContext.getMessages()` returns canonical `messages` and a separate `cachedMessages`
collection. Initial conversions are available through `AgentContext.cachedMessages`.
Canonical messages have no cache field, including those returned to management callers.

## Serverless invocation controls

`createAgentHandler()` exposes the same implementation as `createAgentServer()`
as a Node HTTP request handler. Use it in a serverless route instead of starting
an HTTP server. `pathPrefix` must match the deployed endpoint prefix:

```ts
import { createAgentHandler } from '@trytilde/sdk';

export default createAgentHandler({
  signingKey: process.env.AGENT_SIGNING_KEY!,
  pathPrefix: '/api/agent',
  async run(context) {
    // Restore framework state by context.threadId, then run your agent.
    // Pass context.signal to model and tool calls.
  },
  async checkpoint(context) {
    // Quiesce your framework and persist its state before resolving.
  },
});
```

Mount a catch-all route below that prefix so ConnectRPC method paths reach the
handler; preserve the raw request stream (disable framework body parsing). The
handler supports HTTP/1 requests, including those behind a serverless ingress.
No session affinity is required for control requests.

Each running invocation opens `InvocationControlService.WatchCommands` to its
callback URL before user code starts. Steering is accepted into that execution's
input queue and acknowledged. Stop aborts its signal. Suspend runs the optional
checkpoint hook, acknowledges it, and ends the request; resume is a fresh invocation
that restores framework state. Already-persisted conversation state remains in Tilde;
the hook owns any additional framework checkpoint. It must not merely copy mutable
state while the framework continues executing.

Unacknowledged input replays after reconnect and is deduplicated by input ID.
Control connections use the current renewed invocation token. A sustained connection
failure aborts the local execution. Cancellation is cooperative: custom code must
honor the signal. The host has only Invoke and Healthz; inbound Stop/Steer/Cancel
methods and synchronous pause acknowledgements no longer exist.

Invoke is a wake, not a result channel. The handler still verifies the HMAC signature
and keeps the response open until the invocation ends (serverless platforms need the
request alive), and it echoes `acceptedCommandId` for legacy gateways, but reasoning
and the end of the run are no longer written to that stream. Every host shape reports
them through `tilde.run.v1.RunService.Report` instead; see "Connected hosts".

## Connected hosts

A long-running process does not need an inbound endpoint or a signing key. It dials
in with a deployment token (from the Deployments tab or `RegisterDeployment`) and
receives wakes as frames on `tilde.run.v1.RunService.Watch`:

```ts
import { connectAgent } from '@trytilde/sdk';

const host = connectAgent({
  gatewayUrl: process.env.TILDE_RUNTIME_URL!, // the runtime listener root
  deploymentToken: process.env.TILDE_DEPLOYMENT_TOKEN!,
  instanceId: process.env.HOSTNAME, // optional; defaults to a random UUID
  async run(context) {
    // Identical to createAgentServer's run.
  },
  async checkpoint(context) {
    // Optional, as for createAgentHandler.
  },
});
for (const signal of ['SIGINT', 'SIGTERM'] as const)
  process.once(signal, () => void host.close());
```

`connectAgent` opens `Watch` with `authorization: Bearer <deploymentToken>` and the
instance ID, records the `registered` frame (`host.registration`, `onRegistered`),
runs each `wake` frame concurrently with the same execution path as an HTTP wake
(virtual-thread exclusivity, controls subscription, checkpoint/suspension), and ignores
pings. It sends `Heartbeat({ instanceId, ready: true })` every 3 seconds and reconnects
after a 1-second backoff whenever the stream ends, until `close()` is called. `close()`
stops watching and heartbeating, stops active invocations and waits for their final
reports. Stream wakes are authenticated by the deployment token, so no HMAC check applies.

Every host, connected or serverless, reports through `RunService.Report` on the
invocation's `callbackUrl` with `authorization: Bearer <capability>` (renewed like the
other runtime RPCs): `accepted { commandId }` once controls are ready, one
`reasoningDelta` per `context.reason()` call, and exactly one `stopped { pendingInputIds }`
when the invocation ends by return, stop or suspension. Report failures are logged and
never crash the host. `createAgentHandler`/`createAgentServer` remain the choice for
serverless routes and hosts that must be reachable inbound.

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
