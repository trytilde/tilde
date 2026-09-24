import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ApiKeyCreate } from "./api-key-create";
const rpc = vi.hoisted(() => ({ createApiKey: vi.fn(), listAgents: vi.fn() }));
vi.mock("@/client", () => ({ apiKeys: rpc, agents: rpc }));
vi.mock("@/hooks/use-caller", () => ({
  useCaller: () => ({ userId: "user-1", admin: false, groupIds: [], creatable: [1] }),
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it("turns the toggled rows into roles, then shows the secret once", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  rpc.listAgents.mockResolvedValue({
    agents: [{ id: "agent-1", name: "Alpha" }],
    nextPageToken: "",
  });
  rpc.createApiKey.mockResolvedValue({ apiKey: { id: "k" }, secret: "tilde_key_creation_secret" });
  const done = vi.fn();
  render(<ApiKeyCreate onDone={done} />);
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: " Backend " } });
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Agents" })).getByRole("tab", { name: "Editor" }),
  );
  // Non-administrators cannot reach every agent.
  expect(
    within(screen.getByRole("tablist", { name: "Agents scope" }))
      .getByRole("tab", { name: "All" })
      .getAttribute("aria-disabled"),
  ).toBe("true");
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Manage agent deployments" })).getByRole("tab", {
      name: "Yes",
    }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Select agents for Manage agent deployments" }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Alpha" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  fireEvent.click(await screen.findByRole("button", { name: "Create key" }));
  await waitFor(() =>
    expect(rpc.createApiKey).toHaveBeenCalledWith({
      name: "Backend",
      roleIds: ["agent/agent-1/deployer"],
    }),
  );
  const secret = await screen.findByLabelText("API key");
  expect((secret as HTMLInputElement).value).toBe("tilde_key_creation_secret");
  fireEvent.click(screen.getByRole("button", { name: "Copy key" }));
  await screen.findByRole("button", { name: "Copied" });
  expect(writeText).toHaveBeenCalledWith("tilde_key_creation_secret");
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  expect(done).toHaveBeenCalled();
});
