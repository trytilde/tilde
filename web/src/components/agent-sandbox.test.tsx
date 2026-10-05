import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import {
  SandboxBlueprintSchema,
  SandboxReuse,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { AgentSandbox } from "./agent-sandbox";

const rpc = vi.hoisted(() => ({
  connections: { listConnections: vi.fn(), getProvider: vi.fn() },
  sandboxes: {
    getAgentSandbox: vi.fn(),
    listSandboxBlueprints: vi.fn(),
    setAgentSandbox: vi.fn(),
    removeAgentSandbox: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

function renderSection() {
  const root = createRootRoute({ component: () => <AgentSandbox agentId="agent-1" /> });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.sandboxes.listSandboxBlueprints.mockResolvedValue({
    blueprints: [
      create(SandboxBlueprintSchema, { id: "bp-1", name: "Dev box", reuse: SandboxReuse.THREAD }),
      create(SandboxBlueprintSchema, { id: "bp-2", name: "Shared", reuse: SandboxReuse.GLOBAL }),
    ],
  });
  rpc.connections.listConnections.mockResolvedValue({ connections: [], nextPageToken: "" });
  rpc.connections.getProvider.mockRejectedValue(new Error("unused"));
  rpc.sandboxes.setAgentSandbox.mockResolvedValue({});
  rpc.sandboxes.removeAgentSandbox.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("gives an agent without a sandbox one at once", async () => {
  rpc.sandboxes.getAgentSandbox.mockResolvedValue({});
  renderSection();
  const select = await screen.findByRole("combobox", { name: "Sandbox" });
  await waitFor(() => expect(select.hasAttribute("disabled")).toBe(false));
  fireEvent.focus(select);
  fireEvent.keyDown(select, { key: "ArrowDown" });
  fireEvent.input(select, { target: { value: "sha" } });
  expect(screen.queryByRole("option", { name: "Dev box" })).toBeNull();
  fireEvent.click(await screen.findByRole("option", { name: "Shared" }));
  await waitFor(() =>
    expect(rpc.sandboxes.setAgentSandbox).toHaveBeenCalledWith({
      agentId: "agent-1",
      blueprintId: "bp-2",
    }),
  );
  screen.getByText(/Shared by all agents: Every agent using this blueprint shares one sandbox\./);
  expect(screen.getByRole("link", { name: "Open Shared" }).getAttribute("href")).toBe(
    "/sandboxes/bp-2/settings",
  );
});

it("confirms before removing a sandbox, which terminates the agent's sandboxes", async () => {
  rpc.sandboxes.getAgentSandbox.mockResolvedValue({
    sandbox: { agentId: "agent-1", blueprintId: "bp-1", toolSourceId: "src-1" },
  });
  renderSection();
  await screen.findByRole("link", { name: "Open Dev box" });
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  const dialog = await screen.findByRole("alertdialog");
  within(dialog).getByText(/existing sandboxes are terminated/);
  expect(rpc.sandboxes.removeAgentSandbox).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "Remove sandbox" }));
  await waitFor(() =>
    expect(rpc.sandboxes.removeAgentSandbox).toHaveBeenCalledWith({ agentId: "agent-1" }),
  );
  expect(rpc.sandboxes.setAgentSandbox).not.toHaveBeenCalled();
});
