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
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { AuthDriver, BrokeringSchema } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";
import { ToolHostSchema, ToolHostType } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { ToolConnectionDetail } from "./tool-connection-detail";
import { DashboardBreadcrumbProvider } from "./dashboard-breadcrumbs";
import { SiteHeader } from "./site-header";
import { SidebarProvider } from "./ui/sidebar";
import { TooltipProvider } from "./ui/tooltip";

const rpc = vi.hoisted(() => ({
  connections: {
    disconnect: vi.fn(),
    getConnection: vi.fn(),
    getProvider: vi.fn(),
    reconnect: vi.fn(),
    startConnection: vi.fn(),
  },
  connectionSetup: { getSetup: vi.fn(), setConnectionName: vi.fn(), saveCredentials: vi.fn() },
  toolHosts: { listToolHosts: vi.fn(), rotateToolHostToken: vi.fn(), deleteToolHost: vi.fn() },
  tools: { listToolSources: vi.fn(), listProviderTools: vi.fn() },
}));
vi.mock("@/client", () => rpc);

beforeEach(() => {
  // The header's sidebar trigger reads a media query for its mobile breakpoint.
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }),
  );
  rpc.connections.getConnection.mockResolvedValue({
    connection: create(ConnectionSchema, {
      id: "c1",
      name: "Tavily",
      providerId: "tavily",
      typeId: "api",
      status: "ready",
    }),
  });
  rpc.connections.getProvider.mockResolvedValue({
    provider: create(ProviderSchema, {
      id: "tavily",
      name: "Tavily",
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
    }),
  });
  rpc.tools.listProviderTools.mockImplementation(async ({ search }) => ({
    tools: [
      { name: "search", description: "Search the web" },
      { name: "extract", description: "Read a page" },
    ].filter((tool) => !search || tool.name.includes(search)),
  }));
  rpc.toolHosts.listToolHosts.mockResolvedValue({
    toolHosts: [
      create(ToolHostSchema, {
        id: "h1",
        name: "ops-host",
        type: ToolHostType.CONNECTED,
        available: true,
      }),
    ],
  });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

