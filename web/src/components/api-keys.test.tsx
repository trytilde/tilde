import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ApiKeysPage } from "./api-keys";
const rpc = vi.hoisted(() => ({
  listApiKeys: vi.fn(),
  createApiKey: vi.fn(),
  revokeApiKey: vi.fn(),
  listConnections: vi.fn(),
}));
vi.mock("@/client", () => ({ apiKeys: rpc, connections: rpc, agents: rpc }));
const key = {
  id: "key-1",
  name: "Backend",
  prefix: "tilde_key_abcdefgh",
  roles: [{ id: "agents/reader", name: "Reader of all agents", description: "" }],
  createdAt: { seconds: 1n, nanos: 0 },
};
beforeEach(() => {
  rpc.listConnections.mockResolvedValue({ connections: [] });
  rpc.listApiKeys.mockResolvedValue({ apiKeys: [key], nextPageToken: "" });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it("keeps a failed revocation retryable and refreshes status after success", async () => {
  rpc.revokeApiKey
    .mockRejectedValueOnce(new Error("Temporarily unavailable"))
    .mockResolvedValueOnce({});
  render(<ApiKeysPage />);
  fireEvent.click(await screen.findByRole("button", { name: "Revoke Backend" }));
  fireEvent.click(screen.getByRole("button", { name: "Revoke key" }));
  await screen.findByText("Temporarily unavailable");
  rpc.listApiKeys.mockResolvedValue({
    apiKeys: [{ ...key, revokedAt: { seconds: 2n, nanos: 0 } }],
    nextPageToken: "",
  });
  fireEvent.click(screen.getByRole("button", { name: "Revoke key" }));
  await screen.findByText("Revoked");
  expect(rpc.revokeApiKey).toHaveBeenLastCalledWith({ id: key.id });
  expect(screen.queryByRole("button", { name: "Revoke Backend" })).toBeNull();
});
