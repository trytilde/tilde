import { Code, ConnectError, createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";
import { create, toJson, type MessageInitShape } from "@bufbuild/protobuf";
import * as z from "zod";
import {
  ToolHostService,
  type ToolCallRequest,
  type VerifyRequest,
} from "@trytilde/contracts/tilde/tool_host/v1/tool_host_pb.js";
import {
  type OAuthConfigurationSchema,
  OAuthGrant,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";

/** Credential fields arrive as strings, so their schemas are `z.string()` or `z.enum()`. */
export type CredentialSchema = z.ZodObject<Record<string, z.ZodType<string, string>>>;
/** One way an instance authenticates, collected on Tilde's setup page. */
export type AuthMethod =
  | { name: string; schema: CredentialSchema }
  | {
      name: string;
      oauth: MessageInitShape<typeof OAuthConfigurationSchema>;
      grant?: "authorization_code" | "client_credentials" | "jwt_bearer";
      /** Fields collected beside the grant, such as a client ID and secret. */
      additionalSchema?: CredentialSchema;
    };
type MethodValues<M> = M extends { schema: infer S extends z.ZodType }
  ? z.output<S>
  : M extends { additionalSchema: infer S extends z.ZodType }
    ? { access_token: string } & z.output<S>
    : { access_token: string };
/** The credentials of an instance, as one of its methods' shapes. */
export type AuthValues<Methods extends Record<string, AuthMethod>> = {
  [K in keyof Methods]: MethodValues<Methods[K]>;
}[keyof Methods];

export type ToolContext<Auth> = {
  /** The agent's tool call ID: use it for upstream idempotency. */
  callId: string;
  agentId: string;
  threadId: string;
  /** The instance the call is made on, and the method it was set up with. */
  connectionId?: string;
  authMethod?: string;
  /** The instance's credentials; undefined for tools that need none. */
  auth: Auth;
  signal: AbortSignal;
};
/** A tool your own process runs. Names are host-local; agents see each as `{source slug}.{name}`. */
export type Tool<Input extends z.ZodType, Output extends z.ZodType, Auth> = {
  description: string;
  /** Short label shown in transcripts in place of the call's detail. */
  summary?: string;
  inputSchema: Input;
  /** Checked before the result reaches the agent. */
  outputSchema?: Output;
  annotations?: {
    readOnly?: boolean;
    destructive?: boolean;
    idempotent?: boolean;
    openWorld?: boolean;
  };
  run(input: z.output<Input>, ctx: ToolContext<Auth>): Promise<z.input<Output>> | z.input<Output>;
};
// oxlint-disable-next-line typescript/no-explicit-any -- tools of any shape share one table
type AnyTool = Tool<z.ZodType, z.ZodType, any>;
export type Tools = Record<string, AnyTool>;

/** Tools declared through an auth definition; only they receive an instance's credentials. */
const authTools = new WeakSet<object>();

/** A tool that needs no credentials. */
export function defineTool<Input extends z.ZodType, Output extends z.ZodType = z.ZodUnknown>(
  tool: Tool<Input, Output, undefined>,
) {
  return tool;
}

export type AuthDefinition<Methods extends Record<string, AuthMethod>> = {
  /** `id` is stable: it prefixes each instance's slug (`id/name`). */
  provider: {
    id: string;
    name: string;
    iconUrl?: string;
    instructions?: string;
    accountNameLabel?: string;
  };
  methods: Methods;
  /**
   * Runs at the end of an instance's setup, before it is ready. Throw to refuse the credentials:
   * the message is shown on the setup form. Return `accountLabel` to name the account they reach.
   */
  verify?(
    auth: AuthValues<Methods>,
    instance: { connectionId: string; method: keyof Methods & string },
  ): Promise<{ accountLabel?: string } | void>;
};
/**
 * Declares that the host's tools need credentials. Every connection of this provider is an
 * instance of the host: set up on Tilde's setup page, stored by Tilde, and sent with each call made
 * on it as `ctx.auth`. OAuth tokens are refreshed by Tilde before they reach the host.
 */
export function defineAuth<const Methods extends Record<string, AuthMethod>>(
  definition: AuthDefinition<Methods>,
) {
  return {
    ...definition,
    /** A tool whose `ctx.auth` holds the calling instance's credentials. */
    tool<Input extends z.ZodType, Output extends z.ZodType = z.ZodUnknown>(
      tool: Tool<Input, Output, AuthValues<Methods>>,
    ) {
      authTools.add(tool);
      return tool;
    },
  };
}
// oxlint-disable-next-line typescript/no-explicit-any -- the host only reads the definition
type AnyAuth = AuthDefinition<any>;

const grants = {
  authorization_code: OAuthGrant.AUTHORIZATION_CODE,
  client_credentials: OAuthGrant.CLIENT_CREDENTIALS,
  jwt_bearer: OAuthGrant.JWT_BEARER,
};
function jsonSchema(schema: z.ZodType, io: "input" | "output") {
  return JSON.stringify(z.toJSONSchema(schema, { io }));
}
/** Tilde renders credential forms from a flat schema that names every field it accepts. */
function credentialSchema(schema: CredentialSchema) {
  return JSON.stringify({
    ...z.toJSONSchema(schema, { io: "input" }),
    additionalProperties: false,
  });
}
function providerDefinition(auth: AnyAuth): MessageInitShape<typeof ProviderSchema> {
  return {
    ...auth.provider,
    connectionTypes: Object.entries(auth.methods as Record<string, AuthMethod>).map(
      ([id, method]) => ({
        id,
        name: method.name,
        credentialSource:
          "schema" in method
            ? { case: "static", value: { schemaJson: credentialSchema(method.schema) } }
            : {
                case: "oauth",
                value: {
                  configuration: method.oauth,
                  grant: grants[method.grant ?? "authorization_code"],
                  additionalSchemaJson: method.additionalSchema
                    ? credentialSchema(method.additionalSchema)
                    : undefined,
                },
              },
      }),
    ),
  };
}
function definitions(tools: Tools) {
  return Object.entries(tools).map(([name, tool]) => ({
    name,
    description: tool.description,
    summary: tool.summary ?? "",
    inputSchemaJson: jsonSchema(tool.inputSchema, "input"),
    outputSchemaJson: tool.outputSchema ? jsonSchema(tool.outputSchema, "output") : "",
    annotations: tool.annotations,
  }));
}
/** An instance's credentials in the shape of the method it was set up with. */
function credentials(
  auth: AnyAuth | undefined,
  method: string | undefined,
  fields: { key: string; value: string }[],
) {
  if (!auth || !method) return undefined;
  const values = Object.fromEntries(fields.map((field) => [field.key, field.value]));
  const definition = (auth.methods as Record<string, AuthMethod>)[method];
  if (!definition) throw new Error(`Unknown auth method ${method}`);
  if ("schema" in definition) return definition.schema.parse(values);
  const extra = definition.additionalSchema?.parse(values) ?? {};
  return { ...extra, access_token: values.access_token ?? "" };
}
function messageOf(error: unknown, fallback: string) {
  if (error instanceof z.ZodError) return z.prettifyError(error);
  return error instanceof Error ? error.message : fallback;
}
type Call = Pick<
  ToolCallRequest,
  "callId" | "name" | "inputJson" | "agentId" | "threadId" | "connectionId" | "connectionType"
> & { credentials?: { key: string; value: string }[] };
async function run(tools: Tools, auth: AnyAuth | undefined, call: Call, signal: AbortSignal) {
  const tool = Object.hasOwn(tools, call.name) ? tools[call.name] : undefined;
  if (!tool) return { error: `Unknown tool ${call.name}` };
  try {
    const needsAuth = authTools.has(tool);
    if (needsAuth && !call.connectionType)
      return { error: `${call.name} is only callable through an instance of its server` };
    const input = tool.inputSchema.parse(JSON.parse(call.inputJson));
    const output = await tool.run(input, {
      callId: call.callId,
      agentId: call.agentId,
      threadId: call.threadId,
      connectionId: call.connectionId,
      authMethod: call.connectionType,
      auth: needsAuth ? credentials(auth, call.connectionType, call.credentials ?? []) : undefined,
      signal,
    });
    const checked = tool.outputSchema ? tool.outputSchema.parse(output) : output;
    return { outputJson: JSON.stringify(checked ?? null) };
  } catch (error) {
    // The message reaches the agent; keep it free of secrets.
    return { error: messageOf(error, "Tool failed") };
  }
}
async function verify(
  auth: AnyAuth | undefined,
  request: Pick<VerifyRequest, "connectionId" | "connectionType"> & {
    credentials?: { key: string; value: string }[];
  },
): Promise<{ error: string } | { accountLabel?: string }> {
  try {
    const values = credentials(auth, request.connectionType, request.credentials ?? []);
    const result = await auth?.verify?.(values, {
      connectionId: request.connectionId,
      method: request.connectionType,
    });
    return { accountLabel: result?.accountLabel };
  } catch (error) {
    // Shown to the person entering the credentials; keep it free of secrets.
    return { error: messageOf(error, "The credentials were refused") };
  }
}

/**
 * Dial in to Tilde as a connected tool host: publish `tools` (and `auth`, when they need
 * credentials), run each call that arrives and answer it. The host is never dialed, so it needs
 * no public address. Reconnects until closed.
 */
export function createToolHost(options: {
  gatewayUrl?: string;
  token?: string;
  tools: Tools;
  auth?: AnyAuth;
}) {
  const baseUrl = options.gatewayUrl ?? process.env.TILDE_GATEWAY_URL;
  const token = options.token ?? process.env.TILDE_TOOL_HOST_TOKEN;
  if (!baseUrl || !token) throw new Error("A gateway URL and tool host token are required");
  const client = createClient(
    ToolHostService,
    createConnectTransport({ baseUrl, httpVersion: "2" }),
  );
  const headers = { authorization: `Bearer ${token}` };
  const closing = new AbortController();
  const connected = (async () => {
    let delay = 500;
    while (!closing.signal.aborted) {
      try {
        for await (const frame of client.watch(
          {
            tools: definitions(options.tools),
            provider: options.auth && providerDefinition(options.auth),
          },
          { headers, signal: closing.signal },
        )) {
          delay = 500;
          // Calls run concurrently; one slow tool must not hold up the stream.
          if (frame.frame.case === "verify") {
            const request = frame.frame.value;
            void verify(options.auth, request).then((outcome) =>
              client.respond(
                "error" in outcome
                  ? { callId: request.callId, outcome: { case: "error", value: outcome.error } }
                  : {
                      callId: request.callId,
                      outcome: { case: "outputJson", value: "{}" },
                      accountLabel: outcome.accountLabel,
                    },
                { headers },
              ),
            );
          }
          if (frame.frame.case !== "call") continue;
          const call = frame.frame.value;
          void run(options.tools, options.auth, call, closing.signal).then((outcome) =>
            client.respond(
              {
                callId: call.callId,
                outcome:
                  "error" in outcome
                    ? { case: "error", value: outcome.error ?? "" }
                    : { case: "outputJson", value: outcome.outputJson },
              },
              { headers },
            ),
          );
        }
      } catch (error) {
        if (closing.signal.aborted) return;
        if (error instanceof ConnectError && error.code === Code.Unauthenticated) throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, delay));
      delay = Math.min(delay * 2, 15_000);
    }
  })();
  return { closed: connected, close: () => closing.abort() };
}

/** An AWS Lambda handler for a Lambda tool host: answers Tilde's list, verify and call events. */
export function createToolLambdaHandler(options: { tools: Tools; auth?: AnyAuth }) {
  return async (event: {
    listTools?: unknown;
    verify?: Parameters<typeof verify>[1];
    call?: Call;
  }) => {
    if (event.call)
      return run(options.tools, options.auth, event.call, new AbortController().signal);
    if (event.verify) return verify(options.auth, event.verify);
    // Tilde reads the response as Protobuf JSON.
    return {
      tools: definitions(options.tools),
      provider:
        options.auth &&
        toJson(ProviderSchema, create(ProviderSchema, providerDefinition(options.auth))),
    };
  };
}
