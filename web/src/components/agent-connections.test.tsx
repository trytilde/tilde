import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  Capability,
  ConnectionSchema,
  ProviderSchema,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { AgentConnections } from "./agent-connections";

const rpc = vi.hoisted(() => ({
  listProviders: vi.fn(),
  listConnections: vi.fn(),
  startConnection: vi.fn(),
  assignCapability: vi.fn(),
  unassignCapability: vi.fn(),
  reconnect: vi.fn(),
}));
const inferenceRpc = vi.hoisted(() => ({
  getUsage: vi.fn(),
  listBudgets: vi.fn(),
  setBudget: vi.fn(),
  deleteBudget: vi.fn(),
}));
const tildeRpc = vi.hoisted(() => ({ getCredentials: vi.fn(), rotateCredentials: vi.fn() }));
vi.mock("@/client", () => ({ connections: rpc, inference: inferenceRpc, tildeChat: tildeRpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const whatsapp = create(ProviderSchema, {
  id: "whatsapp",
  iconUrl: "https://cdn.jsdelivr.net/gh/glincker/thesvg@main/public/icons/whatsapp/default.svg",
  name: "WhatsApp",
  categories: ["chat"],
  connectionTypes: [
    { id: "meta", name: "Meta WhatsApp Business", capabilities: [Capability.CHANNEL] },
  ],
});
const custom = create(ProviderSchema, {
  id: "custom",
  name: "Custom chat",
  connectionTypes: [
    { id: "oauth", name: "OAuth", capabilities: [Capability.CHANNEL] },
    { id: "key", name: "API key", capabilities: [Capability.CHANNEL] },
    { id: "tools", name: "Tools only", capabilities: [] },
  ],
});
const connection = create(ConnectionSchema, {
  id: "connection-1",
  name: "Support WhatsApp",
  providerId: "whatsapp",
  typeId: "meta",
  status: "ready",
  capabilities: [Capability.CHANNEL],
});
const brokeringUrl =
  "https://api.example.com/connections/broker/setup-1?connection_setup_token=setup-token";
function defaults() {
  rpc.listProviders.mockResolvedValue({ providers: [whatsapp], nextPageToken: "" });
  rpc.listConnections.mockResolvedValue({ connections: [], nextPageToken: "" });
}

it("loads every catalog page and presents chat methods in pill menus", async () => {
  defaults();
  rpc.listProviders.mockImplementation(async ({ pageToken }) =>
    pageToken
      ? { providers: [custom], nextPageToken: "" }
      : { providers: [whatsapp], nextPageToken: "next" },
  );
  render(<AgentConnections agentId="agent-1" />);
  const pill = await screen.findByRole("button", { name: "Custom chat" });
  expect(pill.className).toContain("rounded-full");
  // The server keeps only providers with a chat method.
  expect(rpc.listProviders).toHaveBeenCalledWith(
    { capability: Capability.CHANNEL, pageSize: 100, pageToken: "next" },
    expect.anything(),
  );
  fireEvent.click(pill);
  await screen.findByRole("menuitem", { name: "Use existing connection" });
  expect(screen.getByText("Or add new")).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "OAuth" })).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "API key" })).toBeTruthy();
  expect(screen.queryByText("Tools only")).toBeNull();
  rpc.startConnection.mockResolvedValue({ connection, brokeringUrl });
  fireEvent.click(screen.getByRole("menuitem", { name: "API key" }));
  await screen.findByTitle("Connect Custom chat");
  expect(rpc.startConnection).toHaveBeenCalledWith(
    expect.objectContaining({
      providerId: "custom",
      typeId: "key",
      assignments: [{ capability: Capability.CHANNEL, agentId: "agent-1" }],
    }),
  );
});

