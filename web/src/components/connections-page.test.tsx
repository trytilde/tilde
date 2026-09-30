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
import {
  Capability,
  ConnectionSchema,
  McpCredential,
  OAuthClient,
  OAuthGrant,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { ToolHostSchema } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { ConnectionsPage } from "./connections-page";

const rpc = vi.hoisted(() => ({
  connections: {
    listProviders: vi.fn(),
    listConnections: vi.fn(),
    getProvider: vi.fn(),
    registerProvider: vi.fn(),
    disconnect: vi.fn(),
  },
  agents: { listAgents: vi.fn() },
  toolHosts: { listToolHosts: vi.fn() },
  tools: { listProviderTools: vi.fn(), refreshConnectionTools: vi.fn(), listToolSources: vi.fn() },
}));
vi.mock("@/client", () => rpc);
// What the server registers for "Wiki" with OAuth: a provider of its own, served over MCP.
const wiki = create(ProviderSchema, {
  id: "wiki",
  name: "Wiki",
  kind: { case: "configured", value: {} },
  connectionTypes: [
    {
      id: "oauth",
      name: "Sign in with OAuth",
      capabilities: [Capability.TOOL],
      mcpServer: { url: "https://wiki.example/mcp", credential: McpCredential.BEARER },
      credentialSource: {
        case: "oauth",
        value: {
          grant: OAuthGrant.AUTHORIZATION_CODE,
          configuration: { client: OAuthClient.DYNAMIC, pkce: true },
        },
      },
    },
  ],
});
beforeEach(() => {
  rpc.connections.listProviders.mockResolvedValue({ providers: [], nextPageToken: "" });
  rpc.connections.listConnections.mockResolvedValue({ connections: [], nextPageToken: "" });
  rpc.toolHosts.listToolHosts.mockResolvedValue({ toolHosts: [] });
  rpc.tools.listProviderTools.mockResolvedValue({ tools: [] });
  rpc.tools.listToolSources.mockResolvedValue({ sources: [] });
  rpc.agents.listAgents.mockResolvedValue({ agents: [], nextPageToken: "" });
});
function renderPage() {
  const root = createRootRoute({ component: () => <Outlet /> });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/tools/connections",
    component: ConnectionsPage,
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/tools/$toolId",
    component: () => <p>Detail</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([page, detail]),
    history: createMemoryHistory({ initialEntries: ["/tools/connections"] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}

it("groups connections by provider, each with the agents using it", async () => {
  rpc.connections.listProviders.mockResolvedValue({
    providers: [create(ProviderSchema, { id: "tavily", name: "Tavily" }), wiki],
    nextPageToken: "",
  });
  rpc.connections.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, {
        id: "c1",
        name: "Research",
        providerId: "tavily",
        status: "ready",
      }),
      create(ConnectionSchema, {
        id: "c2",
        name: "Support",
        providerId: "tavily",
        accountLabel: "support@acme.test",
        status: "requires_action",
      }),
      create(ConnectionSchema, { id: "c3", name: "Docs", providerId: "wiki", status: "ready" }),
      create(ConnectionSchema, { id: "c4", name: "EU", providerId: "crm", status: "ready" }),
    ],
    nextPageToken: "",
  });
  rpc.toolHosts.listToolHosts.mockResolvedValue({
    toolHosts: [create(ToolHostSchema, { id: "h1", name: "crm-host", providerId: "crm" })],
  });
  rpc.agents.listAgents.mockResolvedValue({
    agents: [
      { id: "a1", name: "Researcher" },
      { id: "a2", name: "Helper" },
    ],
    nextPageToken: "",
  });
  rpc.tools.listToolSources.mockImplementation(async ({ connectionId }) => ({
    sources:
      connectionId === "c1"
        ? [
            { id: "s1", agentId: "a1", connectionId },
            { id: "s2", agentId: "a2", connectionId },
          ]
        : [],
  }));
  renderPage();
  const table = await screen.findByRole("table", { name: "Tool connections" });
  const tavily = await within(table).findByRole("row", { name: "Provider Tavily" });
  expect(tavily.textContent).toContain("Catalog");
  expect(within(table).getByRole("row", { name: "Provider Wiki" }).textContent).toContain("MCP");
  // A remote server's provider opens the server's page.
  const crm = within(table).getByRole("row", { name: "Provider crm-host" });
  expect(within(crm).getByRole("link", { name: "crm-host" }).getAttribute("href")).toBe(
    "/tools/h1?kind=host",
  );

  const research = within(table).getByRole("link", { name: "Research" }).closest("tr")!;
  const stack = await within(research).findByRole("button", {
    name: "Agents: Researcher, Helper",
  });
  const support = within(table).getByRole("link", { name: "Support" }).closest("tr")!;
  expect(support.textContent).toContain("support@acme.test");
  expect(support.textContent).toContain("Setup pending");
  // One ListToolSources per listed connection: the API takes one connection at a time.
  expect(rpc.tools.listToolSources).toHaveBeenCalledTimes(4);

  fireEvent.click(stack);
  const agents = await screen.findByRole("dialog", { name: "Agents with Research" });
  expect(within(agents).getByRole("link", { name: "Researcher" }).getAttribute("href")).toBe(
    "/agent/a1/tools",
  );

  // A tab asks the server for one kind of provider.
  fireEvent.keyDown(agents, { key: "Escape" });
  fireEvent.click(screen.getByRole("tab", { name: "MCP" }));
  await waitFor(() =>
    expect(rpc.connections.listConnections).toHaveBeenLastCalledWith(
      expect.objectContaining({ source: ProviderSource.MCP_SERVER }),
      expect.anything(),
    ),
  );
});

