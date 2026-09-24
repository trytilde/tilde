import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { GroupDetail } from "./group-detail";

const rpc = vi.hoisted(() => ({
  getGroup: vi.fn(),
  listGroupMembers: vi.fn(),
  listUsers: vi.fn(),
  addGroupMember: vi.fn(),
  removeGroupMember: vi.fn(),
  deleteGroup: vi.fn(),
}));
vi.mock("@/client", () => ({ iam: rpc }));
vi.mock("@/hooks/use-caller", () => ({
  useCaller: () => ({ userId: "user-1", admin: true, groupIds: [], creatable: [1] }),
}));
beforeEach(() => {
  rpc.getGroup.mockResolvedValue({ group: { id: "local:ops", name: "Ops", source: 3 } });
  rpc.listGroupMembers.mockResolvedValue({
    members: [{ userId: "user-2", label: "rafiek@faro.co" }],
    nextPageToken: "",
  });
  rpc.listUsers.mockResolvedValue({
    users: [
      { id: "user-3", subject: "grace", email: "grace@faro.co", displayName: "Grace" },
      { id: "user-4", subject: "hal", email: "hal@faro.co" },
    ],
  });
  rpc.addGroupMember.mockResolvedValue({});
  rpc.removeGroupMember.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it("names the group, lists members with removal, and adds users as chips", async () => {
  render(<GroupDetail groupId="local:ops" onDeleted={vi.fn()} />);
  await screen.findByRole("heading", { name: "Ops" });
  await screen.findByText("rafiek@faro.co");
  fireEvent.click(screen.getByRole("button", { name: "Remove rafiek@faro.co" }));
  await waitFor(() =>
    expect(rpc.removeGroupMember).toHaveBeenCalledWith({ groupId: "local:ops", userId: "user-2" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add users" }));
  const input = await screen.findByRole("combobox", { name: "Users" });
  fireEvent.change(input, { target: { value: "gr" } });
  // Enter takes the highlighted match; a click takes another; the chip's × drops one again.
  await screen.findByRole("option", { name: "Grace" });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("button", { name: "Remove Grace" })).toBeTruthy();
  fireEvent.change(input, { target: { value: "ha" } });
  fireEvent.click(await screen.findByRole("option", { name: "hal@faro.co" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove Grace" }));
  fireEvent.click(screen.getByRole("button", { name: "Add 1 user" }));
  await waitFor(() =>
    expect(rpc.addGroupMember).toHaveBeenCalledWith({ groupId: "local:ops", userId: "user-4" }),
  );
  expect(rpc.addGroupMember).toHaveBeenCalledTimes(1);
});
it("asks before deleting the group", async () => {
  const onDeleted = vi.fn();
  rpc.deleteGroup.mockResolvedValue({});
  render(<GroupDetail groupId="local:ops" onDeleted={onDeleted} />);
  fireEvent.click(await screen.findByRole("button", { name: "Delete group" }));
  expect(rpc.deleteGroup).not.toHaveBeenCalled();
  fireEvent.click(await screen.findByRole("button", { name: "Delete group" }));
  await waitFor(() => expect(rpc.deleteGroup).toHaveBeenCalledWith({ id: "local:ops" }));
  expect(onDeleted).toHaveBeenCalled();
});
