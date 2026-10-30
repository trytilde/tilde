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
  MemorySynthesisBatchStatus,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import { BankSynthesis } from "./memory-synthesis";

const rpc = vi.hoisted(() => ({
  connections: { listConnections: vi.fn() },
  wikis: { listWikis: vi.fn() },
  memory: {
    getMemorySynthesis: vi.fn(),
    setMemorySynthesis: vi.fn(),
    listMemorySynthesisBatches: vi.fn(),
    listChatModels: vi.fn(),
    setMemoryBankRetain: vi.fn(),
    setMemoryBankWiki: vi.fn(),
    runMemorySynthesis: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

const bank = create(MemoryBankSchema, {
  id: "bank-team",
  name: "Team",
  scope: MemoryBankScope.SHARED,
  links: [
    { agentId: "writer", access: MemoryAccess.READ_WRITE },
    { agentId: "reader", access: MemoryAccess.READ },
  ],
});
const registry = new Map(
  ["writer", "reader"].map((id) => [id, create(AgentSchema, { id, name: `Agent ${id}` })]),
);
beforeEach(() => {
  rpc.connections.listConnections.mockResolvedValue({
    connections: [{ id: "openai", name: "OpenAI" }],
    nextPageToken: "",
  });
  rpc.wikis.listWikis.mockResolvedValue({
    wikis: [
      { id: "handbook", name: "Handbook" },
      { id: "runbooks", name: "Runbooks", pageCount: 40 },
    ],
  });
  rpc.memory.listChatModels.mockResolvedValue({ models: ["gpt-test"] });
  rpc.memory.listMemorySynthesisBatches.mockResolvedValue({
    batches: [
      {
        id: "b1",
        pageId: "p1",
        revision: 2,
        wikiId: "handbook",
        path: "office/deploys",
        status: MemorySynthesisBatchStatus.APPLIED,
        written: 1,
        superseded: 0,
        dropped: 1,
        notes: "Operation 2: cites evidence not in this batch",
        memories: [
          { id: "m1", title: "Deploys on Tuesdays" },
          { id: "m0", title: "Deploys on Mondays", forgotten: true },
        ],
      },
    ],
  });
  rpc.memory.getMemorySynthesis.mockResolvedValue({
    synthesis: { connectionId: "openai", model: "gpt-test", instructions: "" },
    retainingAgentIds: [],
    wikis: [{ wikiId: "handbook", name: "Handbook" }],
    failures: 2,
    lastError: "OpenAI answered 500",
    pendingConversations: 3,
    pendingPages: 1,
  });
  for (const write of [
    rpc.memory.setMemorySynthesis,
    rpc.memory.setMemoryBankRetain,
    rpc.memory.setMemoryBankWiki,
  ])
    write.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("learns only from agents that may write, from linked wikis, and shows why batches failed", async () => {
  const router = createRouter({
    routeTree: createRootRoute({
      component: () => <BankSynthesis bank={bank} registry={registry} />,
    }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  render(<RouterProvider router={router} />);
  await screen.findByText(
    "Failing (2 in a row): OpenAI answered 500 3 conversations, 1 pages waiting.",
  );
  // A batch links to its source and to the memories it wrote.
  const batches = screen.getByLabelText("Recent batches");
  within(batches).getByRole("link", { name: "office/deploys, revision 2" });
  expect(
    within(batches).getByRole("link", { name: "Deploys on Tuesdays" }).getAttribute("href"),
  ).toBe("/memory/bank-team/memories?memory=m1");
  // What it forgot is listed, not linked.
  within(batches).getByText("Deploys on Mondays");
  expect(within(batches).queryByRole("link", { name: "Deploys on Mondays" })).toBeNull();
  // A read-only agent cannot feed the bank.
  const conversations = screen.getByLabelText("Conversations it learns from");
  expect(within(conversations).queryByText("Agent reader")).toBeNull();
  fireEvent.click(
    within(conversations).getByRole("switch", {
      name: "Learn from Agent writer's conversations",
    }),
  );
  await waitFor(() =>
    expect(rpc.memory.setMemoryBankRetain).toHaveBeenCalledWith({
      bankId: "bank-team",
      agentId: "writer",
      autoRetain: true,
    }),
  );
  // Only wikis not already feeding it are offered.
  const wikis = screen.getByLabelText("Wikis it learns from");
  const add = within(wikis).getByRole("combobox", { name: "Learn from a wiki" });
  expect(within(add).queryByRole("option", { name: "Handbook" })).toBeNull();
  // Linking says what it will cost first.
  fireEvent.change(add, { target: { value: "runbooks" } });
  const confirm = await screen.findByRole("alertdialog");
  within(confirm).getByText(/Its 40 pages are each read now, one model call per page/);
  expect(rpc.memory.setMemoryBankWiki).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "Learn from it" }));
  await waitFor(() =>
    expect(rpc.memory.setMemoryBankWiki).toHaveBeenCalledWith({
      bankId: "bank-team",
      wikiId: "runbooks",
    }),
  );
  screen.getByText("Operation 2: cites evidence not in this batch");

  fireEvent.change(screen.getByLabelText("Instructions"), {
    target: { value: "Keep customers' preferences." },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(rpc.memory.setMemorySynthesis).toHaveBeenCalledWith({
      bankId: "bank-team",
      synthesis: {
        connectionId: "openai",
        model: "gpt-test",
        instructions: "Keep customers' preferences.",
      },
    }),
  );
});

it("shows what waits while off, and that a linked wiki is read once synthesis is on", async () => {
  rpc.memory.getMemorySynthesis.mockResolvedValue({
    retainingAgentIds: [],
    wikis: [],
    failures: 0,
    lastError: "",
    pendingConversations: 2,
    pendingPages: 0,
  });
  const router = createRouter({
    routeTree: createRootRoute({
      component: () => <BankSynthesis bank={bank} registry={registry} />,
    }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  render(<RouterProvider router={router} />);
  await screen.findByText("Off: nothing is synthesized. 2 conversations, 0 pages waiting.");
  fireEvent.change(screen.getByRole("combobox", { name: "Learn from a wiki" }), {
    target: { value: "runbooks" },
  });
  const confirm = await screen.findByRole("alertdialog");
  within(confirm).getByText(/Its 40 pages are each read once synthesis is on/);
});