/** The detail page under the dashboard's header, which shows its breadcrumbs. */
function renderPage(kind: "connection" | "host" = "connection") {
  const id = kind === "connection" ? "c1" : "h1";
  const root = createRootRoute({
    component: () => (
      <DashboardBreadcrumbProvider>
        <TooltipProvider>
          <SidebarProvider>
            <div>
              <SiteHeader />
              <Outlet />
            </div>
          </SidebarProvider>
        </TooltipProvider>
      </DashboardBreadcrumbProvider>
    ),
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/tools/$toolId",
    component: () => <ToolConnectionDetail id={id} kind={kind} />,
  });
  const lists = ["/tools/connections", "/tools/remote-servers"].map((path) =>
    createRoute({ getParentRoute: () => root, path, component: () => <p>{path}</p> }),
  );
  const router = createRouter({
    routeTree: root.addChildren([detail, ...lists]),
    history: createMemoryHistory({ initialEntries: [`/tools/${id}?kind=${kind}`] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
function crumbs() {
  const nav = screen.getByRole("navigation", { name: "Breadcrumb" });
  return within(nav)
    .getAllByRole("listitem")
    .filter((item) => item.textContent !== "/")
    .map((item) => [item.textContent, item.querySelector("a")?.getAttribute("href") ?? null]);
}

it("names the connection in the breadcrumbs, leaving agents to the table", async () => {
  renderPage();
  await screen.findByRole("heading", { name: "Tavily" });
  await waitFor(() =>
    expect(crumbs()).toEqual([
      ["Tools", "/tools/connections"],
      ["Connections", "/tools/connections"],
      ["Tavily", null],
    ]),
  );
  // No back button or agent list: those live on the Connections table.
  const [back] = screen.getAllByRole("link", { name: "Connections" });
  expect(screen.getAllByRole("link", { name: "Connections" })).toHaveLength(1);
  expect(back.closest("nav")?.getAttribute("aria-label")).toBe("Breadcrumb");
  expect(screen.queryByRole("region", { name: "Used by" })).toBeNull();
  expect(rpc.tools.listToolSources).not.toHaveBeenCalled();
});

it("lists a connection's tools read-only", async () => {
  renderPage();
  const tools = await screen.findByRole("region", { name: "Available tools" });
  await within(tools).findByText("Search the web");
  within(tools).getByText("extract");
  expect(within(tools).queryByRole("switch")).toBeNull();
  // Searching asks the server again.
  fireEvent.change(within(tools).getByPlaceholderText("Search tools"), {
    target: { value: "extr" },
  });
  await waitFor(() => expect(within(tools).queryByText("Search the web")).toBeNull());
  expect(rpc.tools.listProviderTools).toHaveBeenLastCalledWith(
    { connectionId: "c1", search: "extr" },
    expect.anything(),
  );
});

it("disconnects only once confirmed, then returns to Connections", async () => {
  rpc.connections.disconnect.mockResolvedValue({});
  const router = renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Disconnect" }));
  let dialog = await screen.findByRole("alertdialog", { name: "Disconnect Tavily?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(rpc.connections.disconnect).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
  dialog = await screen.findByRole("alertdialog", { name: "Disconnect Tavily?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Disconnect" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/tools/connections"));
  expect(rpc.connections.disconnect).toHaveBeenCalledWith({ id: "c1" });
});

it("confirms rotating a tool host's token and deleting the host", async () => {
  rpc.toolHosts.rotateToolHostToken.mockResolvedValue({ token: "tth_new" });
  rpc.toolHosts.deleteToolHost.mockResolvedValue({});
  const router = renderPage("host");
  await screen.findByRole("heading", { name: "ops-host" });
  await waitFor(() =>
    expect(crumbs()).toEqual([
      ["Tools", "/tools/connections"],
      ["Remote servers", "/tools/remote-servers"],
      ["ops-host", null],
    ]),
  );

  fireEvent.click(screen.getByRole("button", { name: "Rotate token" }));
  let confirm = await screen.findByRole("alertdialog", { name: "Rotate ops-host's token?" });
  expect(rpc.toolHosts.rotateToolHostToken).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "Rotate token" }));
  const shown = await screen.findByRole("dialog", { name: "New tool host token" });
  expect((within(shown).getByLabelText("Tool host token") as HTMLInputElement).value).toBe(
    "tth_new",
  );
  expect(rpc.toolHosts.rotateToolHostToken).toHaveBeenCalledWith({ id: "h1" });
  fireEvent.keyDown(shown, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  confirm = await screen.findByRole("alertdialog", { name: "Delete ops-host?" });
  expect(rpc.toolHosts.deleteToolHost).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/tools/remote-servers"));
  expect(rpc.toolHosts.deleteToolHost).toHaveBeenCalledWith({ id: "h1" });
});

it("reconnects with the catalog's setup form, keeping the account and its method", async () => {
  const setup = { setupId: "s1", connectionId: "c1" };
  rpc.connections.reconnect.mockResolvedValue({
    brokeringUrl: "http://localhost/connections/broker/s1?connection_setup_token=tok",
  });
  rpc.connectionSetup.getSetup.mockResolvedValue({
    state: create(BrokeringSchema, {
      ...setup,
      actionId: "act1",
      authDriver: AuthDriver.STATIC,
      connectionName: "Tavily",
      action: { case: "form", value: {} },
    }),
  });
  rpc.connectionSetup.saveCredentials.mockResolvedValue({
    state: create(BrokeringSchema, { ...setup, action: { case: "complete", value: {} } }),
  });
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Reconnect" }));
  const dialog = await screen.findByRole("dialog", { name: "Reconnect Tavily" });
  expect((within(dialog).getByLabelText(/^Account name/) as HTMLInputElement).value).toBe("Tavily");
  fireEvent.change(within(dialog).getByLabelText("API key"), { target: { value: "tvly-new" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(rpc.connections.reconnect).toHaveBeenCalledWith({ id: "c1" });
  expect(rpc.connections.startConnection).not.toHaveBeenCalled();
  expect(rpc.connectionSetup.saveCredentials).toHaveBeenCalledWith(
    expect.objectContaining({ actionId: "act1", fields: [{ key: "api_key", value: "tvly-new" }] }),
  );
});
