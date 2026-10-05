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
  ConnectionSchema,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import {
  SandboxBlueprintSchema,
  SandboxReuse,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { SandboxesPage } from "./sandboxes";

const rpc = vi.hoisted(() => ({
  agents: { listAgents: vi.fn() },
  connections: { listConnections: vi.fn(), getProvider: vi.fn() },
  sandboxes: { listSandboxBlueprints: vi.fn(), createSandboxBlueprint: vi.fn() },
}));
vi.mock("@/client", () => rpc);

const devBox = create(SandboxBlueprintSchema, {
  id: "bp-1",
  name: "Dev box",
  connectionId: "e2b-1",
  template: "base",
  reuse: SandboxReuse.AGENT,
  agentIds: ["agent-1", "agent-2"],
});

function renderPage() {
  const root = createRootRoute({
    component: () => (
      <>
        <SandboxesPage />
        <Outlet />
      </>
    ),
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/sandboxes/$blueprintId/settings",
    component: () => <p>Blueprint page</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([detail]),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({
    agents: [
      { id: "agent-1", name: "Ops bot" },
      { id: "agent-2", name: "Fixer" },
    ],
    nextPageToken: "",
  });
  rpc.connections.listConnections.mockImplementation(async ({ providerId }) => ({
    connections:
      providerId === "e2b"
        ? [
            create(ConnectionSchema, {
              id: "e2b-1",
              name: "E2B prod",
              providerId,
              status: "ready",
            }),
            create(ConnectionSchema, {
              id: "e2b-2",
              name: "E2B expired",
              providerId,
              status: "requires_action",
            }),
          ]
        : [create(ConnectionSchema, { id: "modal-1", name: "Modal", providerId, status: "ready" })],
    nextPageToken: "",
  }));
  rpc.connections.getProvider.mockImplementation(async ({ id }) => ({
    provider: create(ProviderSchema, { id, name: id === "e2b" ? "E2B" : "Modal" }),
  }));
  rpc.sandboxes.listSandboxBlueprints.mockResolvedValue({ blueprints: [devBox] });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("lists blueprints and creates one from a ready sandbox connection", async () => {
  rpc.sandboxes.createSandboxBlueprint.mockResolvedValue({
    blueprint: create(SandboxBlueprintSchema, { id: "bp-2", name: "Shared" }),
  });
  renderPage();
  const table = await screen.findByRole("table", { name: "Sandbox blueprints" });
  const row = (await within(table).findByRole("link", { name: "Dev box" })).closest("tr")!;
  await within(row).findByText("E2B prod");
  within(row).getByText("Per agent");
  await within(row).findByRole("button", { name: "Agents: Ops bot, Fixer" });

  // New blueprint starts from an existing connection, searched among the ready sandbox ones.
  fireEvent.click(screen.getByRole("button", { name: "New blueprint" }));
  const menu = await screen.findByRole("menu");
  fireEvent.click(within(menu).getByRole("menuitem", { name: "Use existing connection" }));
  const existing = await screen.findByRole("dialog", { name: "Use an existing connection" });
  expect(within(existing).queryByText("E2B expired")).toBeNull();
  fireEvent.change(within(existing).getByLabelText("Search connections"), {
    target: { value: "prod" },
  });
  expect(within(existing).queryByText("Modal")).toBeNull();
  fireEvent.click(within(existing).getByRole("button", { name: /E2B prod/ }));
  const dialog = await screen.findByRole("dialog", { name: "New blueprint" });
  const connection = within(dialog).getByRole("combobox", { name: "Connection" });
  await waitFor(() => expect((connection as HTMLInputElement).value).toBe("E2B prod"));
  within(dialog).getByLabelText("E2B template ID");

  fireEvent.change(within(dialog).getByLabelText("Name"), { target: { value: " Shared " } });
  // The connection can be searched for; the template field follows its provider.
  fireEvent.focus(connection);
  fireEvent.keyDown(connection, { key: "ArrowDown" });
  fireEvent.input(connection, { target: { value: "mod" } });
  fireEvent.click(await screen.findByRole("option", { name: "Modal" }));
  fireEvent.change(await within(dialog).findByLabelText("Container image"), {
    target: { value: "ghcr.io/acme/sandbox:latest" },
  });
  // The default keeps sessions apart; shared modes warn about what crosses them.
  expect(within(dialog).queryByRole("note")).toBeNull();
  fireEvent.click(await within(dialog).findByRole("radio", { name: /^Shared by all agents/ }));
  expect(within(dialog).getByRole("note").textContent).toContain(
    "Data crosses agents and sessions",
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "Create blueprint" }));
  await waitFor(() =>
    expect(rpc.sandboxes.createSandboxBlueprint).toHaveBeenCalledWith({
      name: "Shared",
      connectionId: "modal-1",
      template: "ghcr.io/acme/sandbox:latest",
      reuse: SandboxReuse.GLOBAL,
      timings: {
        sleepAfterSeconds: 600,
        terminateAfterSeconds: 604_800,
        connectTimeoutSeconds: 120,
      },
    }),
  );
  await screen.findByText("Blueprint page");
});

it("sets up a new E2B or Modal connection when none is ready", async () => {
  rpc.connections.listConnections.mockResolvedValue({ connections: [], nextPageToken: "" });
  rpc.sandboxes.listSandboxBlueprints.mockResolvedValue({ blueprints: [] });
  renderPage();
  await screen.findByText("No sandbox blueprints yet.");
  fireEvent.click(screen.getByRole("button", { name: "New blueprint" }));
  const menu = await screen.findByRole("menu");
  expect(
    within(menu)
      .getAllByRole("menuitem")
      .map((item) => item.textContent),
  ).toEqual(["Use existing connection", "Add new connection"]);
  fireEvent.click(within(menu).getByRole("menuitem", { name: "Add new connection" }));
  // Only the sandbox providers are offered, not the whole catalog.
  const choose = await screen.findByRole("dialog", { name: "New sandbox connection" });
  expect(
    within(choose)
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((name) => name !== "Close"),
  ).toEqual(["E2B", "Modal"]);
  fireEvent.click(within(choose).getByRole("button", { name: "E2B" }));
  await screen.findByRole("dialog", { name: /E2B/ });
  expect(rpc.sandboxes.createSandboxBlueprint).not.toHaveBeenCalled();
});
