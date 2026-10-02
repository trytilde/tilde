# Tilde Python SDK

`trytilde` (import `tilde`) is the Python counterpart of `@trytilde/sdk`: the same dial-in
agent host, invocation-bound context, channel tools, message history, run reports, tracing and
logging, plus FastAPI integration and the server-side chat proxy. Framework adapters live in
sibling packages: `trytilde-langchain`, `trytilde-pydantic-ai`, `trytilde-openai-agents` and
`trytilde-agno`.

```sh
cd sdk/py
uv sync --all-packages          # workspace environment with every package and example
uv run python scripts/generate.py   # Protobuf messages and Connect stubs from proto/
uv run pytest                    # adapter unit tests and the host tests against a fake gateway
```

Generated contracts sit inside the package under the proto package names
(`tilde.runtime.v1.chat_pb2`, `tilde.types.v1.chat_pb2`, `tilde.run.v1.run_connect`, ...);
they are not committed. Every RPC uses `connectrpc` with Google protobuf messages over
`pyqwest` HTTP/2.

## Agent host

```python
from tilde import AgentContext, run_connected_agent


async def run(ctx: AgentContext) -> None:
    await ctx.reason("Inspecting the request.")  # execution activity, never a message
    goal = await ctx.goals.create(objective=ctx.objective)
    task = await ctx.tasks.create(title="Reply", goal_id=goal.id)

    async def words():
        yield "Hello "
        yield "there."

    await ctx.send_native_message(words())
    await ctx.tasks.update(id=task.id, status="completed")
    await ctx.goals.update(id=goal.id, status="completed")
    await ctx.set_run_status("completed")
    ctx.stop()


run_connected_agent(run=run)  # TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN from the environment
```

Tilde never calls an agent over HTTP. A host dials out: `connect_agent` opens
`tilde.run.v1.RunService.Watch` with the deployment token (from `RegisterDeployment` or
`IssueDeploymentToken`) and receives each wake as a stream frame, so the process needs no
inbound port, public URL or shared secret. Wakes run concurrently; the host heartbeats every
3 seconds and reconnects after 1 second whenever the stream ends, until `close()`.
`run_connected_agent(**options)` is the blocking form for standalone scripts: it connects,
waits for SIGINT/SIGTERM and closes. Inside an existing event loop use `connect_agent`:

```python
from tilde import connect_agent

host = connect_agent(gateway_url=RUNTIME_URL, deployment_token=TOKEN, run=run, ready=database_is_up)
await host.wait()  # until host.close()
```

`gateway_url` and `deployment_token` default to `TILDE_GATEWAY_URL` and
`TILDE_DEPLOYMENT_TOKEN`. `ready` is an optional sync or async check whose result is sent with
every heartbeat (not ready when it raises); Tilde only routes wakes to ready instances.

For each wake the host opens `InvocationControlService.WatchCommands` before user code starts,
fetches the invocation-scoped tool catalog, and reports `accepted`, every `reason()` delta and
exactly one `stopped` through `RunService.Report` with the invocation token.

`ctx.tools` holds SDK-local `stop`, `goals.*` and `tasks.*` helpers plus provider tools with
server-authored descriptions, JSON schemas and optional chunk schemas. `ctx.channel.current`
exposes only the inbound connection's tools; `ctx.channel.slack`, `github`, `agentmail`, `linq`,
`whatsapp`, `telnyx_whatsapp` and `native` resolve providers, `ctx.channel.for_connection(id)`
and `ctx.channel.provider(id)` select explicitly, and `call_channel_tool(name, json)` reaches
custom providers. Tools are awaited with the framework's tool-call ID:
`await ctx.channel.slack.send_message({"channelId": "C1", "text": "Hi"}, tool_call_id=call_id)`.
Streaming tools accept an async iterable of chunks in the provider's chunk format.

Provider tools carry `summary`, `output_schema`, `annotations` and `background`;
`ctx.tool_source(slug)` returns one tool source's tools keyed by tool name. `ctx.agent_tools`
collects everything Tilde gives the agent besides messaging (tool sources, Tilde's built-in
tools, personal tools), beside `ctx.channel.current`.

