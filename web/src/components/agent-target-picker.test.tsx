import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AgentTargetPicker } from "./agent-target-picker";
const rpc = vi.hoisted(() => ({ listAgents: vi.fn(), getAgent: vi.fn() }));
vi.mock("@/client", () => ({ agents: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
function Picker() {
  const [value, setValue] = useState<string[]>([]);
  return (
    <AgentTargetPicker
      label="Read agents"
      value={value}
      onConfirm={async (ids) => {
        setValue(ids);
      }}
      disabled={false}
    />
  );
}
it("searches the server, paginates matching agents, and keeps the selected avatar removable", async () => {
  rpc.listAgents.mockImplementation(async ({ search, pageToken }) => {
    if (search) return { agents: [{ id: "grace", name: "Grace" }], nextPageToken: "" };
    if (pageToken) return { agents: [{ id: "alan", name: "Alan" }], nextPageToken: "" };
    return { agents: [{ id: "ada", name: "Ada" }], nextPageToken: "next" };
  });
  render(<Picker />);
  fireEvent.click(screen.getByRole("button", { name: "Select agents for Read agents" }));
  await screen.findByRole("button", { name: "Ada" });
  fireEvent.click(screen.getByRole("button", { name: "Load more" }));
  await screen.findByRole("button", { name: "Alan" });
  expect(rpc.listAgents).toHaveBeenLastCalledWith({ search: "", pageSize: 20, pageToken: "next" });
  fireEvent.change(screen.getByRole("textbox", { name: "Search agents" }), {
    target: { value: "Grace" },
  });
  fireEvent.click(await screen.findByRole("button", { name: "Grace" }));
  expect(rpc.listAgents).toHaveBeenLastCalledWith({ search: "Grace", pageSize: 20, pageToken: "" });
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "Remove Grace from Read agents" }));
  expect(screen.queryByRole("button", { name: "Remove Grace from Read agents" })).toBeNull();
});

it("keeps selections local until confirmation and keeps failed confirmations open for retry", async () => {
  rpc.listAgents.mockResolvedValue({ agents: [{ id: "ada", name: "Ada" }], nextPageToken: "" });
  const confirm = vi
    .fn()
    .mockRejectedValueOnce(new Error("Save failed"))
    .mockResolvedValueOnce(undefined);
  render(<AgentTargetPicker label="Read agents" value={[]} onConfirm={confirm} disabled={false} />);
  fireEvent.click(screen.getByRole("button", { name: "Select agents for Read agents" }));
  fireEvent.click(await screen.findByRole("button", { name: "Ada" }));
  expect(confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  await screen.findByText("Save failed");
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Ada" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(confirm).toHaveBeenLastCalledWith(["ada"]);
});
