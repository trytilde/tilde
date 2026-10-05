import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { ConnectionSchema } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import { ToolDisplay, ToolDefinitionSchema } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import {
  ToolHostSchema,
  ToolMode,
  ToolSourceSchema,
} from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { SandboxBlueprintSchema } from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { DeclaredToolSchema } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import {
  AgentDeploymentSchema,
  DeploymentRouting,
  DeploymentSchema,
  DeploymentStatus,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { AgentTools } from "./agent-tools";

const rpc = vi.hoisted(() => ({
  connections: { listProviders: vi.fn(), listConnections: vi.fn() },
  toolHosts: { listToolHosts: vi.fn() },
  sandboxes: { getAgentSandbox: vi.fn(), getSandboxBlueprint: vi.fn() },
  deployments: { getDeployment: vi.fn(), getDeploymentContents: vi.fn() },
  tools: {
    listToolSources: vi.fn(),
    listProviderTools: vi.fn(),
    addToolSource: vi.fn(),
    getToolMode: vi.fn(),
    listBundledTools: vi.fn(),
    setToolMode: vi.fn(),
    removeToolSource: vi.fn(),
    setAgentTool: vi.fn(),
    removeAgentTool: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

// The agent uses Tavily's search in the background, but not its extract tool.
const tavily = create(ToolSourceSchema, {
  id: "src-tavily",
  agentId: "agent-1",
  connectionId: "c1",
  slug: "tavily",
  tools: [{ toolName: "search", name: "search", isAsync: true }],
});

function renderTab(owner: { agentId: string } | { blueprintId: string } = { agentId: "agent-1" }) {
  const root = createRootRoute({ component: () => <AgentTools {...owner} /> });
  const tools = createRoute({ getParentRoute: () => root, path: "/tools", component: () => null });
  const router = createRouter({
    routeTree: root.addChildren([tools]),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.connections.listProviders.mockResolvedValue({ providers: [], nextPageToken: "" });
  // These stand in for the server's filters.
  const matches = (name: string, search?: string) =>
    !search || name.toLowerCase().includes(search.toLowerCase());
  rpc.connections.listConnections.mockImplementation(async ({ search, status }) => ({
    connections: [
      create(ConnectionSchema, { id: "c1", name: "Tavily", providerId: "tavily", status: "ready" }),
      create(ConnectionSchema, { id: "c2", name: "Linear", providerId: "linear", status: "ready" }),
      create(ConnectionSchema, {
        id: "c3",
        name: "Pending",
        providerId: "linear",
        status: "requires_action",
      }),
    ].filter((c) => (!status || c.status === status) && matches(c.name, search)),
    nextPageToken: "",
  }));
  rpc.toolHosts.listToolHosts.mockImplementation(async ({ search, withProvider }) => ({
    toolHosts: [
      create(ToolHostSchema, {
        id: "h1",
        name: "Orders host",
        tools: [{ name: "lookup" }, { name: "refund" }],
      }),
      // Used through its provider's connections, never directly.
      create(ToolHostSchema, { id: "h2", name: "Billing host", providerId: "billing" }),
    ].filter(
      (h) =>
        (withProvider === undefined || !!h.providerId === withProvider) && matches(h.name, search),
    ),
  }));
  rpc.tools.listToolSources.mockResolvedValue({ sources: [tavily] });
  rpc.tools.listProviderTools.mockImplementation(async ({ connectionId }) => ({
    tools:
      connectionId === "c1"
        ? [
            { name: "search", description: "Search the web" },
            { name: "extract", description: "Read a page" },
          ]
        : [{ name: "issues" }, { name: "comment" }],
  }));
  rpc.tools.getToolMode.mockResolvedValue({ mode: ToolMode.DIRECT });
  rpc.tools.listBundledTools.mockResolvedValue({ tools: [] });
  rpc.deployments.getDeployment.mockResolvedValue({ deployment: undefined, deployments: [] });
  for (const call of ["addToolSource", "setToolMode", "setAgentTool", "removeAgentTool"] as const)
    rpc.tools[call].mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("adds a source with every tool off and edits which tools the agent uses", async () => {
  renderTab();
  await screen.findByRole("link", { name: "Tavily" });
  expect(screen.getByLabelText("Use search").getAttribute("aria-checked")).toBe("true");
  expect(screen.getByLabelText("Use extract").getAttribute("aria-checked")).toBe("false");

  // The server offers ready connections and provider-less hosts; the agent's own are left out.
  // The search is the server's too, and one is added only once chosen and continued.
  fireEvent.click(screen.getByRole("button", { name: "Add tool" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Choose existing" }));
  const dialog = await screen.findByRole("dialog");
  const list = within(dialog).getByRole("listbox", { name: "Available tools" });
  await within(list).findByText("Orders host");
  for (const hidden of ["Tavily", "Pending", "Billing host"])
    expect(within(list).queryByText(hidden)).toBeNull();
  expect(rpc.connections.listConnections).toHaveBeenCalledWith(
    expect.objectContaining({ search: undefined, status: "ready" }),
    expect.anything(),
  );
  fireEvent.change(within(dialog).getByLabelText("Search tools"), { target: { value: " lin " } });
  await waitFor(() => expect(within(list).queryByText("Orders host")).toBeNull());
  expect(rpc.connections.listConnections).toHaveBeenLastCalledWith(
    expect.objectContaining({ search: "lin", status: "ready" }),
    expect.anything(),
  );
  expect(rpc.toolHosts.listToolHosts).toHaveBeenLastCalledWith(
    { search: "lin", withProvider: false },
    expect.anything(),
  );
  const continueButton = within(dialog).getByRole("button", { name: "Continue" });
  expect(continueButton.hasAttribute("disabled")).toBe(true);
  fireEvent.click(within(list).getByText("Linear"));
  fireEvent.click(continueButton);
  await waitFor(() =>
    expect(rpc.tools.addToolSource).toHaveBeenCalledWith({
      agentId: "agent-1",
      connectionId: "c2",
      toolNames: [],
    }),
  );

  fireEvent.click(screen.getByLabelText("Use search"));
  await waitFor(() =>
    expect(rpc.tools.removeAgentTool).toHaveBeenCalledWith({
      sourceId: "src-tavily",
      toolName: "search",
    }),
  );
  fireEvent.click(screen.getByLabelText("Run search in the background"));
  await waitFor(() =>
    expect(rpc.tools.setAgentTool).toHaveBeenCalledWith({
      sourceId: "src-tavily",
      toolName: "search",
      isAsync: false,
      summary: "",
      description: "",
      display: ToolDisplay.UNSPECIFIED,
    }),
  );
  // One setting decides whether all of the agent's tools are listed or found by search.
  const modes = screen.getByRole("tablist", { name: "How the agent's tools are offered" });
  fireEvent.click(within(modes).getByRole("tab", { name: "Found by search" }));
  await waitFor(() =>
    expect(rpc.tools.setToolMode).toHaveBeenCalledWith({
      agentId: "agent-1",
      mode: ToolMode.DYNAMIC,
    }),
  );
});
it("edits the agent's own summary, description and display in place", async () => {
  rpc.tools.listProviderTools.mockResolvedValue({
    tools: [
      { name: "search", description: "Search the web", summary: "" },
      { name: "extract", description: "Read a page", summary: "Read a page" },
    ],
  });
  renderTab();
  await screen.findByRole("link", { name: "Tavily" });
  const saved = (change: object) =>
    waitFor(() =>
      expect(rpc.tools.setAgentTool).toHaveBeenLastCalledWith({
        sourceId: "src-tavily",
        toolName: "search",
        isAsync: true,
        summary: "",
        description: "",
        display: ToolDisplay.UNSPECIFIED,
        ...change,
      }),
    );

  // A tool without a summary says so, and the agent can give it one.
  const searchRow = screen.getByLabelText("Use search").closest("tr")!;
  within(searchRow).getByText("(no summary)");
  fireEvent.click(within(searchRow).getByRole("button", { name: "Edit summary of search" }));
  const summary = within(searchRow).getByLabelText("Summary of search");
  fireEvent.change(summary, { target: { value: "Looked something up" } });
  fireEvent.keyDown(summary, { key: "Enter" });
  await saved({ summary: "Looked something up" });

  fireEvent.click(within(searchRow).getByRole("button", { name: "Edit description of search" }));
  fireEvent.change(within(searchRow).getByLabelText("Description of search"), {
    target: { value: "Search the web for current facts." },
  });
  fireEvent.click(within(searchRow).getByRole("button", { name: "Save description of search" }));
  await saved({ description: "Search the web for current facts." });

  const display = within(searchRow).getByRole("tablist", { name: "How search is displayed" });
  fireEvent.click(within(display).getByRole("tab", { name: "Summary" }));
  await saved({ display: ToolDisplay.SUMMARY });

  // Tools the agent does not use show their own text and cannot be edited.
  const extractRow = screen.getByLabelText("Use extract").closest("tr")!;
  expect(within(extractRow).getAllByText("Read a page")).toHaveLength(2);
  expect(within(extractRow).queryByRole("button", { name: /^Edit/ })).toBeNull();
});
it("removes a source only after confirming", async () => {
  rpc.tools.removeToolSource.mockResolvedValue({});
  renderTab();
  await screen.findByRole("link", { name: "Tavily" });
  fireEvent.click(screen.getByRole("button", { name: "Remove Tavily" }));
  const dialog = await screen.findByRole("alertdialog");
  expect(rpc.tools.removeToolSource).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
  await waitFor(() =>
    expect(rpc.tools.removeToolSource).toHaveBeenCalledWith({ id: "src-tavily" }),
  );
});
const deployment = (id: string, label: string, trafficWeight: number) =>
  create(AgentDeploymentSchema, {
    id,
    agentId: "agent-1",
    label,
    commitSha: `${id}-sha-0000000`,
    status: DeploymentStatus.REGISTERED,
    trafficWeight,
    serving: trafficWeight > 0,
  });
it("lists the tools each serving deployment declared", async () => {
  rpc.tools.listToolSources.mockResolvedValue({ sources: [] });
  rpc.deployments.getDeployment.mockResolvedValue({
    deployment: create(DeploymentSchema, {
      agentId: "agent-1",
      routing: DeploymentRouting.WEIGHTED,
    }),
    deployments: [
      deployment("canary", "Canary", 10),
      deployment("stable", "Stable", 90),
      deployment("idle", "Idle", 0),
    ],
  });
  rpc.deployments.getDeploymentContents.mockImplementation(async ({ deploymentId }) => ({
    prompts: [],
    skills: [],
    tools: [
      create(DeclaredToolSchema, {
        name: deploymentId === "canary" ? "roll_dice_v2" : "roll_dice",
        description: "Roll dice for the user.",
        summary: "Rolled dice",
        display: ToolDisplay.HIDDEN,
      }),
    ],
  }));
  renderTab();
  const table = await screen.findByRole("table", { name: "Agent tools" });
  const canary = (await within(table).findByText("From code · Canary")).closest("tr")!;
  within(canary).getByText("Declared by Canary · canary- · 10% of traffic");
  const stable = within(table).getByText("From code · Stable").closest("tr")!;
  within(stable).getByText("Declared by Stable · stable- · 90% of traffic");
  within(table).getByText("roll_dice_v2");
  const row = within(table).getByText("roll_dice").closest("tr")!;
  within(row).getByText("Hidden");
  expect(within(table).queryByText(/Idle/)).toBeNull();
  // Declared tools stand for the deployments: the latest run's registration is not read.
  expect(rpc.tools.listBundledTools).not.toHaveBeenCalled();
  fireEvent.click(within(canary).getByRole("button", { name: "Collapse From code · Canary" }));
  expect(within(table).queryByText("roll_dice_v2")).toBeNull();
  within(table).getByText("roll_dice");
});
it("falls back to the tools the latest run registered when the serving deployment declared none", async () => {
  rpc.tools.listToolSources.mockResolvedValue({ sources: [] });
  rpc.deployments.getDeployment.mockResolvedValue({
    deployment: create(DeploymentSchema, { agentId: "agent-1" }),
    deployments: [deployment("dep-1", "Current", 100)],
  });
  rpc.deployments.getDeploymentContents.mockResolvedValue({ prompts: [], skills: [], tools: [] });
  rpc.tools.listBundledTools.mockResolvedValue({
    tools: [
      create(ToolDefinitionSchema, {
        name: "roll_dice",
        providerId: "bundled",
        description: "Roll dice for the user.",
        summary: "Rolled dice",
        display: ToolDisplay.SUMMARY,
      }),
    ],
    invocationId: "inv-1",
    registeredAt: timestampFromDate(new Date("2026-09-29T10:00:00Z")),
  });
  renderTab();
  const table = await screen.findByRole("table", { name: "Agent tools" });
  const group = within(table).getByText("From code").closest("tr")!;
  expect(within(group).getByText("Bundled").getAttribute("title")).toContain(
    "Configure it in code",
  );
  within(group).getByText(/^Registered by the run at /);
  expect(rpc.deployments.getDeploymentContents).toHaveBeenCalledWith(
    { agentId: "agent-1", deploymentId: "dep-1" },
    expect.anything(),
  );
  const row = within(table).getByText("roll_dice").closest("tr")!;
  within(row).getByText("Rolled dice");
  within(row).getByText("Roll dice for the user.");
  within(row).getByText("Summary");
  // Configured in code: no switches, tabs or edit buttons.
  expect(within(row).queryByRole("switch")).toBeNull();
  expect(within(row).queryByRole("tablist")).toBeNull();
  expect(within(row).queryByRole("button")).toBeNull();
});
it("says how to declare bundled tools when there are none", async () => {
  rpc.tools.listToolSources.mockResolvedValue({ sources: [] });
  renderTab();
  const table = await screen.findByRole("table", { name: "Agent tools" });
  within(table).getByText(
    /^None declared or registered yet\. Export the agent's tools with defineTools/,
  );
});
it("collapses a group's tool rows", async () => {
  renderTab();
  await screen.findByRole("link", { name: "Tavily" });
  expect(screen.getByLabelText("Use search")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Collapse Tavily" }));
  expect(screen.queryByLabelText("Use search")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Expand Tavily" }));
  expect(screen.getByLabelText("Use search")).toBeTruthy();
});
it("keeps the agent's sandbox tools while its sandbox setting does", async () => {
  rpc.tools.listToolSources.mockResolvedValue({
    sources: [
      tavily,
      create(ToolSourceSchema, {
        id: "src-sandbox",
        agentId: "agent-1",
        sandbox: true,
        slug: "sandbox",
        tools: [{ toolName: "exec" }],
      }),
    ],
  });
  rpc.tools.listProviderTools.mockImplementation(async ({ sandbox }) => ({
    tools: sandbox ? [{ name: "exec" }, { name: "read_file" }] : [],
  }));
  rpc.sandboxes.getAgentSandbox.mockResolvedValue({
    sandbox: { agentId: "agent-1", blueprintId: "bp-1", toolSourceId: "src-sandbox" },
  });
  rpc.sandboxes.getSandboxBlueprint.mockResolvedValue({
    blueprint: create(SandboxBlueprintSchema, { id: "bp-1", name: "Dev box" }),
  });
  renderTab();
  const link = await screen.findByRole("link", { name: "Sandbox · Dev box" });
  expect(link.getAttribute("href")).toBe("/sandboxes/bp-1/settings");
  expect(screen.getByLabelText("Use read_file").getAttribute("aria-checked")).toBe("false");
  fireEvent.click(screen.getByLabelText("Use read_file"));
  await waitFor(() =>
    expect(rpc.tools.setAgentTool).toHaveBeenCalledWith({
      sourceId: "src-sandbox",
      toolName: "read_file",
    }),
  );
  // It goes with the sandbox setting in Capabilities; other sources can still be removed.
  expect(
    screen.getByRole("button", { name: "Remove Sandbox · Dev box" }).hasAttribute("disabled"),
  ).toBe(true);
  expect(screen.getByRole("button", { name: "Remove Tavily" }).hasAttribute("disabled")).toBe(
    false,
  );
});
it("manages a sandbox blueprint's sources without an agent's settings", async () => {
  renderTab({ blueprintId: "bp-1" });
  await screen.findByRole("link", { name: "Tavily" });
  expect(rpc.tools.listToolSources).toHaveBeenCalledWith(
    { sandboxBlueprintId: "bp-1" },
    expect.anything(),
  );
  // Programs call these tools, so there is no tool mode, bundled group or per-tool setting.
  expect(rpc.tools.getToolMode).not.toHaveBeenCalled();
  expect(rpc.deployments.getDeployment).not.toHaveBeenCalled();
  expect(screen.queryByRole("tablist")).toBeNull();
  expect(screen.queryByLabelText("Run search in the background")).toBeNull();
  expect(screen.queryByRole("button", { name: /^Edit/ })).toBeNull();
  screen.getByText(/tilde sandbox call/);

  fireEvent.click(screen.getByLabelText("Use extract"));
  await waitFor(() =>
    expect(rpc.tools.setAgentTool).toHaveBeenCalledWith({
      sourceId: "src-tavily",
      toolName: "extract",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add tool" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Choose existing" }));
  const dialog = await screen.findByRole("dialog");
  fireEvent.click(await within(dialog).findByText("Linear"));
  fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
  await waitFor(() =>
    expect(rpc.tools.addToolSource).toHaveBeenCalledWith({
      sandboxBlueprintId: "bp-1",
      connectionId: "c2",
      toolNames: [],
    }),
  );
});
