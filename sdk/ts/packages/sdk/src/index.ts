import { LogsService } from "./gen/tilde/management/v1/logs_pb.js";
import {
  InvocationControlService,
  InvocationCommandKind,
} from "./gen/tilde/runtime/v1/controls_pb.js";
import { TracingService } from "./gen/tilde/management/v1/tracing_pb.js";
import { AgentAccessService } from "./gen/tilde/management/v1/access_pb.js";
import { initializeTracing, invocationTracing, tracingInterceptor } from "./tracing.js";
export { agentSpanProcessor } from "./tracing.js";
import {
  createClient as connectClient,
  Code,
  ConnectError,
  type Client as RpcClient,
  type HandlerContext,
} from "@connectrpc/connect";
import { createConnectTransport, connectNodeAdapter } from "@connectrpc/connect-node";
import {
  toBinary,
  fromJson,
  type JsonValue,
  type DescMessage,
  type Message,
} from "@bufbuild/protobuf";
import { createServer } from "node:http2";
import { createHash, createHmac, randomUUID, timingSafeEqual } from "node:crypto";
import { ConnectionsService } from "./gen/tilde/management/v1/connections_pb.js";
import { AgentService as ManagementAgentService } from "./gen/tilde/management/v1/agents_pb.js";
import { AgentService as RuntimeAgentService } from "./gen/tilde/runtime/v1/agents_pb.js";
import { ChatService as ManagementChatService } from "./gen/tilde/management/v1/chat_pb.js";
import { ChatService as RuntimeChatService } from "./gen/tilde/runtime/v1/chat_pb.js";
import {
  AgentService as AgentHostService,
  InvokeRequestSchema,
  type InvokeRequest,
} from "./gen/tilde/agent_host/v1/agent_pb.js";
import {
  MessageSchema,
  type Participant,
  type Message as ChatMessage,
} from "./gen/tilde/types/v1/chat_pb.js";
export * from "./gen/tilde/types/v1/chat_pb.js";
export { RuntimeChatService, AgentHostService };
export { BinaryPermission, TargetSelection } from "./gen/tilde/types/v1/agent_pb.js";
/** Connection contracts are namespaced so their capabilities stay distinct from IAM grants. */
export * as management from "./management.js";
export * as runtime from "./runtime.js";

/** One reusable HTTP/2 transport and generated Connect clients; no REST or tenant wrapper. */
export type ClientOptions = { baseUrl: string; accessToken?: string };
function transport(options: ClientOptions) {
  return createConnectTransport({
    baseUrl: options.baseUrl,
    httpVersion: "2",
    interceptors: [
      tracingInterceptor,
      (next) => async (request) => {
        if (options.accessToken)
          request.header.set("Authorization", `Bearer ${options.accessToken}`);
        return next(request);
      },
    ],
  });
}
export class ManagementClient {
  readonly access: RpcClient<typeof AgentAccessService>;
  readonly agents: RpcClient<typeof ManagementAgentService>;
  readonly chat: RpcClient<typeof ManagementChatService>;
  readonly connections: RpcClient<typeof ConnectionsService>;
  readonly logs: RpcClient<typeof LogsService>;
  readonly traces: RpcClient<typeof TracingService>;
  constructor(options: ClientOptions) {
    const rpc = transport(options);
    this.access = connectClient(AgentAccessService, rpc);
    this.agents = connectClient(ManagementAgentService, rpc);
    this.chat = connectClient(ManagementChatService, rpc);
    this.connections = connectClient(ConnectionsService, rpc);
    this.logs = connectClient(LogsService, rpc);
    this.traces = connectClient(TracingService, rpc);
  }
}
export class RuntimeClient {
  readonly agents: RpcClient<typeof RuntimeAgentService>;
  readonly chat: RpcClient<typeof RuntimeChatService>;
  constructor(options: ClientOptions) {
    const rpc = transport(options);
    this.agents = connectClient(RuntimeAgentService, rpc);
    this.chat = connectClient(RuntimeChatService, rpc);
  }
}
export const createManagementClient = (options: ClientOptions) => new ManagementClient(options);
export const createRuntimeClient = (options: ClientOptions) => new RuntimeClient(options);

