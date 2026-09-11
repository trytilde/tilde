import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, within, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  ChannelAccessSchema,
  ChannelIdentitySchema,
  ChannelAccessMode,
  IdentityType,
} from "@/gen/tilde/types/v1/access_pb.js";
import { AgentIam } from "./agent-iam";
const rpc = vi.hoisted(() => ({
  listChannelAccess: vi.fn(),
  listChannelIdentities: vi.fn(),
  setChannelAccess: vi.fn(),
  setIdentityAccess: vi.fn(),
  requestIdentityVerification: vi.fn(),
}));
vi.mock("@/client", () => ({ agentAccess: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const route = create(ChannelAccessSchema, {
  connectionId: "account-1",
  agentId: "agent-1",
  accountName: "Personal assistant",
  providerName: "WhatsApp",
  providerId: "whatsapp",
  mode: ChannelAccessMode.PRIVATE,
  connectionStatus: "ready",
  identityTypes: [IdentityType.PHONE_NUMBER],
  verificationSupported: true,
});
const sender = create(ChannelIdentitySchema, {
  id: "sender-1",
  connectionId: route.connectionId,
  value: "+12025550101",
  identityType: IdentityType.PHONE_NUMBER,
  allowed: true,
  verifiedAt: { seconds: 1n },
});
function setup() {
  let mode = ChannelAccessMode.PRIVATE;
  rpc.listChannelAccess.mockImplementation(async () => ({
    routes: [{ ...route, mode }],
    nextPageToken: "",
  }));
  rpc.listChannelIdentities.mockResolvedValue({ identities: [sender], nextPageToken: "" });
  rpc.setChannelAccess.mockImplementation(async (req) => {
    mode = req.mode;
    return {};
  });
  rpc.setIdentityAccess.mockResolvedValue({});
}
function selectMode(mode: string) {
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "WhatsApp access mode" })).getByRole("tab", {
      name: mode,
    }),
  );
}
it("renders grouped accounts and address/badge rows and persists modes and allow controls", async () => {
  setup();
  render(<AgentIam agentId="agent-1" />);
  await screen.findByText("Personal assistant");
  expect(screen.queryByRole("columnheader", { name: "Verification" })).toBeNull();
  expect(screen.queryByText("WhatsApp")).toBeNull();
  const row = screen.getByText(sender.value).closest("tr")!;
  expect(within(row).getByText("Verified")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "Allow +12025550101 through WhatsApp" }));
  await waitFor(() =>
    expect(rpc.setIdentityAccess).toHaveBeenCalledWith({
      agentId: "agent-1",
      connectionId: "account-1",
      identityId: "sender-1",
      allowed: false,
    }),
  );
  await waitFor(() =>
    expect(screen.getByRole("tab", { name: "Public" }).getAttribute("aria-disabled")).not.toBe(
      "true",
    ),
  );
  selectMode("Public");
  await waitFor(() => expect(screen.queryByText(sender.value)).toBeNull());
  expect(rpc.setChannelAccess).toHaveBeenCalledWith({
    agentId: "agent-1",
    connectionId: "account-1",
    mode: ChannelAccessMode.PUBLIC,
  });
  expect(screen.queryByRole("button", { name: "Add identity" })).toBeNull();
  selectMode("Private");
  await screen.findByText(sender.value);
  fireEvent.click(screen.getByRole("button", { name: "Collapse Personal assistant" }));
  expect(screen.queryByText(sender.value)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Expand Personal assistant" }));
  expect(screen.getByText(sender.value)).toBeTruthy();
});
it("requests real verification with a typed string and no name or approval URL exposed", async () => {
  setup();
  rpc.requestIdentityVerification.mockResolvedValue({
    verificationId: "verify-1",
    status: "delivered",
  });
  render(<AgentIam agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "Add identity" }));
  expect(screen.queryByLabelText("Name")).toBeNull();
  fireEvent.change(screen.getByLabelText("Phone number"), { target: { value: "+12025550222" } });
  fireEvent.click(screen.getByRole("button", { name: "Send verification" }));
  await waitFor(() =>
    expect(rpc.requestIdentityVerification).toHaveBeenCalledWith(
      expect.objectContaining({
        agentId: "agent-1",
        connectionId: "account-1",
        identityType: IdentityType.PHONE_NUMBER,
        value: "+12025550222",
      }),
    ),
  );
  await screen.findByText(/Verification message sent to/);
  expect(screen.queryByTitle("Recipient approval preview")).toBeNull();
});
it("uses provider-declared identity types and restricts GitHub to public or disabled", async () => {
  setup();
  rpc.listChannelAccess.mockResolvedValue({
    routes: [
      {
        ...route,
        providerName: "GitHub",
        providerId: "github",
        verificationSupported: false,
        mode: ChannelAccessMode.PUBLIC,
        identityTypes: [IdentityType.USERNAME],
      },
    ],
    nextPageToken: "",
  });
  render(<AgentIam agentId="agent-1" />);
  await screen.findByText("Personal assistant");
  expect(
    within(screen.getByRole("tablist", { name: "GitHub access mode" })).queryByRole("tab", {
      name: "Private",
    }),
  ).toBeNull();
  expect(screen.queryByRole("button", { name: "Add identity" })).toBeNull();
});