it("starts the single method in an iframe and accepts completion only from that broker", async () => {
  defaults();
  rpc.startConnection.mockResolvedValue({ connection, brokeringUrl });
  render(<AgentConnections agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "WhatsApp" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Add new connection" }));
  expect(screen.queryByText("Or add new")).toBeNull();
  const frame = (await screen.findByTitle("Connect WhatsApp")) as HTMLIFrameElement;
  expect(frame.parentElement?.getAttribute("aria-hidden")).toBe("true");
  fireEvent.load(frame);
  expect(frame.parentElement?.getAttribute("aria-hidden")).toBe("false");
  expect(frame.src).toBe(brokeringUrl);
  expect(frame.getAttribute("referrerpolicy")).toBe("no-referrer");
  expect(rpc.startConnection).toHaveBeenCalledWith(
    expect.objectContaining({
      name: "WhatsApp",
      providerId: "whatsapp",
      typeId: "meta",
      assignments: [{ capability: Capability.CHANNEL, agentId: "agent-1" }],
    }),
  );
  const dispatch = (source: Window | null, origin: string, connectionId: string) =>
    act(() =>
      window.dispatchEvent(
        new MessageEvent("message", {
          source,
          origin,
          data: { type: "tilde.connection.complete", connectionId },
        }),
      ),
    );
  await dispatch(window, "https://api.example.com", connection.id);
  await dispatch(frame.contentWindow, "https://wrong.example.com", connection.id);
  await dispatch(frame.contentWindow, "https://api.example.com", "another-connection");
  expect(screen.getByTitle("Connect WhatsApp")).toBeTruthy();
  await dispatch(frame.contentWindow, "https://api.example.com", connection.id);
  await waitFor(() => expect(screen.queryByTitle("Connect WhatsApp")).toBeNull());
  expect(rpc.listConnections.mock.calls.length).toBeGreaterThan(1);
});

it("selects an available connection from later pages for only the chosen provider", async () => {
  defaults();
  // The server filters by provider; the first page of matches can still be empty.
  rpc.listConnections.mockImplementation(async ({ agentId, pageToken }) =>
    agentId
      ? { connections: [], nextPageToken: "" }
      : pageToken
        ? { connections: [connection], nextPageToken: "" }
        : { connections: [], nextPageToken: "more" },
  );
  rpc.assignCapability.mockResolvedValue({ connection });
  render(<AgentConnections agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "WhatsApp" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Use existing connection" }));
  const select = await screen.findByRole("button", { name: /Support WhatsApp/ });
  expect(rpc.listConnections).toHaveBeenCalledWith(
    { capability: Capability.CHANNEL, providerId: "whatsapp", pageSize: 100, pageToken: "more" },
    expect.anything(),
  );
  fireEvent.click(select);
  await waitFor(() =>
    expect(rpc.assignCapability).toHaveBeenCalledWith({
      connectionId: connection.id,
      assignment: { capability: Capability.CHANNEL, agentId: "agent-1" },
    }),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(rpc.startConnection).not.toHaveBeenCalled();
});

it("retries a failed setup using the same creation ID", async () => {
  defaults();
  rpc.startConnection
    .mockRejectedValueOnce(new Error("Temporarily unavailable"))
    .mockResolvedValue({ connection, brokeringUrl });
  render(<AgentConnections agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "WhatsApp" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Add new connection" }));
  fireEvent.click(await screen.findByRole("button", { name: "Retry setup" }));
  await screen.findByTitle("Connect WhatsApp");
  expect(rpc.startConnection.mock.calls[0][0].id).toBe(rpc.startConnection.mock.calls[1][0].id);
});

it("dismisses setup on an outside click without reopening after a late create response", async () => {
  defaults();
  let finish!: (value: { connection: typeof connection; brokeringUrl: string }) => void;
  rpc.startConnection.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  render(<AgentConnections agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "WhatsApp" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Add new connection" }));
  expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
  await screen.findByRole("status", { name: "Loading connection setup" });
  const backdrop = document.querySelector('[data-slot="dialog-overlay"]')!;
  fireEvent.mouseDown(backdrop);
  fireEvent.mouseUp(backdrop);
  fireEvent.click(backdrop);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await act(async () => {
    finish({ connection, brokeringUrl });
  });
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("renders chat provider IDs in the shared table and copies the agent-scoped ID", async () => {
  defaults();
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  rpc.listConnections.mockResolvedValue({
    connections: [{ ...connection, name: "support", slug: "whatsapp/support" }],
    nextPageToken: "",
  });
  render(<AgentConnections agentId="agent-1" />);
  await screen.findByRole("table", { name: "Chat providers" });
  expect(screen.getByRole("columnheader", { name: "Provider ID" })).toBeTruthy();
  expect(screen.queryByRole("columnheader", { name: "Alias" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Copy provider ID" }));
  await screen.findByText("Copied to clipboard");
  expect(writeText).toHaveBeenCalledWith("whatsapp/support");
});

it("lists the agent's system-managed Tilde connection with only its key action", async () => {
  defaults();
  const tilde = create(ProviderSchema, {
    id: "tilde",
    name: "Tilde",
    iconUrl: "/tilde-mark.svg",
    connectionTypes: [
      { id: "application", name: "Application", capabilities: [Capability.CHANNEL] },
    ],
  });
  rpc.listProviders.mockResolvedValue({ providers: [whatsapp, tilde], nextPageToken: "" });
  rpc.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, {
        id: "tilde-1",
        name: "Support agent",
        slug: "tilde/Support agent",
        providerId: "tilde",
        typeId: "application",
        status: "ready",
        capabilities: [Capability.CHANNEL],
      }),
    ],
    nextPageToken: "",
  });
  tildeRpc.getCredentials.mockResolvedValue({ apiKey: "tilde_chat_secret" });
  render(<AgentConnections agentId="agent-1" />);
  const table = await screen.findByRole("table", { name: "Chat providers" });
  expect(screen.getByRole("button", { name: "WhatsApp" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Tilde" })).toBeNull();
  expect(within(table).getByText("Always enabled")).toBeTruthy();
  expect(within(table).queryByRole("button", { name: "Remove" })).toBeNull();
  fireEvent.click(within(table).getByRole("button", { name: "Reveal API key" }));
  await waitFor(() => expect(tildeRpc.getCredentials).toHaveBeenCalledWith({ agentId: "agent-1" }));
  await waitFor(() =>
    expect(
      (within(screen.getByRole("dialog")).getByLabelText("Tilde chat API key") as HTMLInputElement)
        .value,
    ).toBe("tilde_chat_secret"),
  );
});

it("shows spend, a monthly budget and the spend chart for inference connections", async () => {
  const openai = create(ProviderSchema, {
    id: "openai",
    name: "OpenAI",
    connectionTypes: [{ id: "api", name: "OpenAI API", capabilities: [Capability.INFERENCE] }],
  });
  const key = create(ConnectionSchema, {
    id: "connection-2",
    name: "prod",
    slug: "openai/prod",
    providerId: "openai",
    typeId: "api",
    status: "ready",
    capabilities: [Capability.INFERENCE],
    associatedAgents: [
      { capability: Capability.INFERENCE, id: "agent-1", name: "Agent", alias: "fast" },
    ],
  });
  rpc.listProviders.mockResolvedValue({ providers: [openai], nextPageToken: "" });
  rpc.listConnections.mockResolvedValue({ connections: [key], nextPageToken: "" });
  inferenceRpc.getUsage.mockResolvedValue({
    connections: [
      {
        connectionId: "connection-2",
        requests: 3n,
        inputTokens: 10n,
        outputTokens: 5n,
        costMicros: 1_250_000n,
      },
    ],
    series: [],
    totalCostMicros: 9_000_000n,
  });
  inferenceRpc.listBudgets.mockResolvedValue({
    budgets: [
      {
        id: "budget-1",
        scope: 1,
        scopeId: "agent-1",
        connectionId: "connection-2",
        period: 2,
        limitMicros: 20_000_000n,
        action: 1,
        spentMicros: 1_250_000n,
      },
    ],
  });
  inferenceRpc.setBudget.mockResolvedValue({});
  render(<AgentConnections agentId="agent-1" capability={Capability.INFERENCE} />);
  // Spent cell and the MTD metric both show this month's spend.
  expect(await screen.findAllByText("$1.25")).toHaveLength(2);
  expect(screen.getByText("$20.00 / mo")).toBeTruthy();
  expect(screen.getByText("Inference spend")).toBeTruthy();
  expect(screen.getByText("$9.00")).toBeTruthy();
  fireEvent.click(screen.getByLabelText("Edit budget"));
  const input = screen.getByLabelText("Monthly budget for openai/prod") as HTMLInputElement;
  fireEvent.change(input, { target: { value: "35" } });
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() =>
    expect(inferenceRpc.setBudget).toHaveBeenCalledWith(
      expect.objectContaining({ connectionId: "connection-2", limitMicros: 35_000_000n }),
    ),
  );
});