/** Local wrappers for invocation-scoped provider tools. Descriptions and formats come from Tilde. */
export type Tool = {
  description: string;
  inputSchema: Record<string, unknown>;
  /** Present on provider tools; SDK-local lifecycle helpers have no provider owner. */
  providerId?: string;
  chunkSchema?: Record<string, unknown>;
  execute(input: unknown, execution: { toolCallId: string }): Promise<unknown>;
  /** A framework must explicitly feed chunks; registering a local tool does not stream tokens. */
  stream?(
    input: unknown,
    chunks: AsyncIterable<unknown>,
    execution: { toolCallId: string },
  ): Promise<unknown>;
};
export type ToolCatalog = Record<string, Tool>;
export type SteeringInput = { id: string; text: string; message?: ChatMessage };

class StopLoop extends Error {}
class Queue<T> {
  private items: T[] = [];
  private wake?: () => void;
  private space?: () => void;
  closed = false;
  error?: unknown;
  async push(item: T) {
    while (this.items.length >= 64 && !this.closed)
      await new Promise<void>((r) => {
        this.space = r;
      });
    if (this.closed) throw new StopLoop();
    this.items.push(item);
    this.wake?.();
  }
  end(error?: unknown) {
    this.closed = true;
    this.error ??= error;
    this.wake?.();
    this.space?.();
  }
  async *read() {
    while (true) {
      if (this.items.length) {
        const item = this.items.shift()!;
        this.space?.();
        yield item;
      } else if (this.closed) {
        if (this.error) throw this.error;
        return;
      } else
        await new Promise<void>((r) => {
          this.wake = r;
        });
    }
  }
}
function toolId(invocation: string, call: string, name: string) {
  if (!call) throw new Error("Tool call ID is required");
  const h = createHash("sha256").update(`${invocation}:${call}:${name}`).digest("hex");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-4${h.slice(13, 16)}-a${h.slice(17, 20)}-${h.slice(20, 32)}`;
}
function object(input: unknown): Record<string, unknown> {
  if (!input || typeof input !== "object" || Array.isArray(input))
    throw new Error("Tool input must be an object");
  return input as Record<string, unknown>;
}
function string(input: Record<string, unknown>, key: string): string {
  if (typeof input[key] !== "string") throw new Error(`${key} must be a string`);
  return input[key];
}

/** Invocation-bound SDK context. The model cannot choose its acting agent or thread. */
export class AgentContext {
  readonly invocationId: string;
  readonly runId: string;
  readonly threadId: string;
  readonly agentId: string;
  readonly agentGeneration: bigint;
  readonly objective: string;
  private messageHistory: ChatMessage[];
  get messages(): readonly ChatMessage[] {
    return this.messageHistory;
  }
  readonly getMessages;
  readonly cachedMessages: InvokeRequest["cachedMessages"];
  readonly participants: readonly Participant[];
  readonly signal: AbortSignal;
  readonly tools: ToolCatalog;
  readonly agents;
  readonly invokeAgent;
  readonly attachments;
  readonly goals;
  readonly tasks;
  private readonly session: RpcClient<typeof RuntimeChatService>;
  private readonly providerToolNames = new Set<string>();
  private readonly steering: SteeringInput[] = [];
  private readonly accepted = new Map<string, string>();
  #headers: Headers;
  private readonly controls: RpcClient<typeof InvocationControlService>;
  private suspension?: Promise<void>;
  async settleSuspension() {
    await this.suspension;
  }
  constructor(
    request: InvokeRequest,
    private readonly controller: AbortController,
    private readonly output: Queue<{
      reasoningDelta?: string;
      stopped?: boolean;
    }>,
  ) {
    this.invocationId = request.invocationId;
    this.runId = request.runId;
    this.threadId = request.threadId;
    this.agentId = request.agentId;
    this.agentGeneration = request.agentGeneration;
    this.objective = request.objective;
    this.messageHistory = request.messages;
    this.cachedMessages = request.cachedMessages;
    this.participants = request.thread?.participants ?? [];
    this.signal = controller.signal;
    this.#headers = new Headers({
      authorization: `Bearer ${request.capability}`,
    });
    const transport = createConnectTransport({
      baseUrl: request.callbackUrl,
      httpVersion: "2",
      interceptors: [tracingInterceptor],
    });
    this.session = connectClient(RuntimeChatService, transport);
    this.controls = connectClient(InvocationControlService, transport);
    const options = () => ({ headers: this.#headers, signal: this.signal });
    const renewal = setInterval(
      async () => {
        try {
          const renewed = await this.session.renewConnectToken({}, options());
          this.#headers.set("authorization", `Bearer ${renewed.token}`);
        } catch {
          this.controller.abort();
        }
      },
      5 * 60 * 1000,
    );
    renewal.unref();
    this.signal.addEventListener("abort", () => clearInterval(renewal), { once: true });
    const registry = connectClient(RuntimeAgentService, transport);
    const chat = connectClient(RuntimeChatService, transport);
    this.getMessages = (input: { beforeMessageId?: string; limit?: number } = {}) =>
      chat.listMessages({ ...input }, options());
    this.attachments = {
      upload: (input: { id?: string; filename: string; mediaType: string; content: Uint8Array }) =>
        chat.uploadAttachment({ ...input, id: input.id ?? randomUUID() }, options()),
      download: (attachmentId: string) => chat.downloadAttachment({ attachmentId }, options()),
    };
    this.agents = {
      create: (input: Parameters<typeof registry.createAgent>[0]) =>
        registry.createAgent(input, options()),
      get: (id: string) => registry.getAgent({ id }, options()),
      list: (input: Parameters<typeof registry.listAgents>[0] = {}) =>
        registry.listAgents(input, options()),
      update: (input: Parameters<typeof registry.updateAgent>[0]) =>
        registry.updateAgent(input, options()),
      delete: (id: string) => registry.deleteAgent({ id }, options()),
    };
    this.invokeAgent = (input: { agentId: string; objective: string; idempotencyKey?: string }) =>
      chat.startRun({ ...input, idempotencyKey: input.idempotencyKey ?? randomUUID() }, options());
    this.goals = {
      list: async () => (await this.session.listGoals({}, options())).goals,
      create: async (input: { objective: string; id?: string }) =>
        (await this.session.createGoal({ ...input, id: input.id ?? randomUUID() }, options()))
          .goal!,
      update: async (input: {
        id: string;
        status: "active" | "completed" | "failed" | "canceled";
      }) => (await this.session.updateGoal(input, options())).goal!,
    };
    this.tasks = {
      list: async () => (await this.session.listTasks({}, options())).tasks,
      create: async (input: {
        title: string;
        id?: string;
        goalId?: string;
        dependencyIds?: string[];
      }) =>
        (await this.session.createTask({ ...input, id: input.id ?? randomUUID() }, options()))
          .task!,
      update: async (input: {
        id: string;
        status: "pending" | "working" | "blocked" | "completed" | "failed" | "canceled";
        blockedReason?: string;
      }) => (await this.session.updateTask(input, options())).task!,
    };
    const tool = (
      description: string,
      properties: Record<string, unknown>,
      required: string[],
      execute: Tool["execute"],
    ): Tool => ({
      description,
      inputSchema: {
        type: "object",
        properties,
        required,
        additionalProperties: false,
      },
      execute,
    });
    const str = { type: "string" };
    this.tools = {
      stop: tool(
        "Stop this invocation without completing its run, goal or tasks.",
        {},
        [],
        async () => this.stop(),
      ),
      "goals.list": tool("List your goals in this thread.", {}, [], async () => this.goals.list()),
      "goals.create": tool(
        "Create your desired outcome.",
        { objective: str },
        ["objective"],
        async (i, e) =>
          this.goals.create({
            objective: string(object(i), "objective"),
            id: toolId(this.invocationId, e.toolCallId, "goal"),
          }),
      ),
      "goals.update": tool(
        "Change your goal status.",
        {
          id: str,
          status: { enum: ["active", "completed", "failed", "canceled"] },
        },
        ["id", "status"],
        async (i) =>
          this.session.updateGoal(
            {
              id: string(object(i), "id"),
              status: string(object(i), "status"),
            },
            options(),
          ),
      ),
      "tasks.list": tool("List your tasks in this thread.", {}, [], async () => this.tasks.list()),
      "tasks.create": tool(
        "Create your task with optional existing dependencies.",
        {
          title: str,
          goalId: str,
          dependencyIds: { type: "array", items: str },
        },
        ["title"],
        async (i, e) => {
          const p = object(i);
          if (
            p.dependencyIds !== undefined &&
            (!Array.isArray(p.dependencyIds) ||
              !p.dependencyIds.every((v) => typeof v === "string"))
          )
            throw new Error("Invalid dependencies");
          return this.tasks.create({
            title: string(p, "title"),
            id: toolId(this.invocationId, e.toolCallId, "task"),
            goalId: p.goalId === undefined ? undefined : string(p, "goalId"),
            dependencyIds: p.dependencyIds as string[] | undefined,
          });
        },
      ),
      "tasks.update": tool(
        "Change task status; blocked tasks include a reason.",
        {
          id: str,
          status: {
            enum: ["pending", "working", "blocked", "completed", "failed", "canceled"],
          },
          blockedReason: str,
        },
        ["id", "status"],
        async (i) =>
          this.session.updateTask(
            {
              id: string(object(i), "id"),
              status: string(object(i), "status"),
              blockedReason:
                object(i).blockedReason === undefined ? "" : string(object(i), "blockedReason"),
            },
            options(),
          ),
      ),
    };
  }
  /** Persist opaque converted messages in this agent's cache; canonical messages stay unchanged. */
  async cacheConvertedMessages(input: {
    messages: { chatMessageId: string; message: JsonValue }[];
  }) {
    return this.session.cacheConvertedMessages(
      {
        messages: input.messages.map((m) => ({
          messageId: m.chatMessageId,
          messageJson: JSON.stringify(m.message),
        })),
      },
      { headers: this.#headers, signal: this.signal },
    );
  }
  async hydrateConvertedMessages(input: {
    messageIds: string[];
  }): Promise<{ messages: { chatMessageId: string; message: JsonValue }[] }> {
    const result = await this.session.hydrateConvertedMessages(input, {
      headers: this.#headers,
      signal: this.signal,
    });
    return {
      messages: result.messages.map((m) => ({
        chatMessageId: m.messageId,
        message: JSON.parse(m.messageJson) as JsonValue,
      })),
    };
  }
  /** Audit local framework tools. Provider tools invoked through Tilde are audited automatically. */
  async reportToolCall(input: {
    toolCallId: string;
    name: string;
    status: "running" | "completed" | "failed" | "aborted";
    input?: JsonValue;
    output?: JsonValue;
    error?: string;
    inputDelta?: string;
  }) {
    await this.session.reportToolCall(
      {
        toolCall: {
          id: toolId(this.invocationId, input.toolCallId, input.name),
          name: input.name,
          status: input.status,
          inputJson: input.input === undefined ? "" : JSON.stringify(input.input),
          outputJson: input.output === undefined ? "" : JSON.stringify(input.output),
          error: input.error ?? "",
          inputDelta: input.inputDelta ?? "",
        },
      },
      { headers: this.#headers, signal: this.signal },
    );
  }
  async setTyping(typing: boolean) {
    await this.session.setTyping({ typing }, { headers: this.#headers, signal: this.signal });
  }

  /** Refresh server-authored descriptors before exposing local tools to the reasoning framework. */
  async refreshTools() {
    const response = await this.session.listTools(
      {},
      { headers: this.#headers, signal: this.signal },
    );
    const incoming = new Map<string, Tool>();
    for (const definition of response.tools) {
      if (
        !definition.name ||
        ["__proto__", "constructor", "prototype"].includes(definition.name) ||
        incoming.has(definition.name) ||
        (Object.hasOwn(this.tools, definition.name) && !this.providerToolNames.has(definition.name))
      ) {
        throw new ConnectError("Conflicting provider tool catalog", Code.Internal);
      }
      const inputSchema = JSON.parse(definition.inputSchemaJson) as Record<string, unknown>;
      const chunkSchema = definition.chunkSchemaJson
        ? (JSON.parse(definition.chunkSchemaJson) as Record<string, unknown>)
        : undefined;
      const wrapper: Tool = {
        description: definition.description,
        inputSchema,
        providerId: definition.providerId,
        chunkSchema,
        execute: (input, execution) =>
          this.invokeProviderTool(definition.name, input, undefined, execution.toolCallId),
      };
      if (chunkSchema)
        wrapper.stream = (input, chunks, execution) =>
          this.invokeProviderTool(definition.name, input, chunks, execution.toolCallId);
      incoming.set(definition.name, wrapper);
    }
    for (const name of this.providerToolNames) delete this.tools[name];
    this.providerToolNames.clear();
    for (const [name, wrapper] of incoming) {
      this.tools[name] = wrapper;
      this.providerToolNames.add(name);
    }
  }
  private async invokeProviderTool(
    name: string,
    input: unknown,
    chunks: AsyncIterable<unknown> | undefined,
    toolCallId: string,
  ) {
    this.signal.throwIfAborted();
    const callId = toolId(this.invocationId, toolCallId, name);
    const encode = (value: unknown) => {
      const encoded = JSON.stringify(value);
      if (encoded === undefined) throw new Error("Tool inputs must be JSON values");
      return encoded;
    };
    async function* frames() {
      yield { name, callId, sequence: 0, inputJson: encode(input), finish: !chunks };
      if (!chunks) return;
      let sequence = 1;
      for await (const chunk of chunks)
        yield { name, callId, sequence: sequence++, chunkJson: encode(chunk) };
      yield { name, callId, sequence, finish: true };
    }
    const response = await this.session.invokeTool(frames(), {
      headers: this.#headers,
      signal: this.signal,
    });
    return JSON.parse(response.outputJson) as JsonValue;
  }
  /** Native-thread convenience only. Other providers expose their own sendMessage tool/schema. */
  async sendNativeMessage(
    content: string | AsyncIterable<string>,
    options: {
      toolCallId?: string;
      addressedParticipantIds?: string[];
      inReplyToMessageId?: string;
      attachmentIds?: string[];
    } = {},
  ) {
    const tool = this.tools.sendMessage;
    if (tool?.providerId !== "native" || !tool.stream)
      throw new ConnectError("Native message tool is unavailable", Code.FailedPrecondition);
    async function* chunks() {
      const source =
        typeof content === "string"
          ? (async function* () {
              yield content;
            })()
          : content;
      for await (const chunk of source) {
        if (typeof chunk !== "string") throw new Error("Native message chunks must be strings");
        const chars = Array.from(chunk);
        for (let offset = 0; offset < chars.length; offset += 4096)
          yield { textDelta: chars.slice(offset, offset + 4096).join("") };
      }
    }
    const output = await tool.stream(
      {
        text: "",
        addressedParticipantIds: options.addressedParticipantIds,
        inReplyToMessageId: options.inReplyToMessageId,
        attachmentIds: options.attachmentIds,
      },
      chunks(),
      { toolCallId: options.toolCallId ?? randomUUID() },
    );
    return fromJson(MessageSchema, output as JsonValue);
  }
  /** Stream reasoning into execution activity, never into the thread transcript. */
  async reason(text: string) {
    this.signal.throwIfAborted();
    await this.output.push({ reasoningDelta: text });
  }
  /** Consume newly steered input at the agent framework's next safe checkpoint. */
  takeInputs(): SteeringInput[] {
    return this.steering.splice(0);
  }
  /** SDK lifecycle receipt: inputs not yet handed to the reasoning loop remain pending. */
  pendingInputIds(): string[] {
    return this.steering.map((input) => input.id);
  }
  /** Internal exporter callback: always read the latest renewed token. */
  traceAuthorization(): string {
    return this.#headers.get("authorization")!;
  }
  /** The running request subscribes outward; control never needs host affinity. */
  async consumeControls(ready: () => void, checkpoint?: (context: AgentContext) => Promise<void>) {
    let connected = Date.now();
    while (!this.signal.aborted) {
      try {
        for await (const command of this.controls.watchCommands(
          {},
          { headers: this.#headers, signal: this.signal },
        )) {
          connected = Date.now();
          if (command.kind === InvocationCommandKind.READY) {
            ready();
            continue;
          }
          if (command.kind === InvocationCommandKind.STEER) {
            this.acceptInput({
              id: command.inputId,
              text: command.text,
              ...(command.message ? { message: command.message } : {}),
            });
            await this.controls.acknowledgeCommand(
              { id: command.id },
              { headers: this.#headers, signal: this.signal },
            );
          } else if (
            command.kind === InvocationCommandKind.STOP ||
            command.kind === InvocationCommandKind.SUSPEND
          ) {
            const complete = async () => {
              if (command.kind === InvocationCommandKind.SUSPEND && checkpoint) {
                try {
                  await checkpoint(this);
                } catch (error) {
                  try {
                    await this.setRunStatus("failed");
                  } finally {
                    this.output.end(
                      new ConnectError("Suspension checkpoint failed", Code.Internal),
                    );
                  }
                  throw error;
                }
              }
              await this.controls.acknowledgeCommand(
                { id: command.id },
                { headers: this.#headers, signal: this.signal },
              );
            };
            const completion = complete();
            if (command.kind === InvocationCommandKind.SUSPEND) this.suspension = completion;
            try {
              await completion;
            } finally {
              this.controller.abort(new StopLoop());
            }
            return;
          } else {
            throw new ConnectError("Unknown invocation control", Code.InvalidArgument);
          }
        }
      } catch (error) {
        if (this.signal.aborted) return;
        const code = ConnectError.from(error).code;
        if (
          [
            Code.Unauthenticated,
            Code.PermissionDenied,
            Code.NotFound,
            Code.InvalidArgument,
            Code.AlreadyExists,
            Code.ResourceExhausted,
          ].includes(code)
        )
          throw error;
      }
      if (Date.now() - connected >= 15_000)
        throw new ConnectError("Invocation control connection lost", Code.Unavailable);
      await new Promise<void>((resolve) => {
        const done = () => {
          clearTimeout(timer);
          this.signal.removeEventListener("abort", done);
          resolve();
        };
        const timer = setTimeout(done, 250);
        this.signal.addEventListener("abort", done, { once: true });
      });
    }
  }
  /** Internal server hook; duplicate IDs replay acknowledgment, different content is rejected. */
  acceptInput(input: SteeringInput) {
    this.signal.throwIfAborted();
    const prior = this.accepted.get(input.id);
    if (prior !== undefined) {
      if (prior !== input.text) throw new ConnectError("Input ID conflict", Code.AlreadyExists);
      return;
    }
    if (this.accepted.size >= 4096)
      throw new ConnectError("Too many steering inputs", Code.ResourceExhausted);
    if (input.message) {
      if (input.message.id !== input.id || input.message.threadId !== this.threadId)
        throw new ConnectError("Steering message is outside this input", Code.InvalidArgument);
      this.messageHistory = [
        ...this.messageHistory.filter((m) => m.id !== input.message!.id),
        input.message,
      ].slice(-100);
    }
    this.accepted.set(input.id, input.text);
    this.steering.push(input);
  }
  async setRunStatus(status: "waiting" | "completed" | "failed" | "canceled") {
    await this.session.setRunStatus({ status }, { headers: this.#headers, signal: this.signal });
  }
  /** Always present. Framework integrations must honor signal cancellation and this control exception. */
  stop(): never {
    const error = new StopLoop();
    this.controller.abort(error);
    throw error;
  }
}

function verify(
  schema: DescMessage,
  message: Message,
  ctx: HandlerContext,
  key: string,
  method: string,
) {
  const timestamp = ctx.requestHeader.get("x-tilde-timestamp") ?? "";
  const signature = ctx.requestHeader.get("x-tilde-signature") ?? "";
  if (
    !/^\d+$/.test(timestamp) ||
    Math.abs(Date.now() / 1000 - Number(timestamp)) > 60 ||
    !/^[0-9a-f]{64}$/.test(signature)
  )
    throw new ConnectError("Invalid signature", Code.Unauthenticated);
  const digest = createHash("sha256").update(toBinary(schema, message)).digest("hex");
  const expected = createHmac("sha256", key).update(`${method}.${timestamp}.${digest}`).digest();
  if (!timingSafeEqual(expected, Buffer.from(signature, "hex")))
    throw new ConnectError("Invalid signature", Code.Unauthenticated);
}
/** Create a ConnectRPC agent service. The handler's return value is intentionally ignored. */
export function createAgentHandler(options: {
  signingKey: string;
  /** Prefix mounted by a serverless route, for example /api/agent. */
  pathPrefix?: string;
  /** Persist framework state before suspension; resume loads it in run(). */
  checkpoint?: (context: AgentContext) => Promise<void>;
  /** Existing OTel provider must include agentSpanProcessor. */
  tracing?: "existing";
  run: (context: AgentContext) => Promise<unknown>;
  /** Called by the Healthz RPC. Honor cancellation for dependency checks; throwing reports unhealthy. */
  healthz?: (signal: AbortSignal) => boolean | Promise<boolean>;
}): ReturnType<typeof connectNodeAdapter> {
  if (options.signingKey.length < 32) throw new Error("A caller-generated signing key is required");
  initializeTracing(options.tracing === "existing");
  const finished = new Set<string>();
  // A virtual conversation thread owns one reasoning loop at a time. Different
  // agent/thread pairs remain independent even when this host serves many agents.
  const virtualThreads = new Map<
    string,
    {
      invocationId: string;
      generation: bigint;
      controller: AbortController;
      completion: Promise<unknown>;
    }
  >();
  const handler = connectNodeAdapter({
    requestPathPrefix: options.pathPrefix,
    routes: (router) =>
      router.service(AgentHostService, {
        async *invoke(request, ctx) {
          verify(InvokeRequestSchema, request, ctx, options.signingKey, "Invoke");
          if (finished.has(request.invocationId))
            throw new ConnectError("Invocation already accepted", Code.AlreadyExists);
          const threadKey = `${request.agentId}:${request.threadId}`;
          for (;;) {
            const previous = virtualThreads.get(threadKey);
            if (!previous) break;
            if (previous.generation >= request.assignmentGeneration)
              throw new ConnectError(
                "Conversation already has an active invocation",
                Code.AlreadyExists,
              );
            previous.controller.abort(new StopLoop());
            await previous.completion;
            ctx.signal.throwIfAborted();
            if (virtualThreads.get(threadKey) === previous) virtualThreads.delete(threadKey);
            // Another request may have acquired the thread while we awaited.
          }
          const controller = new AbortController();
          const output = new Queue<{
            reasoningDelta?: string;
            stopped?: boolean;
          }>();
          controller.signal.addEventListener("abort", () => output.end(), {
            once: true,
          });
          const abort = () => controller.abort(ctx.signal.reason);
          ctx.signal.addEventListener("abort", abort, { once: true });
          const context = new AgentContext(request, controller, output);
          let resolveReady!: () => void;
          let rejectReady!: (error: unknown) => void;
          const ready = new Promise<void>((resolve, reject) => {
            resolveReady = resolve;
            rejectReady = reject;
          });
          const rejectAborted = () =>
            rejectReady(
              controller.signal.reason instanceof ConnectError
                ? controller.signal.reason
                : new ConnectError("Invocation is no longer active", Code.Canceled),
            );
          controller.signal.addEventListener("abort", rejectAborted, { once: true });
          const controls = context
            .consumeControls(resolveReady, options.checkpoint)
            .catch((error) => {
              rejectReady(error);
              output.end(ConnectError.from(error));
              controller.abort(error);
            });
          const telemetry = invocationTracing(request, ctx.requestHeader, () =>
            context.traceAuthorization(),
          );
          let traceFailed = false;
          const execution = telemetry.run(async () => {
            try {
              await ready;
              await context.refreshTools();
              controller.signal.throwIfAborted();
              await options.run(context);
              await context.settleSuspension();
              output.end();
            } catch (error) {
              traceFailed = !(error instanceof StopLoop || controller.signal.aborted);
              output.end(
                error instanceof StopLoop || controller.signal.aborted
                  ? undefined
                  : new ConnectError("Agent execution failed", Code.Internal),
              );
            }
          });
          virtualThreads.set(threadKey, {
            invocationId: request.invocationId,
            generation: request.assignmentGeneration,
            controller,
            completion: execution,
          });
          try {
            await ready;
            if (request.commandId) yield { acceptedCommandId: request.commandId };
            for await (const value of output.read()) yield value;
            finished.add(request.invocationId);
            yield { stopped: true, pendingInputIds: context.pendingInputIds() };
          } finally {
            controller.abort();
            output.end();
            finished.add(request.invocationId);
            if (finished.size > 4096) finished.delete(finished.values().next().value!);
            void execution.finally(() => {
              if (virtualThreads.get(threadKey)?.invocationId === request.invocationId)
                virtualThreads.delete(threadKey);
            });
            ctx.signal.removeEventListener("abort", abort);
            controller.signal.removeEventListener("abort", rejectAborted);
            void controls;
            void telemetry.end(traceFailed);
            void execution;
          }
        },
        async healthz(_request, ctx) {
          try {
            return { ready: options.healthz ? (await options.healthz(ctx.signal)) === true : true };
          } catch {
            return { ready: false };
          }
        },
      }),
  });
  // Plaintext HTTP/2 for localhost/reverse-proxy deployment. Terminate public TLS at the proxy.
  return handler;
}

/** Standalone host wrapper; serverless deployments export createAgentHandler(). */
export function createAgentServer(options: Parameters<typeof createAgentHandler>[0]) {
  return createServer(createAgentHandler(options));
}
