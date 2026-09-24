import { ApiKeysService } from "@trytilde/contracts/tilde/management/v1/api_keys_pb.js";
import { IamService } from "@trytilde/contracts/tilde/management/v1/iam_pb.js";
import { IdentitiesService } from "@trytilde/contracts/tilde/management/v1/identities_pb.js";
import { createChannels } from "./channels.js";
import type { Channels } from "./channel-types.js";
export type * from "./channel-types.js";
import { createMessageClient, type MessageClient } from "./messages.js";
export type {
  ContextMessage,
  ConversationMessage,
  ObjectiveMessage,
  GoalMessage,
  TaskMessage,
  MessageHistory,
  MessageHistoryOptions,
  MessageClient,
} from "./messages.js";
import { DeploymentService } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { LogsService } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import {
  InvocationControlService,
  InvocationCommandKind,
} from "@trytilde/contracts/tilde/runtime/v1/controls_pb.js";
import { TracingService } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { AgentAccessService } from "@trytilde/contracts/tilde/management/v1/access_pb.js";
import { initializeTracing, invocationTracing, tracingInterceptor } from "./tracing.js";
export { agentSpanProcessor } from "./tracing.js";
import {
  initializeLogging,
  invocationLogging,
  agentLogProcessor,
  type DeploymentLoggingOptions,
} from "./logging.js";
export {
  agentLogProcessor,
  configureDeploymentLogging,
  type DeploymentLoggingOptions,
} from "./logging.js";
import {
  createClient as connectClient,
  Code,
  ConnectError,
  type Client as RpcClient,
} from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";
import { fromJson, type JsonValue, type MessageInitShape } from "@bufbuild/protobuf";
import { setTimeout as sleep } from "node:timers/promises";
import { createHash, randomUUID } from "node:crypto";
import { ConnectionsService } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { AgentService as ManagementAgentService } from "@trytilde/contracts/tilde/management/v1/agents_pb.js";
import { AgentService as RuntimeAgentService } from "@trytilde/contracts/tilde/runtime/v1/agents_pb.js";
import { TildeChatProviderService } from "@trytilde/contracts/tilde/management/v1/tilde_chat_pb.js";
import { ChatService as RuntimeChatService } from "@trytilde/contracts/tilde/runtime/v1/chat_pb.js";
import {
  InvokeRequestSchema,
  type InvokeRequest,
} from "@trytilde/contracts/tilde/agent_host/v1/agent_pb.js";
import {
  RunService,
  type ReportRequestSchema,
  type RunRegistered,
} from "@trytilde/contracts/tilde/run/v1/run_pb.js";
import {
  MessageSchema,
  type Participant,
  type Message as ChatMessage,
} from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
export * from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
export { RuntimeChatService, RunService };
export {
  BinaryPermission,
  TargetSelection,
  AgentConcurrencyPolicy,
} from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
/** Connection contracts are namespaced so their capabilities stay distinct from IAM grants. */
export * as management from "./management.js";
export * as runtime from "./runtime.js";
export { createTildeChatClient, type TildeChatClientOptions } from "./tilde-chat.js";

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
  readonly apiKeys: RpcClient<typeof ApiKeysService>;
  /** Groups, memberships and resource grants. */
  readonly iam: RpcClient<typeof IamService>;
  readonly identities: RpcClient<typeof IdentitiesService>;
  readonly access: RpcClient<typeof AgentAccessService>;
  readonly agents: RpcClient<typeof ManagementAgentService>;
  readonly tildeChat: RpcClient<typeof TildeChatProviderService>;
  readonly deployments: RpcClient<typeof DeploymentService>;
  readonly connections: RpcClient<typeof ConnectionsService>;
  readonly logs: RpcClient<typeof LogsService>;
  readonly traces: RpcClient<typeof TracingService>;
  constructor(options: ClientOptions) {
    const rpc = transport(options);
    this.apiKeys = connectClient(ApiKeysService, rpc);
    this.iam = connectClient(IamService, rpc);
    this.identities = connectClient(IdentitiesService, rpc);
    this.access = connectClient(AgentAccessService, rpc);
    this.agents = connectClient(ManagementAgentService, rpc);
    this.tildeChat = connectClient(TildeChatProviderService, rpc);
    this.deployments = connectClient(DeploymentService, rpc);
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
  readonly message: MessageClient;
  readonly channel: Channels;
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
  private readonly callbackUrl: string;
  private readonly controls: RpcClient<typeof InvocationControlService>;
  private readonly runService: RpcClient<typeof RunService>;
  private reports: Promise<void> = Promise.resolve();
  private stoppedReported = false;
  private suspension?: Promise<void>;
  async settleSuspension() {
    await this.suspension;
  }
  constructor(
    request: InvokeRequest,
    private readonly controller: AbortController,
    /** Internal host hook: ends the invocation with a failure the wake caller can observe. */
    private readonly fail: (error: ConnectError) => void,
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
    this.callbackUrl = request.callbackUrl;
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
    this.runService = connectClient(RunService, transport);
    const options = () => ({ headers: this.#headers, signal: this.signal });
    const renewal = setInterval(
      async () => {
        try {
          const renewed = await this.session.renewConnectToken(
            {},
            {
              ...options(),
              timeoutMs: 30_000,
            },
          );
          this.#headers.set("authorization", `Bearer ${renewed.token}`);
        } catch {
          this.controller.abort();
        }
      },
      // Connect tokens last five minutes; renew with one minute left for the RPC.
      4 * 60 * 1000,
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
    this.message = createMessageClient(this);
    const incoming = request.messages
      .filter(
        (message) =>
          message.delivery?.connectionId === request.thread?.channel?.connectionId &&
          !this.participants.some(
            (p) => p.id === message.participantId && p.agentId === this.agentId,
          ),
      )
      .at(-1);
    this.channel = createChannels(this.tools, request.thread?.channel, incoming);
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
    await this.report({ case: "reasoningDelta", value: text });
  }
  /**
   * Internal host hook: report one run event through tilde.run.v1.RunService with the
   * invocation capability. Reports are ordered and never cancelled by the invocation
   * signal (the final `stopped` outlives it). Acceptance and the end of the run decide
   * what the gateway records, so they retry a few times; a lost reasoning delta only
   * costs a UI update. `stopped` is sent at most once.
   */
  report(event: NonNullable<MessageInitShape<typeof ReportRequestSchema>["event"]>): Promise<void> {
    if (event.case === "stopped") {
      if (this.stoppedReported) return this.reports;
      this.stoppedReported = true;
    }
    const invocationId = this.invocationId;
    const attempts = event.case === "reasoningDelta" ? 1 : 4;
    this.reports = this.reports.then(async () => {
      for (let attempt = 1; ; attempt++) {
        try {
          await this.runService.report(
            { invocationId, event },
            { headers: this.#headers, timeoutMs: 10_000 },
          );
          return;
        } catch (error) {
          const connectError = ConnectError.from(error);
          // The gateway answered and refused: retrying cannot change that.
          const permanent = ![
            Code.Unavailable,
            Code.DeadlineExceeded,
            Code.Unknown,
            Code.Internal,
          ].includes(connectError.code);
          if (permanent || attempt >= attempts) {
            console.warn(
              `[tilde] run report (${event.case}) for invocation ${invocationId} failed:`,
              connectError.message,
            );
            return;
          }
          await sleep(250 * attempt).catch(() => {});
        }
      }
    });
    return this.reports;
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
  /**
   * Route a provider SDK through Tilde's inference gateway. `slug` is the connection's
   * `provider/account` name (for example `openai/prod`) or an alias this agent was given for
   * it (for example `fast`). Spread the result into the provider
   * factory: `createOpenAI(ctx.inference("openai/prod"))`. The gateway forwards the provider's
   * own wire format, swaps in the real credential and accounts the call; `fetch` carries the
   * current invocation token, so renewals apply to long-running loops.
   */
  inference(slug: string): { baseURL: string; apiKey: string; fetch: typeof fetch } {
    if (!/^([a-z0-9_]+\/)?[A-Za-z0-9._@+:-]+$/.test(slug))
      throw new Error("Inference slug must be provider/account or an alias");
    const baseURL = `${this.callbackUrl.replace(/\/$/, "")}/inference/${slug}`;
    const authorization = () => this.#headers.get("authorization")!;
    return {
      baseURL,
      apiKey: "tilde",
      fetch: (input, init) => {
        const headers = new Headers(
          init?.headers ?? (input instanceof Request ? input.headers : undefined),
        );
        headers.set("authorization", authorization());
        return fetch(input, { ...init, headers, signal: init?.signal ?? this.signal });
      },
    };
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
                    this.fail(new ConnectError("Suspension checkpoint failed", Code.Internal));
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
        // The server closes a healthy stream at its token's expiry. Reconnect
        // with the renewed token instead of treating the stream's age as an outage.
        connected = Date.now();
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

/** Options shared by every host shape: what to run and how to checkpoint it. */
export type InvocationOptions = {
  /** Persist framework state before suspension; resume loads it in run(). */
  checkpoint?: (context: AgentContext) => Promise<void>;
  /** Existing OTel provider must include agentSpanProcessor. */
  tracing?: "existing";
  /** Existing OTel LoggerProvider must include agentLogProcessor. */
  logging?: "existing";
  /** Export startup/background logs with deployment credentials (one deployment per process). */
  deploymentLogging?: DeploymentLoggingOptions;
  run: (context: AgentContext) => Promise<unknown>;
};
/**
 * Per-process state shared by every invocation a host executes, regardless of how it was
 * woken. A virtual conversation thread owns one reasoning loop at a time. Different
 * agent/thread pairs remain independent even when this host serves many agents.
 */
type HostState = {
  finished: Set<string>;
  virtualThreads: Map<
    string,
    {
      invocationId: string;
      generation: bigint;
      controller: AbortController;
      completion: Promise<unknown>;
    }
  >;
};
function createHostState(): HostState {
  return { finished: new Set(), virtualThreads: new Map() };
}

/**
 * Execute one wake to completion. Resolves when the invocation ends (normal return, stop or
 * suspension) and rejects with a ConnectError when it could not start or failed. Every
 * lifecycle event is reported through tilde.run.v1.RunService.Report with the invocation
 * capability; the wake caller never has to relay it. Authentication of the wake itself
 * (deployment token on Watch, cloud IAM) is the caller's concern. Aborting `signal` ends
 * the invocation, for example when the connected host closes.
 */
async function runInvocation(
  request: InvokeRequest,
  options: InvocationOptions,
  host: HostState,
  signal?: AbortSignal,
): Promise<void> {
  if (host.finished.has(request.invocationId))
    throw new ConnectError("Invocation already accepted", Code.AlreadyExists);
  const threadKey = `${request.agentId}:${request.threadId}`;
  for (;;) {
    const previous = host.virtualThreads.get(threadKey);
    if (!previous) break;
    if (previous.generation >= request.assignmentGeneration)
      throw new ConnectError("Conversation already has an active invocation", Code.AlreadyExists);
    previous.controller.abort(new StopLoop());
    await previous.completion;
    signal?.throwIfAborted();
    if (host.virtualThreads.get(threadKey) === previous) host.virtualThreads.delete(threadKey);
    // Another wake may have acquired the thread while we awaited.
  }
  const controller = new AbortController();
  let settle!: (error?: ConnectError) => void;
  const done = new Promise<void>((resolve, reject) => {
    settle = (error) => (error ? reject(error) : resolve());
  });
  done.catch(() => {});
  const abort = () => controller.abort(signal?.reason);
  signal?.addEventListener("abort", abort, { once: true });
  const context = new AgentContext(request, controller, settle);
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
  const controls = context.consumeControls(resolveReady, options.checkpoint).catch((error) => {
    rejectReady(error);
    settle(ConnectError.from(error));
    controller.abort(error);
  });
  // Wakes carry no request headers; the wake itself carries the trace context.
  const traceHeaders = new Headers();
  if (request.traceparent) {
    traceHeaders.set("traceparent", request.traceparent);
    if (request.tracestate) traceHeaders.set("tracestate", request.tracestate);
  }
  const telemetry = invocationTracing(request, traceHeaders, () => context.traceAuthorization());
  const logging = invocationLogging(request, telemetry.context, () => context.traceAuthorization());
  let traceFailed = false;
  let failure: string | undefined;
  const execution = logging.run(async () => {
    try {
      await ready;
      await context.refreshTools();
      controller.signal.throwIfAborted();
      await options.run(context);
      await context.settleSuspension();
      settle();
    } catch (error) {
      traceFailed = !(error instanceof StopLoop || controller.signal.aborted);
      if (traceFailed) failure = error instanceof Error ? error.message : String(error);
      settle(
        error instanceof StopLoop || controller.signal.aborted
          ? undefined
          : new ConnectError("Agent execution failed", Code.Internal),
      );
    }
  });
  host.virtualThreads.set(threadKey, {
    invocationId: request.invocationId,
    generation: request.assignmentGeneration,
    controller,
    completion: execution,
  });
  try {
    await ready;
    if (request.commandId)
      await context.report({ case: "accepted", value: { commandId: request.commandId } });
    await done;
  } finally {
    controller.abort();
    host.finished.add(request.invocationId);
    if (host.finished.size > 4096) host.finished.delete(host.finished.values().next().value!);
    void execution.finally(() => {
      if (host.virtualThreads.get(threadKey)?.invocationId === request.invocationId)
        host.virtualThreads.delete(threadKey);
    });
    signal?.removeEventListener("abort", abort);
    controller.signal.removeEventListener("abort", rejectAborted);
    void controls;
    // A failed run says so, so the gateway records a failed invocation instead of a stop.
    await context.report({
      case: "stopped",
      value: { pendingInputIds: context.pendingInputIds(), error: failure ?? "" },
    });
    // Keep the host request alive until final telemetry is flushed (including serverless hosts).
    await Promise.allSettled([logging.end(), telemetry.end(traceFailed)]);
  }
}

export type ConnectAgentOptions = Omit<InvocationOptions, "deploymentLogging"> & {
  /** Tilde's runtime listener root. Defaults to TILDE_GATEWAY_URL. */
  gatewayUrl?: string;
  /** Deployment token from the Deployments tab or RegisterDeployment. Defaults to TILDE_DEPLOYMENT_TOKEN. */
  deploymentToken?: string;
  /** Stable per-process identity; defaults to a random UUID. */
  instanceId?: string;
  /** Sent as `ready` in every heartbeat. Honor cancellation for dependency checks; throwing reports not ready. */
  ready?: (signal: AbortSignal) => boolean | Promise<boolean>;
  /** Receive registration details when the gateway acknowledges Watch. */
  onRegistered?: (registration: RunRegistered) => void;
};
export type ConnectedAgent = {
  readonly instanceId: string;
  /** Last registration acknowledged by the gateway, if any. */
  readonly registration: RunRegistered | undefined;
  /** Stop watching and heartbeating; active invocations are stopped and reported. */
  close(): Promise<void>;
};

/**
 * Long-running host: open RunService.Watch with the deployment token, run every wake frame
 * concurrently, heartbeat every 3 seconds and reconnect after 1 second whenever the stream
 * ends, until close() is called. Tilde never calls the host; it only dials out.
 */
export function connectAgent(options: ConnectAgentOptions): ConnectedAgent {
  const gatewayUrl = options.gatewayUrl ?? process.env.TILDE_GATEWAY_URL;
  const deploymentToken = options.deploymentToken ?? process.env.TILDE_DEPLOYMENT_TOKEN;
  if (!gatewayUrl || !deploymentToken)
    throw new Error(
      "connectAgent requires gatewayUrl and deploymentToken (or TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN)",
    );
  const host = createHostState();
  initializeTracing(options.tracing === "existing");
  try {
    initializeLogging(options.logging === "existing", { gatewayUrl, deploymentToken });
  } catch {
    // Startup/background logs belong to one deployment per process: the first host keeps them.
    // Invocation logs are routed per invocation and are unaffected.
    initializeLogging(options.logging === "existing");
    console.warn(
      "[tilde] deployment logs stay with the first connected deployment in this process",
    );
  }
  const instanceId = options.instanceId ?? randomUUID();
  const client = connectClient(
    RunService,
    createConnectTransport({
      baseUrl: gatewayUrl,
      httpVersion: "2",
      interceptors: [tracingInterceptor],
    }),
  );
  const headers = new Headers({ authorization: `Bearer ${deploymentToken}` });
  const closed = new AbortController();
  const active = new Set<Promise<void>>();
  let registration: RunRegistered | undefined;
  const isReady = async () => {
    try {
      return options.ready ? (await options.ready(closed.signal)) === true : true;
    } catch {
      return false;
    }
  };
  const heartbeat = async () => {
    try {
      await client.heartbeat(
        { instanceId, ready: await isReady() },
        { headers, signal: closed.signal, timeoutMs: 5_000 },
      );
    } catch (error) {
      if (!closed.signal.aborted)
        console.warn(
          `[tilde] heartbeat for instance ${instanceId} failed:`,
          ConnectError.from(error).message,
        );
    }
  };
  const heartbeatTimer = setInterval(() => void heartbeat(), 3_000);
  const watching = (async () => {
    while (!closed.signal.aborted) {
      try {
        for await (const frame of client.watch(
          { instanceId },
          { headers, signal: closed.signal },
        )) {
          switch (frame.frame.case) {
            case "registered":
              registration = frame.frame.value;
              await heartbeat();
              options.onRegistered?.(registration);
              break;
            case "wake": {
              const request = frame.frame.value;
              const invocation = runInvocation(request, options, host, closed.signal)
                .catch((error) => {
                  console.warn(
                    `[tilde] invocation ${request.invocationId} ended with an error:`,
                    ConnectError.from(error).message,
                  );
                })
                .finally(() => active.delete(invocation));
              active.add(invocation);
              break;
            }
            default:
              break;
          }
        }
      } catch (error) {
        if (closed.signal.aborted) break;
        console.warn(
          `[tilde] watch for instance ${instanceId} failed:`,
          ConnectError.from(error).message,
        );
      }
      if (closed.signal.aborted) break;
      await sleep(1_000, undefined, { signal: closed.signal }).catch(() => {});
    }
  })();
  return {
    instanceId,
    get registration() {
      return registration;
    },
    async close() {
      clearInterval(heartbeatTimer);
      closed.abort(new StopLoop());
      await watching;
      await Promise.allSettled(active);
      await agentLogProcessor.forceFlush();
    },
  };
}

/**
 * AWS Lambda host: the event is the JSON-encoded InvokeRequest delivered by the cloud invoke
 * API, which authenticates the wake. Runs the invocation to completion and reports through
 * RunService; failures are reported there and logged rather than rethrown, so the platform
 * does not retry an invocation that already ended.
 */
export function createLambdaHandler(options: InvocationOptions): (event: unknown) => Promise<void> {
  initializeTracing(options.tracing === "existing");
  initializeLogging(options.logging === "existing", options.deploymentLogging);
  const host = createHostState();
  return async (event) => {
    const request = fromJson(InvokeRequestSchema, event as JsonValue);
    try {
      await runInvocation(request, options, host);
    } catch (error) {
      console.warn(
        `[tilde] invocation ${request.invocationId} ended with an error:`,
        ConnectError.from(error).message,
      );
    }
  };
}
