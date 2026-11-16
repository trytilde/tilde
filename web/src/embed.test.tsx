import { afterEach, expect, it, vi } from "vitest";
import { createEmbed } from "../../sdk/ts/packages/embed-server/src/index";
import { mountConnectionSetup, mountDashboard } from "../../sdk/ts/packages/embed/src/index";

/** Tilde's API as the embed server sees it: records each call, answers like Connect JSON. */
function fakeTilde() {
  const calls: { url: string; auth: string; identity: string | null; body: any }[] = [];
  const fetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const headers = new Headers(init?.headers);
    const body = init?.body ? JSON.parse(String(init.body)) : undefined;
    calls.push({
      url,
      auth: headers.get("authorization") ?? "",
      identity: headers.get("x-tilde-identity"),
      body,
    });
    if (url.endsWith(".EmbedService/OpenDashboard")) {
      if (body.user.groups.includes("blocked"))
        return Response.json({ code: "not_found", message: "Not found" }, { status: 404 });
      return Response.json({
        frameUrl: "https://env-1-dash.cloud.test/pipeline/?v=abc",
        token: `token-${calls.length}`,
        expiresAt: "1900000000",
        title: "Pipeline",
      });
    }
    if (url.endsWith(".ChatService/StartConnection"))
      return Response.json({
        setup: {
          connectionId: "conn-1",
          setupUrl: "https://cloud.test/connections/broker/conn-1#t",
          expiresAt: "1900000000",
        },
      });
    if (url.endsWith(".ChatService/ListConnections"))
      return Response.json({
        connections: [{ id: "conn-1", providerId: "linear", name: "acme", status: "ready" }],
      });
    return Response.json({ code: "not_found", message: "Unknown" }, { status: 404 });
  });
  return { calls, fetch };
}

afterEach(() => {
  document.body.innerHTML = "";
});

it("mounts a dashboard for the signed-in user through the app's own server", async () => {
  const tilde = fakeTilde();
  const embed = createEmbed({
    apiKey: "tilde_embed_secret",
    baseUrl: "https://api.cloud.test",
    fetch: tilde.fetch,
    agents: ["agent-1"],
    resolveIdentity: (request) =>
      request.headers.get("cookie") === "session=ann"
        ? { id: "ann", groups: ["acme"], attributes: { customer: "acme" } }
        : null,
  });
  // The browser package talks only to the app's server, which adds the user.
  const browserFetch = (async (input: RequestInfo | URL, init?: RequestInit) =>
    embed.handle(
      new Request(new URL(String(input), "https://app.test"), {
        ...init,
        headers: { ...(init?.headers as Record<string, string>), cookie: "session=ann" },
      }),
    )) as typeof fetch;
  const navigated: string[] = [];
  const element = document.createElement("div");
  document.body.appendChild(element);
  const mounted = mountDashboard(element, {
    environment: "env-1",
    dashboard: "pipeline",
    fetch: browserFetch,
    theme: { tokens: { accent: "#ff0000" }, scheme: "dark" },
    onNavigate: (to) => navigated.push(to),
  });
  await vi.waitFor(() => expect(element.querySelector("iframe")).not.toBeNull());
  const frame = mounted.frame;
  expect(frame.src).toBe("https://env-1-dash.cloud.test/pipeline/?v=abc");
  expect(frame.getAttribute("sandbox")).toBe("allow-scripts allow-forms");
  expect(tilde.calls[0]).toMatchObject({
    url: "https://api.cloud.test/tilde.ontology_control.v1.EmbedService/OpenDashboard",
    auth: "Bearer tilde_embed_secret",
    body: {
      environmentId: "env-1",
      dashboard: "pipeline",
      user: { id: "ann", groups: ["acme"], attributes: { customer: "acme" } },
    },
  });

  // The frame's messages are answered only for its own window: theme, then the first token.
  const posted: any[] = [];
  const frameWindow = frame.contentWindow!;
  vi.spyOn(frameWindow, "postMessage").mockImplementation((message: any) => posted.push(message));
  const fromFrame = (data: unknown) =>
    window.dispatchEvent(new MessageEvent("message", { data, source: frameWindow }));
  window.dispatchEvent(new MessageEvent("message", { data: { tilde: "ready" }, source: window }));
  expect(posted).toEqual([]);
  fromFrame({ tilde: "ready", dashboard: "pipeline", version: "abc" });
  await vi.waitFor(() => expect(posted).toHaveLength(2));
  expect(posted[0]).toEqual({ tilde: "theme", tokens: { accent: "#ff0000" }, scheme: "dark" });
  expect(posted[1]).toEqual({ tilde: "token", token: "token-1", expiresAt: 1900000000 });
  // A renewal asks the server again; resize and navigation reach the page.
  fromFrame({ tilde: "token-request" });
  await vi.waitFor(() => expect(posted).toHaveLength(3));
  expect(posted[2].token).toBe("token-2");
  fromFrame({ tilde: "resize", height: 812.4 });
  expect(frame.style.height).toBe("813px");
  fromFrame({ tilde: "navigate", to: "/customers/acme" });
  expect(navigated).toEqual(["/customers/acme"]);
  mounted.unmount();
  expect(element.querySelector("iframe")).toBeNull();

  // Without a session the server refuses before Tilde is called; a refused dashboard is a 404.
  const before = tilde.calls.length;
  const anonymous = await embed.handle(
    new Request("https://app.test/api/tilde/dashboards/session", {
      method: "POST",
      body: JSON.stringify({ environment: "env-1", dashboard: "pipeline" }),
    }),
  );
  expect(anonymous.status).toBe(401);
  expect(tilde.calls.length).toBe(before);
  const blocked = createEmbed({
    apiKey: "tilde_embed_secret",
    fetch: tilde.fetch,
    resolveIdentity: () => ({ id: "bob", groups: ["blocked"] }),
  });
  expect(
    await blocked.openDashboard({ id: "bob", groups: ["blocked"] }, "env-1", "pipeline"),
  ).toBeNull();
});

