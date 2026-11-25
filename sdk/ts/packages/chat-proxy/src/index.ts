import { create, fromJson, toBinary, toJson, type JsonValue } from "@bufbuild/protobuf";
import {
  ChatService,
  ListAgentsResponseSchema,
  type ChatAgent,
} from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";

export interface ChatAgentConfig {
  agentId: string;
  /** The agent's chat key (`tilde_chat_...`), or a Tilde Cloud embed key that names the agent. */
  apiKey: string;
  /** Gateway origin. Defaults to the hosted Tilde API. */
  baseUrl?: string;
  /** Complete agent ingress base URL; replaces both origin and agent prefix. */
  baseUrlOverride?: string;
}
export type ChatProxyOptions = (ChatAgentConfig | { agents: readonly ChatAgentConfig[] }) & {
  /** Read your authenticated server session. Return null to deny access to this agent. */
  resolveIdentity: (request: Request, agentId: string) => string | null | Promise<string | null>;
  mountPath?: string;
  fetch?: typeof globalThis.fetch;
};
export interface ChatProxy {
  handle(request: Request, path?: readonly string[]): Promise<Response>;
}
const methods = new Set(ChatService.methods.map((method) => method.name));
const CONNECT_TYPE = /^application\/(?:json|proto|connect\+json|connect\+proto)(?:;|$)/i;
const failure = (status: number, code: string, message: string) =>
  Response.json({ code, message }, { status, headers: { "cache-control": "no-store" } });
const identityHeader = (identity: string) =>
  btoa(String.fromCharCode(...new TextEncoder().encode(identity)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

/** Server-only reverse proxy. Client bodies and Connect streams remain protocol-native. */
export function createChatProxy(options: ChatProxyOptions): ChatProxy {
  const agents = "agents" in options ? [...options.agents] : [options];
  if (!agents.length || new Set(agents.map((a) => a.agentId)).size !== agents.length)
    throw new Error("Configure distinct chat agents");
  const upstream = new Map(
    agents.map((agent) => {
      const base =
        agent.baseUrlOverride ??
        `${(agent.baseUrl ?? "https://api.trytilde.ai").replace(/\/$/, "")}/agents/${encodeURIComponent(agent.agentId)}`;
      const url = new URL(base);
      if (
        !["http:", "https:"].includes(url.protocol) ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        !(agent.apiKey.startsWith("tilde_chat_") || agent.apiKey.startsWith("tilde_embed_"))
      )
        throw new Error("Invalid chat provider configuration");
      return [agent.agentId, { ...agent, base: base.replace(/\/$/, "") }];
    }),
  );
  const fetcher = options.fetch ?? globalThis.fetch;
  const mount = (options.mountPath ?? "/api/chat").replace(/\/$/, "");
  async function headers(request: Request, agentId: string): Promise<Headers | null> {
    const identity = await options.resolveIdentity(request, agentId);
    if (!identity || Array.from(identity).length > 256) return null;
    const result = new Headers({
      authorization: `Bearer ${upstream.get(agentId)!.apiKey}`,
      "x-tilde-identity": identityHeader(identity),
    });
    for (const name of [
      "content-type",
      "connect-protocol-version",
      "connect-timeout-ms",
      "traceparent",
      "tracestate",
    ]) {
      const value = request.headers.get(name);
      if (value) result.set(name, value);
    }
    return result;
  }
  return {
    async handle(request, suppliedPath) {
      try {
        const url = new URL(request.url);
        const origin = request.headers.get("origin");
        if (origin && origin !== url.origin)
          return failure(403, "permission_denied", "Origin is not allowed");
        if (!suppliedPath && !url.pathname.startsWith(`${mount}/`))
          return failure(404, "not_found", "Unknown chat route");
        const path = suppliedPath
          ? [...suppliedPath]
          : url.pathname.slice(mount.length + 1).split("/");
        if (
          path.some(
            (part) => !part || !/^[a-zA-Z0-9_.-]+$/.test(part) || part === "." || part === "..",
          )
        )
          return failure(404, "not_found", "Unknown chat route");
        const [service, method] = path.slice(-2);
        const contentType = request.headers.get("content-type") ?? "";
        // `ChatService/ListAgents` at the mount itself: every configured agent this user may
        // chat with, each asked of its own ingress (the proxy's own answer, not one agent's).
        if (path.length === 2 && service === ChatService.typeName && method === "ListAgents") {
          if (request.method !== "POST") return failure(405, "unimplemented", "Use ConnectRPC POST");
          if (!CONNECT_TYPE.test(contentType))
            return failure(415, "invalid_argument", "Connect content type required");
          const listed: ChatAgent[] = [];
          for (const agent of upstream.values()) {
            const auth = await headers(request, agent.agentId);
            if (!auth) continue;
            auth.set("content-type", "application/json");
            auth.set("connect-protocol-version", "1");
            const response = await fetcher(`${agent.base}/${ChatService.typeName}/ListAgents`, {
              method: "POST",
              headers: auth,
              body: "{}",
              signal: request.signal,
              redirect: "manual",
              cache: "no-store",
            });
            if (!response.ok) return failure(502, "unavailable", "Chat provider is unavailable");
            const data = fromJson(ListAgentsResponseSchema, (await response.json()) as JsonValue, {
              ignoreUnknownFields: true,
            });
            for (const entry of data.agents) if (entry.id === agent.agentId) listed.push(entry);
          }
          if (!listed.length) return failure(401, "unauthenticated", "Sign in to chat");
          const answer = create(ListAgentsResponseSchema, { agents: listed });
          const binary = /^application\/(?:connect\+)?proto/i.test(contentType);
          return new Response(
            binary
              ? toBinary(ListAgentsResponseSchema, answer)
              : JSON.stringify(toJson(ListAgentsResponseSchema, answer)),
            {
              headers: {
                "content-type": binary ? "application/proto" : "application/json",
                "cache-control": "no-store",
              },
            },
          );
        }
        const agent =
          path.length === 3
            ? upstream.get(path[0])
            : agents.length === 1
              ? upstream.get(agents[0].agentId)
              : undefined;
        if (
          !agent ||
          ![2, 3].includes(path.length) ||
          service !== ChatService.typeName ||
          !methods.has(method)
        )
          return failure(404, "not_found", "Unknown chat route");
        if (request.method !== "POST") return failure(405, "unimplemented", "Use ConnectRPC POST");
        if (!CONNECT_TYPE.test(contentType))
          return failure(415, "invalid_argument", "Connect content type required");
        const auth = await headers(request, agent.agentId);
        if (!auth) return failure(401, "unauthenticated", "Sign in to chat");
        const init: RequestInit & { duplex: "half" } = {
          method: "POST",
          headers: auth,
          body: request.body,
          signal: request.signal,
          redirect: "manual",
          cache: "no-store",
          duplex: "half",
        };
        const response = await fetcher(`${agent.base}/${service}/${method}`, init);
        if (response.status >= 300 && response.status < 400)
          return failure(502, "unavailable", "Provider redirects are not supported");
        const outgoing = new Headers({ "cache-control": "no-store", "x-accel-buffering": "no" });
        for (const name of [
          "content-type",
          "connect-content-encoding",
          "connect-accept-encoding",
        ]) {
          const value = response.headers.get(name);
          if (value) outgoing.set(name, value);
        }
        return new Response(response.body, { status: response.status, headers: outgoing });
      } catch (error) {
        if (request.signal.aborted) throw error;
        return failure(502, "unavailable", "Chat request failed");
      }
    },
  };
}
