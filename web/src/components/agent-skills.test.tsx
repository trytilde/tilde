import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { Capability, ConnectionSchema } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ConnectionSkillsSchema } from "@trytilde/contracts/tilde/management/v1/skills_pb.js";
import {
  SkillSchema,
  SkillSourceKind,
  SkillSourceSchema,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import {
  AgentDeploymentSchema,
  DeploymentRouting,
  DeploymentSchema,
  DeploymentStatus,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { DeploymentSkillSchema } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { AgentSkills } from "./agent-skills";

const rpc = vi.hoisted(() => ({
  skills: {
    listAgentSkills: vi.fn(),
    listSkillSources: vi.fn(),
    listSkills: vi.fn(),
    assignSkillSource: vi.fn(),
    unassignSkillSource: vi.fn(),
    unassignSkill: vi.fn(),
    setSkillSourceEnabled: vi.fn(),
    setSkillEnabled: vi.fn(),
    listCatalog: vi.fn(),
  },
  deployments: { getDeployment: vi.fn(), getDeploymentContents: vi.fn() },
  connections: {
    listConnections: vi.fn(),
    assignCapability: vi.fn(),
    unassignCapability: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

const catalog = create(SkillSourceSchema, {
  id: "src-catalog",
  name: "Tilde runtime",
  kind: SkillSourceKind.CATALOG,
  skillCount: 2,
});
const git = create(SkillSourceSchema, {
  id: "src-git",
  name: "Support playbooks",
  kind: SkillSourceKind.GIT,
  skillCount: 3,
});
const editor = create(SkillSourceSchema, {
  id: "src-editor",
  name: "Drafts",
  kind: SkillSourceKind.EDITOR,
  skillCount: 1,
});
const skill = (id: string, name: string, source: typeof catalog) =>
  create(SkillSchema, {
    id,
    name,
    sourceId: source.id,
    sourceName: source.name,
    sourceKind: source.kind,
  });
const channels = skill("s-channels", "tilde-channels", catalog);
const refunds = skill("s-refunds", "refunds", git);
const escalation = skill("s-escalation", "escalation", git);
const draft = skill("s-draft", "draft-skill", editor);

function renderTab() {
  const root = createRootRoute({ component: () => <AgentSkills agentId="agent-1" /> });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.deployments.getDeployment.mockResolvedValue({ deployment: undefined, deployments: [] });
  rpc.skills.listSkillSources.mockResolvedValue({ sources: [catalog, git, editor] });
  rpc.skills.listSkills.mockResolvedValue({ skills: [channels, refunds, escalation, draft] });
  rpc.skills.assignSkillSource.mockResolvedValue({});
  rpc.skills.unassignSkillSource.mockResolvedValue({});
  rpc.skills.unassignSkill.mockResolvedValue({});
  rpc.skills.setSkillSourceEnabled.mockResolvedValue({});
  rpc.skills.setSkillEnabled.mockResolvedValue({});
  rpc.skills.listCatalog.mockResolvedValue({ groups: [] });
  rpc.connections.listConnections.mockResolvedValue({
    connections: [
      create(ConnectionSchema, { id: "c-zendesk", name: "Zendesk", status: "ready" }),
      create(ConnectionSchema, { id: "c-github", name: "GitHub", status: "ready" }),
    ],
    nextPageToken: "",
  });
  rpc.connections.assignCapability.mockResolvedValue({});
  rpc.connections.unassignCapability.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("switches a group on or off as a whole, and each skill of an enabled group alone", async () => {
  rpc.skills.listAgentSkills.mockResolvedValue({
    sources: [catalog, git],
    skills: [],
    connections: [],
    groupSkills: [channels, refunds, escalation],
    disabledSourceIds: ["src-catalog"],
    disabledSkillIds: ["s-escalation"],
  });
  renderTab();
  const table = await screen.findByRole("table", { name: "Assigned skills" });
  const runtime = await within(table).findByRole("row", { name: "Group Tilde runtime" });
  const support = within(table).getByRole("row", { name: "Group Support playbooks" });
  const use = (name: string) => within(table).getByRole("switch", { name: `Use ${name}` });
  const state = (name: string) => [
    use(name).getAttribute("aria-checked"),
    use(name).hasAttribute("data-disabled"),
  ];
  // Tilde runtime is off: its skills are off and cannot switch until the group is on.
  expect(within(runtime).getByText("0 of 1 skill")).toBeTruthy();
  expect(state("tilde-channels")).toEqual(["false", true]);
  // Support playbooks is on, with escalation switched off on its own.
  expect(within(support).getByText("1 of 2 skills")).toBeTruthy();
  expect(state("refunds")).toEqual(["true", false]);
  expect(state("escalation")).toEqual(["false", false]);
  fireEvent.click(use("escalation"));
  await waitFor(() =>
    expect(rpc.skills.setSkillEnabled).toHaveBeenCalledWith({
      agentId: "agent-1",
      skillId: "s-escalation",
      enabled: true,
    }),
  );
  await waitFor(() => expect(use("refunds").hasAttribute("data-disabled")).toBe(false));
  fireEvent.click(use("refunds"));
  await waitFor(() =>
    expect(rpc.skills.setSkillEnabled).toHaveBeenCalledWith({
      agentId: "agent-1",
      skillId: "s-refunds",
      enabled: false,
    }),
  );
  await waitFor(() =>
    expect(
      within(runtime)
        .getByRole("switch", { name: "Use every skill in Tilde runtime" })
        .hasAttribute("data-disabled"),
    ).toBe(false),
  );
  fireEvent.click(
    within(runtime).getByRole("switch", { name: "Use every skill in Tilde runtime" }),
  );
  await waitFor(() =>
    expect(rpc.skills.setSkillSourceEnabled).toHaveBeenCalledWith({
      agentId: "agent-1",
      sourceId: "src-catalog",
      enabled: true,
    }),
  );
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "Add skills" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );

  // Use existing offers only the groups the agent does not have.
  fireEvent.click(screen.getByRole("button", { name: "Add skills" }));
  // Adding new skills happens on the Skills page.
  expect(
    (await screen.findByRole("menuitem", { name: "Add a new skill" })).getAttribute("href"),
  ).toBe("/skills");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Choose existing" }));
  const picker = await screen.findByRole("listbox", { name: "Available groups" });
  expect(within(picker).queryByRole("option", { name: /Tilde runtime/ })).toBeNull();
  fireEvent.click(within(picker).getByRole("option", { name: /Drafts.*1 skill/ }));
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() =>
    expect(rpc.skills.assignSkillSource).toHaveBeenCalledWith({
      agentId: "agent-1",
      sourceId: "src-editor",
    }),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

  fireEvent.click(within(runtime).getByRole("button", { name: "Remove Tilde runtime" }));
  // Removing asks first; nothing is removed until the dialog is confirmed.
  fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
  await waitFor(() =>
    expect(rpc.skills.unassignSkillSource).toHaveBeenCalledWith({
      agentId: "agent-1",
      sourceId: "src-catalog",
    }),
  );
});

it("marks what a connection gives and removes the connection's skills", async () => {
  rpc.skills.listAgentSkills.mockResolvedValue({
    sources: [],
    skills: [],
    connections: [
      create(ConnectionSkillsSchema, {
        connectionId: "c-github",
        connectionName: "GitHub",
        status: "pending",
        sources: [git],
      }),
    ],
    groupSkills: [escalation, refunds],
  });
  renderTab();
  const table = await screen.findByRole("table", { name: "Assigned skills" });
  // What the connection links shows under each skill group, marked with the connection.
  const support = await within(table).findByRole("row", { name: "Group Support playbooks" });
  expect(within(support).getByText("via GitHub")).toBeTruthy();
  const linked = screen.getByRole("region", { name: "Connections" });
  expect(within(linked).getByText("3 skills · pending")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Remove GitHub" }));
  // Removing asks first; nothing is unassigned until the dialog is confirmed.
  fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
  await waitFor(() =>
    expect(rpc.connections.unassignCapability).toHaveBeenCalledWith({
      connectionId: "c-github",
      assignment: { capability: Capability.SKILLS, agentId: "agent-1" },
    }),
  );
});

it("lists the newest registered deployment's bundled skills when none is serving", async () => {
  rpc.skills.listAgentSkills.mockResolvedValue({
    sources: [],
    skills: [],
    connections: [],
    groupSkills: [],
  });
  const deployment = (id: string, seconds: bigint, status = DeploymentStatus.REGISTERED) =>
    create(AgentDeploymentSchema, { id, agentId: "agent-1", status, createdAt: { seconds } });
  rpc.deployments.getDeployment.mockResolvedValue({
    deployment: create(DeploymentSchema, { agentId: "agent-1" }),
    deployments: [
      deployment("dep-old", 1n),
      deployment("dep-new", 2n),
      deployment("dep-retired", 3n, DeploymentStatus.RETIRED),
    ],
  });
  rpc.deployments.getDeploymentContents.mockResolvedValue({
    prompts: [],
    skills: [
      create(DeploymentSkillSchema, {
        skillId: "s-code",
        name: "refunds",
        versionId: "sv-1",
        number: 2,
        description: "Issue refunds within policy.",
        origin: "src/mastra/skills/refunds",
      }),
    ],
  });
  renderTab();
  const table = await screen.findByRole("table", { name: "Assigned skills" });
  const group = await within(table).findByRole("row", { name: /^Group / });
  expect(within(group).getByText("Bundled").getAttribute("title")).toMatch(
    /takes precedence over an assigned skill/,
  );
  // Bundled skills ship with the deployment and are always used: nothing to switch or remove.
  expect(within(table).queryAllByRole("switch")).toHaveLength(0);
  expect(within(group).queryByRole("button", { name: /^Remove/ })).toBeNull();
  expect(rpc.deployments.getDeploymentContents).toHaveBeenCalledWith(
    { agentId: "agent-1", deploymentId: "dep-new" },
    expect.anything(),
  );
  const row = within(table).getByText("refunds").closest("tr")!;
  expect(within(row).getByText("v2")).toBeTruthy();
  expect(within(row).getByText("Issue refunds within policy.")).toBeTruthy();
  // Bundled skills open on the (read-only) skill page.
  const link = within(row).getByRole("link", { name: "refunds" });
  expect(link.getAttribute("href")).toBe("/skills/s-code");
  expect(link.getAttribute("title")).toBe("Declared at src/mastra/skills/refunds");
});

it("groups bundled skills by every deployment that weighted routing sends traffic to", async () => {
  rpc.skills.listAgentSkills.mockResolvedValue({
    sources: [],
    skills: [],
    connections: [],
    groupSkills: [],
  });
  const deployment = (id: string, label: string, trafficWeight: number) =>
    create(AgentDeploymentSchema, {
      id,
      agentId: "agent-1",
      label,
      commitSha: `${id}-sha-0000000`,
      status: DeploymentStatus.REGISTERED,
      trafficWeight,
      serving: trafficWeight > 0,
    });
  rpc.deployments.getDeployment.mockResolvedValue({
    deployment: create(DeploymentSchema, {
      agentId: "agent-1",
      routing: DeploymentRouting.WEIGHTED,
    }),
    deployments: [
      deployment("canary", "Canary", 10),
      deployment("stable", "Stable", 90),
      deployment("idle", "Idle", 0),
    ],
  });
  const bundledSkill = (name: string) =>
    create(DeploymentSkillSchema, {
      skillId: `s-${name}`,
      name,
      versionId: `sv-${name}`,
      number: 1,
    });
  rpc.deployments.getDeploymentContents.mockImplementation(({ deploymentId }) =>
    Promise.resolve({
      prompts: [],
      skills: [bundledSkill(deploymentId === "canary" ? "refunds-v2" : "refunds")],
    }),
  );
  renderTab();
  const table = await screen.findByRole("table", { name: "Assigned skills" });
  const canary = await within(table).findByRole("row", { name: "Group Canary" });
  const stable = within(table).getByRole("row", { name: "Group Stable" });
  expect(within(canary).getByText(/10% of traffic/)).toBeTruthy();
  expect(within(stable).getByText(/90% of traffic/)).toBeTruthy();
  expect(within(table).getByText("refunds-v2")).toBeTruthy();
  expect(within(table).getByText("refunds")).toBeTruthy();
  expect(within(table).queryByRole("row", { name: /Idle/ })).toBeNull();
  expect(rpc.deployments.getDeploymentContents).toHaveBeenCalledTimes(2);
});