it("starts a connection setup for the user and reports completion from Tilde's page only", async () => {
  const tilde = fakeTilde();
  const embed = createEmbed({
    apiKey: "tilde_embed_secret",
    baseUrl: "https://api.cloud.test",
    fetch: tilde.fetch,
    agents: ["agent-1"],
    resolveIdentity: () => "ann",
  });
  const response = await embed.handle(
    new Request("https://app.test/api/tilde/connections/setup", {
      method: "POST",
      body: JSON.stringify({ provider: "linear" }),
    }),
  );
  const setup = await response.json();
  expect(setup).toEqual({
    connectionId: "conn-1",
    setupUrl: "https://cloud.test/connections/broker/conn-1#t",
    expiresAt: 1900000000,
  });
  const call = tilde.calls.at(-1)!;
  expect(call.url).toBe(
    "https://api.cloud.test/agents/agent-1/tilde.provider.tilde.v1.ChatService/StartConnection",
  );
  // The identity the agent's chats know the user by, asserted with the embed key.
  expect(atob(call.identity!.replace(/-/g, "+").replace(/_/g, "/"))).toBe("ann");
  expect(await embed.listConnections("ann", "agent-1")).toEqual([
    { id: "conn-1", providerId: "linear", name: "acme", status: "ready" },
  ]);

  const element = document.createElement("div");
  document.body.appendChild(element);
  const completed: string[] = [];
  const mounted = mountConnectionSetup(element, {
    url: setup.setupUrl,
    connectionId: setup.connectionId,
    onComplete: (id) => completed.push(id),
  });
  const frame = mounted.target as HTMLIFrameElement;
  const notify = (source: Window | null, connectionId: string) =>
    window.dispatchEvent(
      new MessageEvent("message", {
        data: { type: "tilde.connection.complete", connectionId },
        source,
      }),
    );
  notify(window, "conn-1");
  notify(frame.contentWindow, "conn-2");
  expect(completed).toEqual([]);
  notify(frame.contentWindow, "conn-1");
  expect(completed).toEqual(["conn-1"]);
  expect(element.querySelector("iframe")).toBeNull();
});
