import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  PrincipalType,
  ResourceKind,
} from "@trytilde/contracts/tilde/types/v1/authorization_pb.js";
import { AgentShare } from "./agent-share";

const rpc = vi.hoisted(() => ({
  getAccess: vi.fn(),
  listRoleAssignments: vi.fn(),
  listRoles: vi.fn(),
  listGroups: vi.fn(),
  listUsers: vi.fn(),
  assignRole: vi.fn(),
  revokeRole: vi.fn(),
}));
vi.mock("@/client", () => ({ iam: rpc }));
const resource = { kind: ResourceKind.AGENT, id: "agent-1" };
const role = (slug: string, description = "") => ({
  id: `agent/agent-1/${slug}`,
  name: slug[0].toUpperCase() + slug.slice(1),
  description,
});
const reader = role("reader", "Can view the agent.");
const editor = role("editor", "Can change and share the agent.");
const deployer = role("deployer", "Can deploy the agent.");
const ops = {
  principal: { type: PrincipalType.GROUP, id: "local:ops" },
  label: "Ops",
  roles: [reader],
};
const everyone = {
  principal: { type: PrincipalType.GROUP, id: "tilde_system:user" },
  label: "All users",
  roles: [reader],
};
beforeEach(() => {
  rpc.listRoleAssignments.mockResolvedValue({ assignments: [ops] });
  rpc.listRoles.mockResolvedValue({ roles: [reader, editor, deployer] });
  rpc.listGroups.mockResolvedValue({ groups: [] });
  rpc.listUsers.mockResolvedValue({ users: [] });
  rpc.assignRole.mockResolvedValue({});
  rpc.revokeRole.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
function open() {
  render(<AgentShare agentId="agent-1" />);
  fireEvent.click(screen.getByRole("button", { name: "Share agent" }));
}
function openMenu(name: string) {
  fireEvent.click(screen.getByRole("button", { name }));
}
function pick(item: HTMLElement) {
  fireEvent.click(item);
}
async function choose(combobox: string, option: string) {
  fireEvent.click(screen.getByRole("combobox", { name: combobox }));
  const item = await screen.findByRole("option", { name: option });
  fireEvent.pointerDown(item, { pointerType: "mouse" });
  fireEvent.click(item);
}
it("limits an editor to the roles within reach and locks rows above them", async () => {
  rpc.getAccess.mockResolvedValue({ actions: ["view", "edit", "share"] });
  const boss = {
    principal: { type: PrincipalType.USER, id: "user-9" },
    label: "Boss",
    roles: [reader, deployer],
  };
  rpc.listRoleAssignments.mockResolvedValue({ assignments: [ops, boss] });
  open();
  await screen.findByText("Boss");
  expect(screen.getByRole("button", { name: "Access for Ops" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Access for Boss" })).toBeNull();
  expect(screen.getByText("Reader, Deployer")).toBeTruthy();
  // The menu explains each role and ticks the ones held; deployer is beyond an editor's reach.
  openMenu("Access for Ops");
  const readerItem = await screen.findByRole("menuitemcheckbox", { name: /Reader/ });
  expect(readerItem.getAttribute("aria-checked")).toBe("true");
  expect(screen.getByText("Can change and share the agent.")).toBeTruthy();
  expect(
    screen.getByRole("menuitemcheckbox", { name: /Editor/ }).getAttribute("aria-checked"),
  ).toBe("false");
  expect(screen.queryByRole("menuitemcheckbox", { name: /Deployer/ })).toBeNull();
  pick(screen.getByRole("menuitemcheckbox", { name: /Editor/ }));
  await waitFor(() =>
    expect(rpc.assignRole).toHaveBeenCalledWith({
      roleId: "agent/agent-1/editor",
      principal: ops.principal,
    }),
  );
});
it("shows who has access read-only to a viewer, with the everyone role as visibility", async () => {
  rpc.getAccess.mockResolvedValue({ actions: ["view"] });
  rpc.listRoleAssignments.mockResolvedValue({ assignments: [ops, everyone] });
  open();
  await screen.findByText("Ops");
  expect(screen.queryByText("All users")).toBeNull();
  expect(screen.getByText("Anyone can view")).toBeTruthy();
  expect(screen.queryByPlaceholderText("Invite people or groups")).toBeNull();
  expect(screen.queryByRole("button", { name: "Access for Ops" })).toBeNull();
});
it("lets a deployer invite a user as reader, add deployer, remove access and change visibility", async () => {
  rpc.getAccess.mockResolvedValue({ actions: ["view", "deploy", "share"] });
  rpc.listUsers.mockResolvedValue({
    users: [{ id: "user-2", subject: "rafiek", email: "rafiek@faro.co", displayName: "Rafiek" }],
  });
  open();
  await screen.findByText("Ops");
  expect((screen.getByRole("button", { name: "Invite" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByPlaceholderText("Invite people or groups"), {
    target: { value: "raf" },
  });
  fireEvent.click(await screen.findByRole("option", { name: /Rafiek/ }));
  expect(rpc.listUsers).toHaveBeenCalledWith({ search: "raf", pageSize: 6 }, expect.anything());
  const invited = {
    principal: { type: PrincipalType.USER, id: "user-2" },
    label: "Rafiek",
    roles: [reader],
  };
  rpc.listRoleAssignments.mockResolvedValue({ assignments: [ops, invited] });
  fireEvent.click(screen.getByRole("button", { name: "Invite" }));
  await waitFor(() =>
    expect(rpc.assignRole).toHaveBeenCalledWith({
      roleId: "agent/agent-1/reader",
      principal: invited.principal,
    }),
  );
  await screen.findByText("Rafiek");
  openMenu("Access for Rafiek");
  expect(screen.queryByRole("menuitemcheckbox", { name: /Editor/ })).toBeNull();
  pick(await screen.findByRole("menuitemcheckbox", { name: /Deployer/ }));
  await waitFor(() =>
    expect(rpc.assignRole).toHaveBeenCalledWith({
      roleId: "agent/agent-1/deployer",
      principal: invited.principal,
    }),
  );
  pick(screen.getByRole("menuitem", { name: "Remove access" }));
  await waitFor(() =>
    expect(rpc.revokeRole).toHaveBeenCalledWith({
      roleId: "agent/agent-1/reader",
      principal: invited.principal,
    }),
  );
  await choose("General agent visibility", "Anyone can view");
  await waitFor(() =>
    expect(rpc.assignRole).toHaveBeenCalledWith({
      roleId: "agent/agent-1/reader",
      principal: { type: PrincipalType.GROUP, id: "tilde_system:user" },
    }),
  );
  expect(resource.id).toBe("agent-1");
});
