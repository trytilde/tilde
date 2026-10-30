import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { AgentSchema } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  MemoryAccess,
  MemoryBankSchema,
  MemoryBankScope,
  MemoryKind,
  MemoryMatch,
  MemorySchema,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import { AgentEditor } from "./agent-editor";
import { MemoryBankPage } from "./memory-bank";

vi.mock("@/components/agent-iam", () => ({ AgentIam: () => null }));
vi.mock("@/components/agent-sandbox", () => ({ AgentSandbox: () => null }));
vi.mock("./agent-connections", () => ({ AgentConnections: () => null }));
const rpc = vi.hoisted(() => ({
  agents: { createAgent: vi.fn(), updateAgent: vi.fn(), listAgents: vi.fn() },
  deployments: { getDeployment: vi.fn(), setDeployment: vi.fn() },
  memory: {
    listMemoryBanks: vi.fn(),
    listAgentMemoryBanks: vi.fn(),
    createMemoryBank: vi.fn(),
    setAgentMemoryBank: vi.fn(),
    getMemoryBank: vi.fn(),
    listMemories: vi.fn(),
    listMemoryPaths: vi.fn(),
    listMemoryPeople: vi.fn(),
    getMemory: vi.fn(),
    createMemory: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

function renderAt(component: () => React.ReactNode) {
  const router = createRouter({
    routeTree: createRootRoute({ component }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
const team = create(MemoryBankSchema, {
  id: "bank-team",
  name: "Team",
  scope: MemoryBankScope.SHARED,
});
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({ agents: [], nextPageToken: "" });
  rpc.memory.listMemoryBanks.mockResolvedValue({ banks: [team] });
  rpc.memory.getMemoryBank.mockResolvedValue({ bank: team });
  rpc.memory.listMemoryPaths.mockResolvedValue({ paths: [] });
  rpc.memory.listMemoryPeople.mockResolvedValue({ people: [] });
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("gives a new agent its own bank and links shared banks once it exists", async () => {
  rpc.agents.createAgent.mockResolvedValue({
    agent: create(AgentSchema, { id: "a1", name: "Ada" }),
  });
  const onSaved = vi.fn();
  renderAt(() => <AgentEditor agent={null} onClose={vi.fn()} onSaved={onSaved} />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada" },
  });
  const section = within(screen.getByRole("region", { name: "Memory" }));
  fireEvent.change(section.getByLabelText("Its own bank"), {
    target: { value: String(MemoryBankScope.PER_PERSON) },
  });
  fireEvent.change(await section.findByRole("combobox", { name: "Shared bank to link" }), {
    target: { value: "bank-team" },
  });
  fireEvent.change(section.getByRole("combobox", { name: "Access to the bank to link" }), {
    target: { value: String(MemoryAccess.READ_WRITE) },
  });
  fireEvent.click(section.getByRole("button", { name: "Link bank" }));
  await section.findByRole("link", { name: "Team" });
  // Nothing is saved before the agent exists.
  expect(rpc.memory.createMemoryBank).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Create agent" }));
  await waitFor(() => expect(onSaved).toHaveBeenCalledWith(true));
  expect(rpc.memory.createMemoryBank).toHaveBeenCalledWith(
    expect.objectContaining({ name: "Ada", agentId: "a1", scope: MemoryBankScope.PER_PERSON }),
  );
  expect(rpc.memory.setAgentMemoryBank).toHaveBeenCalledWith({
    agentId: "a1",
    bankId: "bank-team",
    access: MemoryAccess.READ_WRITE,
  });
});

it("adds a person's fact at the chosen path and shows how a search found each memory", async () => {
  const fact = create(MemorySchema, {
    id: "m1",
    bankId: "bank-team",
    kind: MemoryKind.PATTERNS,
    path: "deploys",
    title: "Deploys on Fridays need a second reviewer",
    content: "Any Friday deploy needs a second reviewer.",
  });
  rpc.memory.listMemoryPaths.mockResolvedValue({ paths: [{ path: "deploys", count: 2 }] });
  rpc.memory.listMemories.mockImplementation(async ({ query }) => ({
    memories: query ? [{ ...fact, matched: [MemoryMatch.MEANING] }] : [],
    nextPageToken: "",
  }));
  rpc.memory.createMemory.mockResolvedValue({ memory: fact });
  rpc.memory.getMemory.mockResolvedValue({ memory: fact, revisions: [] });
  renderAt(() => <MemoryBankPage bankId="bank-team" tab="memories" onTabChange={vi.fn()} />);

  fireEvent.click(await screen.findByRole("button", { name: /deploys/ }));
  fireEvent.click(screen.getByRole("button", { name: "Add fact" }));
  const dialog = within(await screen.findByRole("form", { name: "Memory" }));
  expect((dialog.getByRole("textbox", { name: "Path" }) as HTMLInputElement).value).toBe("deploys");
  fireEvent.change(dialog.getByRole("textbox", { name: "Title" }), {
    target: { value: fact.title },
  });
  fireEvent.change(dialog.getByRole("textbox", { name: "Content" }), {
    target: { value: fact.content },
  });
  fireEvent.click(dialog.getByRole("button", { name: "Add fact" }));
  await waitFor(() =>
    expect(rpc.memory.createMemory).toHaveBeenCalledWith(
      expect.objectContaining({ bankId: "bank-team", path: "deploys", title: fact.title }),
    ),
  );
  await screen.findByRole("article", { name: fact.title });

  fireEvent.change(screen.getByRole("searchbox", { name: "Search memories" }), {
    target: { value: "who checks weekend releases" },
  });
  const list = within(await screen.findByRole("list", { name: "Memory list" }));
  await list.findByText("Meaning");
});