it("disconnects a connection from its row only once confirmed", async () => {
  rpc.connections.listProviders.mockResolvedValue({
    providers: [create(ProviderSchema, { id: "tavily", name: "Tavily" })],
    nextPageToken: "",
  });
  rpc.connections.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, {
        id: "c1",
        name: "Research",
        providerId: "tavily",
        status: "ready",
      }),
    ],
    nextPageToken: "",
  });
  rpc.connections.disconnect.mockResolvedValue({});
  const router = renderPage();
  const trigger = await screen.findByRole("button", { name: "Research actions" });
  fireEvent.click(trigger);
  const item = await screen.findByRole("menuitem", { name: "Disconnect" });
  await waitFor(() => expect(item.getAttribute("aria-disabled")).toBeNull());
  fireEvent.click(item);
  const confirm = await screen.findByRole("alertdialog", { name: "Disconnect Research?" });
  expect(rpc.connections.disconnect).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "Disconnect" }));
  await waitFor(() => expect(rpc.connections.disconnect).toHaveBeenCalledWith({ id: "c1" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  // Confirming inside the row's dialog does not open the connection.
  expect(router.state.location.pathname).toBe("/tools/connections");
  expect(rpc.connections.listConnections).toHaveBeenCalledTimes(2);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("adds an MCP server as its own provider and sets up its first account", async () => {
  rpc.connections.getProvider.mockRejectedValue(new Error("Provider not found"));
  rpc.connections.registerProvider.mockResolvedValue({ provider: wiki });
  const root = createRootRoute({ component: () => <Outlet /> });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/tools/connections",
    component: ConnectionsPage,
  });
  const router = createRouter({
    routeTree: root.addChildren([page]),
    history: createMemoryHistory({ initialEntries: ["/tools/connections"] }),
  });
  render(<RouterProvider router={router} />);
  await screen.findByText(/No connections yet/);
  // The auth method is chosen from the button's menu before the dialog opens.
  fireEvent.click(screen.getByRole("button", { name: "Add MCP server" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Sign in with OAuth" }));
  const add = await screen.findByRole("dialog", { name: "Add MCP server" });
  fireEvent.change(within(add).getByLabelText("Name"), { target: { value: "Wiki" } });
  fireEvent.change(within(add).getByLabelText("Server URL"), {
    target: { value: "https://wiki.example/mcp" },
  });
  fireEvent.click(within(add).getByRole("button", { name: "Add server" }));
  // The first account's setup follows, without leaving Connections.
  await screen.findByRole("dialog", { name: /Add an? Wiki account/ });
  const [{ provider }] = rpc.connections.registerProvider.mock.calls[0];
  expect(provider.id).toBe("wiki");
  expect(provider.connectionTypes[0].mcpServer).toEqual({
    url: "https://wiki.example/mcp",
    credential: McpCredential.BEARER,
  });
  expect(provider.connectionTypes[0].credentialSource.value.configuration.client).toBe(
    OAuthClient.DYNAMIC,
  );
  expect(router.state.location.pathname).toBe("/tools/connections");
});
it("sets up an account on an existing MCP server found by a server-side search", async () => {
  rpc.connections.listProviders.mockImplementation(async (request) => ({
    providers:
      request.source === ProviderSource.MCP_SERVER && request.search === "wik" ? [wiki] : [],
    nextPageToken: "",
  }));
  const root = createRootRoute({ component: () => <Outlet /> });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/tools/connections",
    component: ConnectionsPage,
  });
  render(
    <RouterProvider
      router={createRouter({
        routeTree: root.addChildren([page]),
        history: createMemoryHistory({ initialEntries: ["/tools/connections"] }),
      })}
    />,
  );
  await screen.findByText(/No connections yet/);
  fireEvent.click(screen.getByRole("button", { name: "Add MCP server" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Choose existing" }));
  const dialog = await screen.findByRole("dialog", { name: "Choose an MCP server" });
  fireEvent.change(within(dialog).getByLabelText("Search servers"), { target: { value: "wik" } });
  fireEvent.click(await within(dialog).findByText("Wiki"));
  fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
  // Straight into the new account's setup, not the server's detail panel.
  await screen.findByRole("dialog", { name: /Add an? Wiki account/ });
});

it("searches connections on the server", async () => {
  renderPage();
  await screen.findByText(/No connections yet/);
  const search = screen.getByRole("combobox", { name: "Search connections" });
  fireEvent.change(search, { target: { value: "acme" } });
  fireEvent.submit(search.closest("form")!);
  await waitFor(() =>
    expect(rpc.connections.listConnections).toHaveBeenLastCalledWith(
      expect.objectContaining({ search: "acme" }),
      expect.anything(),
    ),
  );
});
