import { createChatProxy, type ChatAgentConfig, type ChatProxy } from "@trytilde/chat-proxy";

/** Your signed-in user, as your server knows them. */
export interface EmbedUser {
  /** Stable and unique in your application (1 to 256 characters). */
  id: string;
  /** Dashboards: `principal in Group::"ext:<group>"` in the project's policies. */
  groups?: string[];
  /** Dashboards: Cedar tags and `ctx.principal.attrs` (`{ customer: "acme" }`). */
  attributes?: Record<string, string>;
}

export interface EmbedOptions {
  /** An embed key (`tilde_embed_...`) from the project's Embed tab in Tilde Cloud. */
  apiKey: string;
  /** Tilde's API origin. Defaults to the hosted Tilde API. */
  baseUrl?: string;
  /** Read your authenticated server session. Return null to refuse the request. */
  resolveIdentity: (
    request: Request,
  ) => EmbedUser | string | null | Promise<EmbedUser | string | null>;
  /** Agents whose chats the browser may open (each must be on the embed key). */
  agents?: readonly string[];
  /** Where `handle` is mounted. Defaults to `/api/tilde`. */
  mountPath?: string;
  fetch?: typeof globalThis.fetch;
}

export interface DashboardSession {
  frameUrl: string;
  token: string;
  /** Unix seconds. */
  expiresAt: number;
  title: string;
}

export interface OntologyToken {
  token: string;
  /** Unix seconds. */
  expiresAt: number;
  /** Where the token is accepted (the environment's gateway). */
  gatewayUrl: string;
}

export interface ConnectionSetup {
  connectionId: string;
  /** Tilde's setup page: mount it with `mountConnectionSetup` from `@trytilde/embed`. */
  setupUrl: string;
  /** Unix seconds. */
  expiresAt: number;
}

export interface UserConnection {
  id: string;
  providerId: string;
  name: string;
  /** `pending`, `ready`, `failed` or `disconnected`. */
  status: string;
}

export interface Embed {
  /**
   * Serve the browser package's requests below `mountPath`:
   * `POST dashboards/session`, `POST connections/setup`, `GET connections`,
   * `POST connections/<id>/reconnect`, `DELETE connections/<id>`, and the chat proxy under
   * `chat/`. Every request is for the user `resolveIdentity` returns.
   */
  handle(request: Request, path?: readonly string[]): Promise<Response>;
  /** A dashboard frame and token for `user`. Null when the project's policies refuse it. */
  openDashboard(
    user: EmbedUser,
    environment: string,
    dashboard: string,
  ): Promise<DashboardSession | null>;
  /** A token for `user` at the environment's gateway, to call the ontology's API yourself. */
  ontologyToken(user: EmbedUser, environment: string): Promise<OntologyToken>;
  /** A short-lived setup page where `user` connects their own account for `provider`. */
  createConnectionSetup(options: {
    user: EmbedUser | string;
    agent: string;
    provider: string;
  }): Promise<ConnectionSetup>;
  listConnections(user: EmbedUser | string, agent: string): Promise<UserConnection[]>;
  reconnectConnection(
    user: EmbedUser | string,
    agent: string,
    connectionId: string,
  ): Promise<ConnectionSetup>;
  deleteConnection(user: EmbedUser | string, agent: string, connectionId: string): Promise<void>;
}

export class EmbedError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

const EMBED = "tilde.ontology_control.v1.EmbedService";
const CHAT = "tilde.provider.tilde.v1.ChatService";
const failure = (status: number, code: string, message: string) =>
  Response.json({ code, message }, { status, headers: { "cache-control": "no-store" } });
