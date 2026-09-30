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
  OAuthClient,
  OAuthGrant,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { AuthDriver, BrokeringSchema } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";
import { ToolsPage } from "./tool-connections";

const rpc = vi.hoisted(() => ({
  connections: {
    listProviders: vi.fn(),
    listConnections: vi.fn(),
    startConnection: vi.fn(),
  },
  connectionSetup: {
    getSetup: vi.fn(),
    setConnectionName: vi.fn(),
    saveCredentials: vi.fn(),
    startOAuth: vi.fn(),
  },
  toolHosts: { listToolHosts: vi.fn() },
  tools: { listProviderTools: vi.fn() },
}));
vi.mock("@/client", () => rpc);

const tavily = create(ProviderSchema, {
  id: "tavily",
  name: "Tavily",
  instructions: "Search the web.",
  categories: ["search"],
  connectionTypes: [
    {
      id: "api",
      name: "Tavily API",
      capabilities: [Capability.TOOL],
      credentialSource: {
        case: "static",
        value: {
          schemaJson: JSON.stringify({
            type: "object",
            properties: { api_key: { type: "string", title: "API key" } },
            required: ["api_key"],
          }),
        },
      },
    },
  ],
});
// A client registered at setup (dynamic) needs no client fields.
let linearClient = OAuthClient.DYNAMIC;
const linear = create(ProviderSchema, {
  id: "linear",
  name: "Linear",
  instructions: "Plan and track issues.",
  categories: ["project_management"],
  connectionTypes: [
    {
      id: "oauth",
      name: "Linear OAuth",
      capabilities: [Capability.TOOL],
      credentialSource: {
        case: "oauth",
        value: {
          grant: OAuthGrant.AUTHORIZATION_CODE,
          configuration: { client: linearClient, tokenUrl: "https://linear.test/token" },
        },
      },
    },
  ],
});
function renderPage() {
  const root = createRootRoute({ component: () => <Outlet /> });
  const catalog = createRoute({
    getParentRoute: () => root,
    path: "/tools/catalog",
    component: ToolsPage,
  });
  const panel = createRoute({
    getParentRoute: () => catalog,
    path: "$providerId",
    component: () => null,
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/tools/$toolId",
    component: () => <p>Detail</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([catalog.addChildren([panel]), detail]),
    history: createMemoryHistory({ initialEntries: ["/tools/catalog"] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
const state = (init: Parameters<typeof create<typeof BrokeringSchema>>[1]) =>
  create(BrokeringSchema, { setupId: "s1", connectionId: "c9", ...init });
const form = (actionId: string, authDriver = AuthDriver.STATIC) =>
  state({ actionId, authDriver, connectionName: "Work", action: { case: "form", value: {} } });

beforeEach(() => {
  // The server filters; this stands in for it by the request's category and search.
  rpc.connections.listProviders.mockImplementation(
    async (request: { category?: string; search?: string }) => ({
      providers: [tavily, linear].filter(
        (p) =>
          (!request.category || p.categories.includes(request.category)) &&
          (!request.search || p.instructions?.includes(request.search)),
      ),
      categories: ["search", "project_management"],
      nextPageToken: "",
    }),
  );
  rpc.connections.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, { id: "c1", name: "Tavily", providerId: "tavily", status: "ready" }),
    ],
    nextPageToken: "",
  });
  rpc.connections.startConnection.mockResolvedValue({
    connection: create(ConnectionSchema, { id: "c9" }),
    brokeringUrl: "http://localhost:3000/connections/broker/s1?connection_setup_token=tok",
  });
  rpc.tools.listProviderTools.mockImplementation(async (request: { search?: string }) => ({
    tools: [{ name: "search" }, { name: "extract" }].filter(
      (tool) => !request.search || tool.name.includes(request.search),
    ),
  }));
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

it("groups providers by category and asks the server to filter by category and search", async () => {
  renderPage();
  const list = await screen.findByRole("region", { name: "Tool providers" });
  const headings = within(list)
    .getAllByRole("heading", { level: 3 })
    .filter((heading) => heading.id);
  expect(headings.map((heading) => heading.textContent)).toEqual(["Project management", "Search"]);
  // Remote servers' providers and URL-added MCP servers are left out by the server.
  const catalog = { capability: Capability.TOOL, source: ProviderSource.CATALOG };
  expect(rpc.connections.listProviders).toHaveBeenCalledWith(
    expect.objectContaining({ ...catalog, search: undefined, category: undefined }),
    expect.anything(),
  );
  // Which agents use a provider is not shown in the catalog.
  expect(within(list).queryByLabelText(/Enabled for/)).toBeNull();

  fireEvent.change(screen.getByPlaceholderText("Search tools"), { target: { value: " issues " } });
  await waitFor(() => expect(within(list).queryByText("Tavily")).toBeNull());
  expect(within(list).getByText("Linear")).toBeTruthy();
  expect(rpc.connections.listProviders).toHaveBeenLastCalledWith(
    expect.objectContaining({ ...catalog, search: "issues" }),
    expect.anything(),
  );

  fireEvent.change(screen.getByPlaceholderText("Search tools"), { target: { value: "" } });
  fireEvent.click(screen.getByRole("button", { name: "Category" }));
  // The menu lists the server's categories, whatever the current results.
  fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Search" }));
  expect(await within(list).findByText("Tavily")).toBeTruthy();
  expect(within(list).queryByText("Linear")).toBeNull();
  expect(rpc.connections.listProviders).toHaveBeenLastCalledWith(
    expect.objectContaining({ ...catalog, search: undefined, category: "search" }),
    expect.anything(),
  );

  fireEvent.change(screen.getByPlaceholderText("Search tools"), { target: { value: "nothing" } });
  expect(await within(list).findByText("No tools match these filters.")).toBeTruthy();
});

it("opens a provider's panel at its own path with its supported auth and every tool", async () => {
  const router = renderPage();
  fireEvent.click(await screen.findByRole("button", { name: /^Tavily/ }));
  const panel = await screen.findByRole("dialog", { name: "Tavily" });
  expect(router.state.location.pathname).toBe("/tools/catalog/tavily");
  // Accounts are managed from Connections, not the catalog.
  expect(within(panel).queryByRole("region", { name: "Accounts" })).toBeNull();
  expect(within(panel).getByText("Supported auth")).toBeTruthy();
  expect(within(panel).getByText("Tavily API")).toBeTruthy();
  const tools = await within(panel).findByRole("region", { name: "Available tools" });
  expect(await within(tools).findByText("search")).toBeTruthy();
  expect(within(tools).getByText("extract")).toBeTruthy();
  expect(rpc.tools.listProviderTools).toHaveBeenCalledWith(
    { connectionId: "", providerId: "tavily", search: undefined },
    expect.anything(),
  );
  fireEvent.change(within(tools).getByPlaceholderText("Search tools"), {
    target: { value: "extr" },
  });
  await waitFor(() => expect(within(tools).queryByText("search")).toBeNull());
  expect(rpc.tools.listProviderTools).toHaveBeenLastCalledWith(
    { connectionId: "", providerId: "tavily", search: "extr" },
    expect.anything(),
  );
  fireEvent.click(within(panel).getByRole("button", { name: "Close" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/tools/catalog"));
});

async function openSetup(provider: string) {
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${provider}`) }));
  const detail = await screen.findByRole("dialog");
  fireEvent.click(within(detail).getByRole("button", { name: /Add (new )?account/ }));
  return screen.findByRole("dialog", { name: new RegExp(`Add an? ${provider} account`) });
}

it("adds an account with its credentials and continues on its page", async () => {
  rpc.connectionSetup.getSetup.mockResolvedValue({ state: form("act1") });
  rpc.connectionSetup.saveCredentials.mockResolvedValue({
    state: state({ action: { case: "complete", value: {} } }),
  });
  const setup = await openSetup("Tavily");
  const connect = within(setup).getByRole("button", { name: "Connect" });
  expect((connect as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(within(setup).getByLabelText(/^Account name/), { target: { value: "Work" } });
  fireEvent.change(within(setup).getByLabelText("API key"), { target: { value: "tvly-1" } });
  expect((within(setup).getByLabelText("API key") as HTMLInputElement).type).toBe("password");
  fireEvent.click(connect);

  // The new account opens on its own page, where it is given to agents.
  expect(await screen.findByText("Detail")).toBeTruthy();
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(rpc.connections.startConnection).toHaveBeenCalledWith(
    expect.objectContaining({ name: "Work", providerId: "tavily", typeId: "api", assignments: [] }),
  );
  expect(rpc.connectionSetup.saveCredentials).toHaveBeenCalledWith({
    setupId: "s1",
    connectionSetupToken: "tok",
    actionId: "act1",
    fields: [{ key: "api_key", value: "tvly-1" }],
  });
});

it("opens the authorization page for OAuth and waits for it to complete", async () => {
  const open = vi.fn();
  vi.stubGlobal("open", open);
  rpc.connectionSetup.getSetup
    .mockResolvedValueOnce({ state: form("act1", AuthDriver.OAUTH_CODE) })
    .mockResolvedValueOnce({
      state: state({ action: { case: "redirect", value: { url: "https://linear.test/auth" } } }),
    })
    .mockResolvedValue({ state: state({ action: { case: "complete", value: {} } }) });
  rpc.connectionSetup.startOAuth.mockResolvedValue({
    state: state({ action: { case: "redirect", value: { url: "https://linear.test/auth" } } }),
  });
  const setup = await openSetup("Linear");
  // Linear's OAuth client needs no client fields.
  expect(within(setup).queryByLabelText(/Client ID/)).toBeNull();
  fireEvent.change(within(setup).getByLabelText(/^Account name/), { target: { value: "Work" } });
  fireEvent.click(within(setup).getByRole("button", { name: "Continue" }));

  expect(await within(setup).findByText(/Waiting for Linear authorization/)).toBeTruthy();
  expect(open).toHaveBeenCalledWith("https://linear.test/auth", "_blank", "noopener");
  expect(rpc.connectionSetup.startOAuth).toHaveBeenCalledWith(
    expect.objectContaining({ actionId: "act1", fields: [] }),
  );
  expect(await screen.findByText("Detail", {}, { timeout: 4000 })).toBeTruthy();
});

it("shows a refused key and resubmits it to the same setup", async () => {
  rpc.connectionSetup.getSetup
    .mockResolvedValueOnce({ state: form("act1") })
    .mockResolvedValueOnce({ state: form("act2") });
  rpc.connectionSetup.saveCredentials
    .mockRejectedValueOnce(new Error("The API key was rejected."))
    .mockResolvedValueOnce({ state: state({ action: { case: "complete", value: {} } }) });
  const setup = await openSetup("Tavily");
  fireEvent.change(within(setup).getByLabelText(/^Account name/), { target: { value: "Work" } });
  fireEvent.change(within(setup).getByLabelText("API key"), { target: { value: "bad" } });
  fireEvent.click(within(setup).getByRole("button", { name: "Connect" }));
  expect((await within(setup).findByRole("alert")).textContent).toBe("The API key was rejected.");

  fireEvent.change(within(setup).getByLabelText("API key"), { target: { value: "good" } });
  fireEvent.click(within(setup).getByRole("button", { name: "Connect" }));
  await screen.findByText("Detail");
  expect(rpc.connections.startConnection).toHaveBeenCalledTimes(1);
  expect(rpc.connectionSetup.saveCredentials).toHaveBeenLastCalledWith(
    expect.objectContaining({ actionId: "act2", fields: [{ key: "api_key", value: "good" }] }),
  );
});
