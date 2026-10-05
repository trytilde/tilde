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
  within(row).getByText("2");

  fireEvent.click(screen.getByRole("button", { name: "New blueprint" }));
  const dialog = await screen.findByRole("dialog");
  const connection = within(dialog).getByLabelText("Connection");
  // A connection that needs attention cannot launch sandboxes.
  expect(within(connection).queryByText(/E2B expired/)).toBeNull();
  fireEvent.change(within(dialog).getByLabelText("Name"), { target: { value: " Shared " } });
  fireEvent.change(connection, { target: { value: "modal-1" } });
  fireEvent.change(within(dialog).getByLabelText("Template"), {
    target: { value: "ghcr.io/acme/sandbox:latest" },
  });
  // The default keeps sessions apart; shared modes warn about what crosses them.
  expect(within(dialog).queryByRole("note")).toBeNull();
  fireEvent.change(within(dialog).getByLabelText("Reuse"), {
    target: { value: String(SandboxReuse.GLOBAL) },
  });
  within(dialog).getByText("Every agent using this blueprint shares one sandbox.");
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

it("points to the catalog when no sandbox connection is ready", async () => {
  rpc.connections.listConnections.mockResolvedValue({ connections: [], nextPageToken: "" });
  rpc.sandboxes.listSandboxBlueprints.mockResolvedValue({ blueprints: [] });
  renderPage();
  await screen.findByText("No sandbox blueprints yet.");
  fireEvent.click(screen.getByRole("button", { name: "New blueprint" }));
  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByRole("link", { name: "E2B" }).getAttribute("href")).toBe(
    "/tools/catalog/e2b",
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "Create blueprint" }));
  await waitFor(() =>
    expect(within(dialog).getByLabelText("Connection").getAttribute("aria-invalid")).toBe("true"),
  );
  expect(rpc.sandboxes.createSandboxBlueprint).not.toHaveBeenCalled();
});
