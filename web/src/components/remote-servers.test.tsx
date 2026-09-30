import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { ToolHostSchema, ToolHostType } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import {
  Capability,
  ConnectionSchema,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { RemoteServersPage } from "./remote-servers";

const rpc = vi.hoisted(() => ({
  connections: { listProviders: vi.fn(), listConnections: vi.fn(), getProvider: vi.fn() },
  toolHosts: { listToolHosts: vi.fn(), registerToolHost: vi.fn() },
  tools: { listProviderTools: vi.fn(), listMcpServerHealth: vi.fn(), listToolSources: vi.fn() },
  agents: { listAgents: vi.fn() },
}));
vi.mock("@/client", () => rpc);
function renderPage() {
  const root = createRootRoute({ component: () => <Outlet /> });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/tools/remote-servers",
    component: RemoteServersPage,
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/tools/$toolId",
    component: () => <p>Detail</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([page, detail]),
    history: createMemoryHistory({ initialEntries: ["/tools/remote-servers"] }),
  });
  return render(<RouterProvider router={router} />);
}
const hour = (index: number, failed: number) => ({
  hourStart: { seconds: BigInt(1_700_000_000 + index * 3600), nanos: 0 },
  totalChecks: 120,
  failedChecks: failed,
});
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({
    agents: [{ id: "a1", name: "Ops bot" }],
    nextPageToken: "",
  });
  // Only ops-host is used by an agent directly; crm-host is used through its instances.
  rpc.tools.listToolSources.mockImplementation(async ({ toolHostId }) => ({
    sources: toolHostId === "h2" ? [{ id: "s1", agentId: "a1", toolHostId: "h2" }] : [],
  }));
  rpc.tools.listMcpServerHealth.mockResolvedValue({
    servers: [
      {
        providerId: "wiki",
        healthHistory: Array.from({ length: 12 }, (_, index) => hour(index, index === 11 ? 1 : 0)),
      },
    ],
  });
  rpc.tools.listProviderTools.mockImplementation(async ({ providerId, toolHostId }) => ({
    tools:
      providerId === "wiki"
        ? [{ name: "search_pages" }, { name: "get_page" }]
        : toolHostId === "h2"
          ? [{ name: "restart" }]
          : [],
  }));
  rpc.connections.getProvider.mockResolvedValue({
    provider: create(ProviderSchema, {
      id: "acme-crm",
      name: "Acme CRM",
      instructions: "Customers and deals.",
      connectionTypes: [{ id: "api_key", name: "API key", capabilities: [Capability.TOOL] }],
    }),
  });
  // The server keeps only MCP servers added by URL, as the request's source asks.
  rpc.connections.listProviders.mockResolvedValue({
    providers: [
      create(ProviderSchema, {
        id: "wiki",
        name: "Team wiki",
        kind: { case: "configured", value: {} },
        connectionTypes: [
          {
            id: "oauth",
            name: "Sign in with OAuth",
            capabilities: [Capability.TOOL],
            mcpServer: { url: "https://wiki.example/mcp" },
          },
        ],
      }),
    ],
    nextPageToken: "",
  });
  rpc.connections.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, { id: "c1", name: "EU", providerId: "acme-crm", status: "ready" }),
    ],
    nextPageToken: "",
  });
  rpc.toolHosts.listToolHosts.mockResolvedValue({
    toolHosts: [
      create(ToolHostSchema, {
        id: "h1",
        name: "crm-host",
        type: ToolHostType.CONNECTED,
        providerId: "acme-crm",
        tools: [{ name: "search" }, { name: "update" }],
        authMethods: ["API key", "OAuth"],
        healthHistory: Array.from({ length: 12 }, (_, index) => hour(index, index === 11 ? 3 : 0)),
      }),
      create(ToolHostSchema, { id: "h2", name: "ops-host", tools: [{ name: "restart" }] }),
    ],
  });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("lists each server's health, tool count and auth methods", async () => {
  renderPage();
  const table = await screen.findByRole("table", { name: "Remote servers" });
  const [crm, ops, wiki] = within(table).getAllByRole("row").slice(1);
  expect(crm.textContent).toContain("crm-host");
  expect(within(crm).getAllByRole("cell")[2].textContent).toBe("2");
  expect(crm.textContent).toContain("API key, OAuth");
  expect(within(crm).getByRole("img").getAttribute("aria-label")).toBe(
    "Last 12 hours: 11 healthy, 0 degraded, 1 unhealthy, 0 with no data",
  );
  expect(within(ops).getAllByRole("cell")[2].textContent).toBe("1");
  expect(ops.textContent).toContain("None");
  // Tool hosts carry the agents using them, as connections do.
  await within(ops).findByRole("button", { name: "Agents: Ops bot" });
  expect(within(crm).queryByRole("button", { name: /^Agents:/ })).toBeNull();
  // An MCP server added by URL: its connections' discovered tools and its reachability history.
  expect(wiki.textContent).toContain("Team wiki");
  expect(within(wiki).getByRole("img").getAttribute("aria-label")).toBe(
    "Last 12 hours: 11 healthy, 0 degraded, 1 unhealthy, 0 with no data",
  );
  expect(within(wiki).getAllByRole("cell")[2].textContent).toBe("2");
  expect(wiki.textContent).toContain("Sign in with OAuth");

  expect(rpc.connections.listProviders).toHaveBeenCalledWith(
    expect.objectContaining({ source: ProviderSource.MCP_SERVER, search: undefined }),
    expect.anything(),
  );

  // A tab asks only for its kind of server.
  rpc.toolHosts.listToolHosts.mockClear();
  fireEvent.click(screen.getByRole("tab", { name: "MCP" }));
  await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(2));
  expect(within(table).getAllByRole("row")[1].textContent).toContain("Team wiki");
  expect(rpc.toolHosts.listToolHosts).not.toHaveBeenCalled();
  rpc.connections.listProviders.mockClear();
  fireEvent.click(screen.getByRole("tab", { name: "Tilde tool server" }));
  await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(3));
  expect(within(table).queryByText("Team wiki")).toBeNull();
  expect(rpc.connections.listProviders).not.toHaveBeenCalled();
});
it("searches servers by name on the server", async () => {
  renderPage();
  const table = await screen.findByRole("table", { name: "Remote servers" });
  rpc.toolHosts.listToolHosts.mockResolvedValue({
    toolHosts: [create(ToolHostSchema, { id: "h2", name: "ops-host" })],
  });
  rpc.connections.listProviders.mockResolvedValue({ providers: [], nextPageToken: "" });
  fireEvent.change(screen.getByPlaceholderText("Search remote servers"), {
    target: { value: " ops " },
  });
  await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(2));
  expect(within(table).getByText("ops-host")).toBeTruthy();
  expect(rpc.toolHosts.listToolHosts).toHaveBeenLastCalledWith(
    { search: "ops" },
    expect.anything(),
  );
  expect(rpc.connections.listProviders).toHaveBeenLastCalledWith(
    expect.objectContaining({ source: ProviderSource.MCP_SERVER, search: "ops" }),
    expect.anything(),
  );
});
it("registers a server and reveals its token once", async () => {
  rpc.toolHosts.registerToolHost.mockResolvedValue({
    toolHost: create(ToolHostSchema, { id: "h3", name: "billing" }),
    token: "host-secret",
  });
  renderPage();
  await screen.findByRole("table", { name: "Remote servers" });
  fireEvent.click(screen.getByRole("button", { name: "Add Tilde tool server" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Self-hosted (connects out)" }));
  fireEvent.change(await screen.findByLabelText("Name"), { target: { value: " billing " } });
  fireEvent.click(screen.getByRole("button", { name: "Add server" }));
  const token = await screen.findByLabelText("Tool host token");
  expect((token as HTMLInputElement).value).toBe("host-secret");
  expect(rpc.toolHosts.registerToolHost).toHaveBeenCalledWith({
    name: "billing",
    type: ToolHostType.CONNECTED,
    functionArn: undefined,
  });
  await waitFor(() => expect(rpc.toolHosts.listToolHosts).toHaveBeenCalledTimes(2));
});
it("opens a server as a catalog entry: its provider, or itself", async () => {
  renderPage();
  const table = await screen.findByRole("table", { name: "Remote servers" });
  fireEvent.click(within(table).getByText("crm-host"));
  let dialog = await screen.findByRole("dialog", { name: "Acme CRM" });
  expect(within(dialog).getByRole("button", { name: "Add account" })).toBeTruthy();
  expect(within(dialog).getByRole("link", { name: "Manage server" }).getAttribute("href")).toBe(
    "/tools/h1?kind=host",
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  // A server without a provider has no accounts; its page gives it to agents.
  fireEvent.click(within(table).getByText("ops-host"));
  dialog = await screen.findByRole("dialog", { name: "ops-host" });
  expect(within(dialog).queryByRole("button", { name: /Add (new )?account/ })).toBeNull();
  expect(await within(dialog).findByText("restart")).toBeTruthy();
  expect(within(dialog).getByRole("link", { name: "Manage server" }).getAttribute("href")).toBe(
    "/tools/h2?kind=host",
  );
});
