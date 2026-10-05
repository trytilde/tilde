import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { ConnectionSchema } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { AgentSchema } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  SandboxBlueprintSchema,
  SandboxReuse,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { SandboxBlueprintPage, type BlueprintTab } from "./sandbox-blueprint";

const rpc = vi.hoisted(() => ({
  agents: { listAgents: vi.fn() },
  connections: { listConnections: vi.fn(), getProvider: vi.fn() },
  sandboxes: {
    getSandboxBlueprint: vi.fn(),
    updateSandboxBlueprint: vi.fn(),
    deleteSandboxBlueprint: vi.fn(),
    setSandboxEnvVar: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

const devBox = create(SandboxBlueprintSchema, {
  id: "bp-1",
  name: "Dev box",
  connectionId: "e2b-1",
  template: "base",
  reuse: SandboxReuse.THREAD,
  envNames: ["GITHUB_TOKEN"],
  agentIds: ["agent-1"],
});
function renderPage(tab: BlueprintTab) {
  const root = createRootRoute({
    component: () => <SandboxBlueprintPage blueprintId="bp-1" tab={tab} onTabChange={vi.fn()} />,
  });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({
    agents: [create(AgentSchema, { id: "agent-1", name: "Ada" })],
    nextPageToken: "",
  });
  rpc.connections.listConnections.mockImplementation(async ({ providerId }) => ({
    connections:
      providerId === "e2b"
        ? [create(ConnectionSchema, { id: "e2b-1", name: "E2B prod", providerId, status: "ready" })]
        : [],
    nextPageToken: "",
  }));
  rpc.connections.getProvider.mockResolvedValue({});
  rpc.sandboxes.getSandboxBlueprint.mockResolvedValue({ blueprint: devBox });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("saves only what changed and warns that a new reuse mode terminates sandboxes", async () => {
  rpc.sandboxes.updateSandboxBlueprint.mockResolvedValue({
    blueprint: { ...devBox, reuse: SandboxReuse.AGENT },
  });
  renderPage("settings");
  const form = await screen.findByRole("form", { name: "Blueprint settings" });
  expect(within(form).queryByText(/terminates this blueprint's existing sandboxes/)).toBeNull();
  fireEvent.change(within(form).getByLabelText("Reuse"), {
    target: { value: String(SandboxReuse.AGENT) },
  });
  within(form).getByText(/terminates this blueprint's existing sandboxes/);
  within(form).getByText(/visible to every other session of the agent/);
  fireEvent.click(within(form).getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(rpc.sandboxes.updateSandboxBlueprint).toHaveBeenCalledWith({
      id: "bp-1",
      name: undefined,
      connectionId: undefined,
      template: undefined,
      reuse: SandboxReuse.AGENT,
    }),
  );
  // Deleting waits until no agent uses it.
  expect(screen.getByRole("button", { name: "Delete blueprint" }).hasAttribute("disabled")).toBe(
    true,
  );
  expect((await screen.findByRole("link", { name: "Ada" })).getAttribute("href")).toBe(
    "/agent/agent-1/capabilities",
  );
});

it("replaces an environment variable's value without ever showing it", async () => {
  rpc.sandboxes.setSandboxEnvVar.mockResolvedValue({ blueprint: devBox });
  renderPage("environment");
  const table = await screen.findByRole("table", { name: "Environment variables" });
  fireEvent.click(within(table).getByRole("button", { name: "Replace GITHUB_TOKEN" }));
  const dialog = await screen.findByRole("dialog");
  const value = within(dialog).getByLabelText("Value");
  expect(value.getAttribute("type")).toBe("password");
  expect((value as HTMLInputElement).value).toBe("");
  fireEvent.change(value, { target: { value: "ghp_secret" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(rpc.sandboxes.setSandboxEnvVar).toHaveBeenCalledWith({
      blueprintId: "bp-1",
      name: "GITHUB_TOKEN",
      value: "ghp_secret",
    }),
  );
  expect(screen.queryByText("ghp_secret")).toBeNull();
});
