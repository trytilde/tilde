import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  Capability,
  ConnectionSchema,
  ProviderSchema,
} from "@/gen/tilde/types/v1/connections_pb.js";
import { AgentConnections } from "./agent-connections";

const rpc = vi.hoisted(() => ({
  listProviders: vi.fn(),
  listConnections: vi.fn(),
  startConnection: vi.fn(),
  assignCapability: vi.fn(),
  unassignCapability: vi.fn(),
  reconnect: vi.fn(),
}));
vi.mock("@/client", () => ({ connections: rpc }));
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
      : {
          providers: [whatsapp, create(ProviderSchema, { id: "not-chat", name: "Tools provider" })],
          nextPageToken: "next",
        },
  );
  render(<AgentConnections agentId="agent-1" />);
  const pill = await screen.findByRole("button", { name: "Custom chat" });
  expect(pill.className).toContain("rounded-full");
  expect(screen.queryByRole("button", { name: "Tools provider" })).toBeNull();
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
  rpc.listConnections.mockImplementation(async ({ unassignedChannelOnly, pageToken }) =>
    !unassignedChannelOnly
      ? { connections: [], nextPageToken: "" }
      : pageToken
        ? { connections: [connection], nextPageToken: "" }
        : {
            connections: [
              { ...connection, id: "other", name: "Other provider", providerId: "custom" },
            ],
            nextPageToken: "more",
          },
  );
  rpc.assignCapability.mockResolvedValue({ connection });
  render(<AgentConnections agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "WhatsApp" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Use existing connection" }));
  const select = await screen.findByRole("button", { name: /Support WhatsApp/ });
  expect(screen.queryByText("Other provider")).toBeNull();
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
