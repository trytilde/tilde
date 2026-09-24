import { ChatService } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";

export interface ChatAgentConfig {
  agentId: string;
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
        !agent.apiKey.startsWith("tilde_chat_")
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
        if (path.length === 1 && path[0] === "agents" && request.method === "GET") {
          const listed = [];
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
            const data = (await response.json()) as { agents?: { id: string; name: string }[] };
            for (const entry of data.agents ?? [])
              if (entry.id === agent.agentId) listed.push({ id: entry.id, name: entry.name });
          }
          return listed.length
            ? Response.json({ agents: listed }, { headers: { "cache-control": "no-store" } })
            : failure(401, "unauthenticated", "Sign in to chat");
        }
        const agent =
          path.length === 3
            ? upstream.get(path[0])
            : agents.length === 1
              ? upstream.get(agents[0].agentId)
              : undefined;
        const [service, method] = path.slice(-2);
        if (
          !agent ||
          ![2, 3].includes(path.length) ||
          service !== ChatService.typeName ||
          !methods.has(method)
        )
          return failure(404, "not_found", "Unknown chat route");
        if (request.method !== "POST") return failure(405, "unimplemented", "Use ConnectRPC POST");
        if (
          !/^application\/(?:json|proto|connect\+json|connect\+proto)(?:;|$)/i.test(
            request.headers.get("content-type") ?? "",
          )
        )
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