The agent's bundled tools are its own framework tools, written the framework's way and run in
its process. Each adapter's `await with_tilde_tools(ctx, native_tools, options=...)` returns the
framework's tool collection with the current channel's tools, `ctx.agent_tools` and the native
tools, publishes the native tools to Tilde (so `tools.search` and `tools.schemas` describe them,
a `tools.execute` naming one runs it here, and the agent's Tools tab lists them read-only), and
audits every call once with its summary. `BundledOptions(summary=..., display="summary",
annotations=ToolAnnotations(read_only=True))` sets the Tilde metadata per tool name; `display`
(`"full"`, `"summary"` or `"hidden"`) is how its calls show in end-user chats, and traces keep
full detail. Call it once per invocation; the set is replaced each time.

```python
from langchain_core.tools import tool
from tilde import BundledOptions
from tilde_langchain import with_tilde_tools


@tool
def read_file(path: str) -> str:
    """Read a file from the workspace."""
    return Path(path).read_text()


tools = await with_tilde_tools(
    ctx, [read_file], options={"read_file": BundledOptions(summary="Read a file")}
)
```

`ctx.message.history(limit=..., before_message_id=..., include_objective=True, include_work=False)`
returns typed `ConversationMessage`, `ObjectiveMessage`, `GoalMessage` and `TaskMessage` items;
the adapters convert them to framework messages and hydrate attachments through the scoped
`ctx.attachments.download`. `ctx.agents.*` and `ctx.invoke_agent(...)` call the registry with the
current token; the server checks every grant.

Cancellation is cooperative asyncio cancellation: a stop control or a lost callback connection
cancels the task running `run`, and `ctx.stop()` raises `StopLoop` (a `BaseException`, so it
passes through framework `except Exception` handlers). Call `ctx.check()` at framework
checkpoints. Returning from `run` never publishes a message; unfinished runs become waiting
unless `set_run_status` says otherwise. Connect tokens are renewed at four minutes; a denied
renewal aborts execution. `checkpoint=` receives the context on suspension and must quiesce
the framework before returning.

## Inference, prompts and skills

```python
import tilde

MODEL = tilde.inference("default")  # module scope: resolves the running invocation per request
SYSTEM = tilde.define_prompt(
    "system",
    template="You are {{name}}.\n{{> rules}}",
    sections={"rules": "Be brief."},
    config={"model": "gpt-4o-mini"},
)
SKILLS = tilde.define_skills("skills")  # folders holding a SKILL.md, relative to this file


async def run(ctx: tilde.AgentContext) -> None:
    client = AsyncOpenAI(
        base_url=MODEL.base_url, api_key=MODEL.api_key, http_client=MODEL.async_client()
    )
    instructions = SYSTEM.render(name="Support")  # marks the prompt active for this invocation
```

`tilde.inference(alias)` returns the same `Inference` as `ctx.inference(alias)`, but every
request resolves the invocation running it (token, callback URL, active prompt stamps), so model
clients can be built at import time; a request outside `run(ctx)` raises. Both send
`x-tilde-prompt: name@hash[, ...]` for prompts made active with `ctx.prompt(definition)` or by
rendering one inside the invocation. `define_skill(name, description, instructions, files=...)`
declares a skill in code with a generated `SKILL.md`.

At runtime `ctx.skills` reads every skill the invocation may use: `list()` (each
`SkillSummary` says whether it is `deployed`, shipped beside the code, or assigned through the
registry), `read(name, path="SKILL.md")` (text inline, other files as a short-lived
`download_url`) and `summary()` (a system-prompt block). `directory()` keeps the registry skills
in `$TILDE_SKILLS_DIR` (the OS temp directory by default) `/tilde-skills/<agent id>/<name>/…`
for frameworks that load skill folders from disk: the engine pushes the invocation's skills and
versions with each wake, and only new and newer versions are downloaded and removed skills
deleted, so a skill assigned in Tilde reaches
the next invocation without a redeploy. Frameworks without native skills convert
`ctx.skills.tools()` (`list_skills`, `read_skill`) like channel tools and add `summary()` to
their instructions. Framework adapters stamp dynamic prompts with
`ctx.activate_prompt(name, hash)`.

Prompts and skills are registered with a deployment by the [Tilde CLI](https://trytilde.ai/docs/cli),
which this package asks for at its own version, so `uv run tilde` is the matching build:

```sh
uv run tilde dev                                   # register, run and chat with the agent locally
uv run tilde deploy [ENTRY] [--agent-id ID] [--url URL] [--api-key KEY] \
  [--target gateway|sidecar|lambda] [--function-arn ARN] [--external-id ID] [--label L] \
  [--dry-run] [--json]
```

The CLI reads what the code declares through this package's own entrypoint, which prints the
`DeploymentDeclarations` JSON and contacts nothing:

```sh
tilde-declarations [ENTRY] [--allow-empty]         # or: python -m tilde declarations [ENTRY]
```

`ENTRY` is a file or dotted module (default `main.py`), imported with `TILDE_DISCOVERY=1`
(`connect_agent`/`run_connected_agent` then do nothing) and not as `__main__`. The globals of
the entry and of every module loaded from the working directory are scanned for `define_*`
objects and offered to framework discoverers (entry point group `tilde.discover`, for example
`trytilde-crewai`). The inventory goes to stderr; `tilde deploy --dry-run` prints the
`DeploymentDeclarations` JSON; otherwise binary or large skill files are uploaded, the
deployment is registered (`$TILDE_AGENT_ID`, `$TILDE_URL` = management API, and on Tilde Cloud
`$TILDE_API_KEY`; open-source Tilde needs no key) and stdout is the deployment token (`--json`: `{"deploymentId", "token", "created"}`).

## FastAPI

```python
from fastapi import FastAPI
from tilde.fastapi import agent_lifespan, mount_chat_proxy

app = FastAPI(lifespan=agent_lifespan(run=run))
mount_chat_proxy(
    app,
    "/api/chat",
    agent_id=os.environ["TILDE_AGENT_ID"],
    api_key=os.environ["TILDE_CHAT_API_KEY"],
    resolve_identity=lambda request, agent_id: request.session.get("user_id"),
)
```

`agent_lifespan(**connect_agent_options)` calls `connect_agent` on startup and `close()` on
shutdown, so the application hosts an agent without exposing any agent route. Any ASGI server
works.

## Chat proxy

`tilde.chat_proxy.create_chat_proxy(...)` is the Python `@trytilde/chat-proxy`: a pure ASGI
reverse proxy for the `tilde.provider.tilde.v1.ChatService` RPCs used by the embeddable chat UI.
It only forwards declared methods, replaces browser credentials with the server-held API key,
sets the base64url `x-tilde-identity` header from `resolve_identity(request, agent_id)`, keeps
cookies at the host, rejects cross-origin browser requests and upstream redirects, streams
request and response bodies without buffering, and never exposes upstream error details.
`GET /api/chat/agents` lists the agents the caller may use. The same-origin check honors
`X-Forwarded-Proto`/`X-Forwarded-Host` from a TLS-terminating reverse proxy; pass
`trust_forwarded_headers=False` when the ASGI server is exposed directly. Multi-agent proxies take
`agents=[ChatAgentConfig(agent_id, api_key, base_url=...), ...]` and route
`/api/chat/{agentId}/tilde.provider.tilde.v1.ChatService/{Method}`.

## Cloud invoke

`create_lambda_handler(run=run)` returns an AWS Lambda handler for the JSON-encoded
`InvokeRequest` delivered by the cloud invoke API. It runs the invocation through the same
execution path as a Watch wake and reports through `RunService`.

## Tool hosts

`tilde.tool_hosts` serves your own tools to agents. Input and output schemas come from the
pydantic annotations; an `Auth` publishes a provider whose connections (instances) carry
credentials, parsed into the method's model as `ctx.auth`.

```python
from pydantic import BaseModel, SecretStr
from tilde.tool_hosts import Auth, Method, ToolContext, create_tool_host


class ApiKey(BaseModel):
    api_key: SecretStr  # credential fields are strings; SecretStr renders as a secret field


async def verify(auth: ApiKey, instance) -> str | None:
    return "acme"  # account label; raise to refuse (the message is shown on the setup form)


auth = Auth(
    id="acme-crm",
    name="Acme CRM",
    methods={"api_key": Method(name="API key", schema=ApiKey)},
    verify=verify,
)


class SearchInput(BaseModel):
    q: str


class SearchOutput(BaseModel):
    results: list[str]


@auth.tool(description="Search the CRM.")
async def search(input: SearchInput, ctx: ToolContext[ApiKey]) -> SearchOutput:
    return SearchOutput(results=[])


await create_tool_host(auth=auth, tools=[search]).run()  # TILDE_GATEWAY_URL, TILDE_TOOL_HOST_TOKEN
```

`@tool(...)` declares a tool without credentials. `create_tool_host` dials
`ToolHostService.Watch`, runs calls concurrently and reconnects with backoff until `close()`;
`create_tool_lambda_handler(auth=..., tools=[...])` answers the Protobuf-JSON `LambdaRequest`
events of a Lambda tool host. A raised exception's message becomes the tool's error, as does an
output that fails the output model.

## Tracing and logs

Hosts install an OpenTelemetry tracer and logger provider by default. The W3C context of the
wake becomes an `agent.invoke` server span; spans and log records emitted inside the
invocation's asyncio context are batched per invocation and uploaded as OTLP/HTTP protobuf to
the callback base plus `/v1/traces` and `/v1/logs` with the current connect token. Records from
the standard `logging` module are routed through a root handler; nothing outside an invocation
or configured deployment scope is exported. Pass `tracing="existing"` / `logging="existing"`
and add `agent_span_processor` / `agent_log_processor` to your own providers to keep them.
`connect_agent` exports logs outside an invocation with the deployment credentials (one
deployment per process); call `configure_deployment_logging(gateway_url, deployment_token)`
earlier to capture logs emitted before it.

## Management, chat and runtime clients

```python
from tilde import create_management_client, create_tilde_chat_client
from tilde.management.v1.tilde_chat_pb2 import GetCredentialsRequest
from tilde.provider.tilde.v1 import chat_pb2

tilde = create_management_client("http://127.0.0.1:8080")
# Management provisions credentials; chat operations use the Tilde chat provider.
credentials = await tilde.tilde_chat.get_credentials(GetCredentialsRequest(agent_id=agent_id))
chat = create_tilde_chat_client(
    "http://127.0.0.1:8080", agent_id, api_key=credentials.api_key, identity="alice"
)
user = (await chat.get_identity(chat_pb2.GetIdentityRequest())).user
```

`ManagementClient` groups `access`, `agents`, `connections`, `deployments`,
`identities`, `inference`, `logs`, `tilde_chat` and `traces`; `RuntimeClient` groups `agents`
and `chat`. `create_tilde_chat_client` is the server-side client for the built-in chat provider:
resolve `identity` from your authenticated application user, never from browser input. All
clients use the generated request messages directly. Open-source Tilde's management API is
unauthenticated (secure it with your own proxy); pass `access_token=` for Tilde Cloud, which
requires a management credential.