const identityHeader = (identity: string) =>
  btoa(String.fromCharCode(...new TextEncoder().encode(identity)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
const asUser = (user: EmbedUser | string): EmbedUser =>
  typeof user === "string" ? { id: user } : user;
const seconds = (value: unknown) => Number(value ?? 0);
const text = (value: unknown) => (typeof value === "string" ? value : "");
const setupOf = (response: Record<string, unknown>): ConnectionSetup => {
  const data = (response.setup ?? {}) as Record<string, unknown>;
  return {
    connectionId: text(data.connectionId),
    setupUrl: text(data.setupUrl),
    expiresAt: seconds(data.expiresAt),
  };
};

/** Server-only: your embed key never reaches the browser. */
export function createEmbed(options: EmbedOptions): Embed {
  if (!options.apiKey.startsWith("tilde_embed_"))
    throw new Error("Use an embed key (tilde_embed_...)");
  const base = new URL(options.baseUrl ?? "https://api.trytilde.ai");
  if (!["http:", "https:"].includes(base.protocol) || base.username || base.search || base.hash)
    throw new Error("Invalid Tilde base URL");
  const origin = base.toString().replace(/\/$/, "");
  const fetcher = options.fetch ?? globalThis.fetch;
  const mount = (options.mountPath ?? "/api/tilde").replace(/\/$/, "");
  const agents = [...(options.agents ?? [])];

  async function call(
    url: string,
    body: unknown,
    identity?: string,
  ): Promise<Record<string, unknown>> {
    const headers = new Headers({
      authorization: `Bearer ${options.apiKey}`,
      "content-type": "application/json",
      "connect-protocol-version": "1",
    });
    if (identity !== undefined) headers.set("x-tilde-identity", identityHeader(identity));
    const response = await fetcher(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      redirect: "manual",
      cache: "no-store",
    });
    const data = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    if (!response.ok)
      throw new EmbedError(
        response.status,
        text(data.code) || "unavailable",
        text(data.message) || "Tilde request failed",
      );
    return data;
  }
  const embedCall = (method: string, body: unknown) => call(`${origin}/${EMBED}/${method}`, body);
  const chatCall = (agent: string, user: EmbedUser | string, method: string, body: unknown) =>
    call(`${origin}/agents/${encodeURIComponent(agent)}/${CHAT}/${method}`, body, asUser(user).id);
  const wire = (user: EmbedUser) => ({
    id: user.id,
    groups: user.groups ?? [],
    attributes: user.attributes ?? {},
  });

  const embed: Omit<Embed, "handle"> = {
    async openDashboard(user, environment, dashboard) {
      try {
        const data = await embedCall("OpenDashboard", {
          environmentId: environment,
          dashboard,
          user: wire(user),
        });
        return {
          frameUrl: text(data.frameUrl),
          token: text(data.token),
          expiresAt: seconds(data.expiresAt),
          title: text(data.title),
        };
      } catch (error) {
        if (error instanceof EmbedError && error.code === "not_found") return null;
        throw error;
      }
    },
    async ontologyToken(user, environment) {
      const data = await embedCall("IssueOntologyToken", {
        environmentId: environment,
        user: wire(user),
      });
      return {
        token: text(data.token),
        expiresAt: seconds(data.expiresAt),
        gatewayUrl: text(data.gatewayUrl),
      };
    },
    async createConnectionSetup({ user, agent, provider }) {
      return setupOf(await chatCall(agent, user, "StartConnection", { provider }));
    },
    async listConnections(user, agent) {
      const data = await chatCall(agent, user, "ListConnections", {});
      return ((data.connections as Record<string, unknown>[] | undefined) ?? []).map((c) => ({
        id: text(c.id),
        providerId: text(c.providerId),
        name: text(c.name),
        status: text(c.status),
      }));
    },
    async reconnectConnection(user, agent, connectionId) {
      return setupOf(await chatCall(agent, user, "ReconnectConnection", { connectionId }));
    },
    async deleteConnection(user, agent, connectionId) {
      await chatCall(agent, user, "DeleteConnection", { connectionId });
    },
  };

  let chat: ChatProxy | undefined;
  let resolving: { request: Request; user: Promise<EmbedUser | null> } | undefined;
  const identify = (request: Request): Promise<EmbedUser | null> => {
    // One resolution per request, shared by the chat proxy's per-agent lookups.
    if (resolving?.request !== request)
      resolving = {
        request,
        user: Promise.resolve(options.resolveIdentity(request)).then((user) =>
          user === null ? null : asUser(user),
        ),
      };
    return resolving.user;
  };
  if (agents.length) {
    const configs: ChatAgentConfig[] = agents.map((agentId) => ({
      agentId,
      apiKey: options.apiKey,
      baseUrl: origin,
    }));
    chat = createChatProxy({
      agents: configs,
      mountPath: `${mount}/chat`,
      fetch: fetcher,
      resolveIdentity: async (request) => (await identify(request))?.id ?? null,
    });
  }

  const agentOf = (value: unknown) => {
    const agent = typeof value === "string" ? value : agents.length === 1 ? agents[0] : "";
    return agents.includes(agent) ? agent : null;
  };
  const json = async (request: Request) =>
    (await request.json().catch(() => ({}))) as Record<string, unknown>;
  const answer = (data: unknown) =>
    Response.json(data, { headers: { "cache-control": "no-store" } });

  async function route(request: Request, path: string[], user: EmbedUser): Promise<Response> {
    const method = request.method;
    if (method === "POST" && path.join("/") === "dashboards/session") {
      const body = await json(request);
      if (typeof body.environment !== "string" || typeof body.dashboard !== "string")
        return failure(400, "invalid_argument", "environment and dashboard are required");
      const opened = await embed.openDashboard(user, body.environment, body.dashboard);
      return opened ? answer(opened) : failure(404, "not_found", "Dashboard not found");
    }
    if (path[0] !== "connections") return failure(404, "not_found", "Unknown route");
    const url = new URL(request.url);
    if (method === "GET" && path.length === 1) {
      const agent = agentOf(url.searchParams.get("agent"));
      if (!agent) return failure(400, "invalid_argument", "Unknown agent");
      return answer({ connections: await embed.listConnections(user, agent) });
    }
    if (method === "POST" && path.length === 2 && path[1] === "setup") {
      const body = await json(request);
      const agent = agentOf(body.agent);
      if (!agent || typeof body.provider !== "string")
        return failure(400, "invalid_argument", "agent and provider are required");
      return answer(await embed.createConnectionSetup({ user, agent, provider: body.provider }));
    }
    const agent = agentOf(url.searchParams.get("agent"));
    if (!agent) return failure(400, "invalid_argument", "Unknown agent");
    if (method === "POST" && path.length === 3 && path[2] === "reconnect")
      return answer(await embed.reconnectConnection(user, agent, path[1]));
    if (method === "DELETE" && path.length === 2) {
      await embed.deleteConnection(user, agent, path[1]);
      return answer({});
    }
    return failure(404, "not_found", "Unknown route");
  }

  return {
    ...embed,
    async handle(request, suppliedPath) {
      try {
        const url = new URL(request.url);
        const origin = request.headers.get("origin");
        if (origin && origin !== url.origin)
          return failure(403, "permission_denied", "Origin is not allowed");
        if (!suppliedPath && !url.pathname.startsWith(`${mount}/`))
          return failure(404, "not_found", "Unknown route");
        const path = suppliedPath
          ? [...suppliedPath]
          : url.pathname.slice(mount.length + 1).split("/");
        if (
          path.some(
            (part) => !part || !/^[a-zA-Z0-9_.-]+$/.test(part) || part === "." || part === "..",
          )
        )
          return failure(404, "not_found", "Unknown route");
        if (path[0] === "chat") {
          if (!chat) return failure(404, "not_found", "No agents are configured");
          return chat.handle(request, suppliedPath ? path.slice(1) : undefined);
        }
        const user = await identify(request);
        if (!user) return failure(401, "unauthenticated", "Sign in first");
        return await route(request, path, user);
      } catch (error) {
        if (request.signal.aborted) throw error;
        if (error instanceof EmbedError) return failure(error.status, error.code, error.message);
        return failure(502, "unavailable", "Tilde request failed");
      }
    },
  };
}
