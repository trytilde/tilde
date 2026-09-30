import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
  useParams,
} from "@tanstack/react-router";
import {
  SkillSchema,
  SkillSourceKind,
  SkillSourceSchema,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { SkillsPage } from "./skills-page";

const rpc = vi.hoisted(() => ({
  skills: {
    listSkills: vi.fn(),
    listSkillSources: vi.fn(),
    listCatalog: vi.fn(),
    addGitSource: vi.fn(),
    createEditorSource: vi.fn(),
    createSkill: vi.fn(),
    syncSkillSource: vi.fn(),
    deleteSkillSource: vi.fn(),
    deleteSkill: vi.fn(),
  },
  agents: { listAgents: vi.fn() },
}));
vi.mock("@/client", () => rpc);

const git = create(SkillSourceSchema, {
  id: "src-git",
  name: "Support playbooks",
  kind: SkillSourceKind.GIT,
  syncError: "repository not found",
});
const catalog = create(SkillSourceSchema, {
  id: "src-catalog",
  name: "Tilde runtime",
  kind: SkillSourceKind.CATALOG,
  catalogGroup: "tilde-runtime",
});
const drafts = create(SkillSourceSchema, {
  id: "src-drafts",
  name: "Drafts",
  kind: SkillSourceKind.EDITOR,
});
const skill = (id: string, name: string, source: typeof git) =>
  create(SkillSchema, {
    id,
    name,
    sourceId: source.id,
    sourceName: source.name,
    sourceKind: source.kind,
    latest: { number: 2, description: `${name} description` },
  });

function renderPage() {
  const root = createRootRoute({ component: Outlet });
  const index = createRoute({ getParentRoute: () => root, path: "/", component: SkillsPage });
  const source = createRoute({
    getParentRoute: () => root,
    path: "/skills",
    component: () => <p>Skills list</p>,
  });
  const catalogPage = createRoute({
    getParentRoute: () => root,
    path: "/skills/catalog",
    component: () => <p>Catalog page</p>,
  });
  const detail = createRoute({
    getParentRoute: () => root,
    path: "/skills/$skillId",
    component: () => <p>Skill {useParams({ strict: false }).skillId}</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([index, source, catalogPage, detail]),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({ agents: [], nextPageToken: "" });
  rpc.skills.listSkillSources.mockResolvedValue({ sources: [git, catalog, drafts] });
  rpc.skills.listCatalog.mockResolvedValue({
    groups: [{ id: "tilde-runtime", iconUrl: "/tilde-mark.svg" }],
  });
  rpc.skills.listSkills.mockResolvedValue({
    skills: [
      skill("s1", "refunds", git),
      skill("s2", "escalation", git),
      skill("s3", "tilde-channels", catalog),
    ],
  });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("groups skills under their group, collapses a group and searches within groups", async () => {
  renderPage();
  const table = await screen.findByRole("table", { name: "Skills" });
  await within(table).findByRole("link", { name: "tilde-channels" });
  const names = () =>
    within(table)
      .getAllByRole("row")
      .slice(1)
      .map(
        (row) =>
          row.getAttribute("aria-label") ?? within(row).queryAllByRole("link")[0]?.textContent,
      );
  // Groups sort by name and members by name; an empty editor group still shows.
  expect(names()).toEqual([
    "Group Drafts",
    undefined,
    "Group Support playbooks",
    "escalation",
    "refunds",
    "Group Tilde runtime",
    "tilde-channels",
  ]);
  expect(within(table).getByText("No skills in this group yet.")).toBeTruthy();
  // A catalog group shows its provider's logo; git and editor groups keep their kind icon.
  const runtime = within(table).getByRole("row", { name: "Group Tilde runtime" });
  await waitFor(() =>
    expect(runtime.querySelector("img")?.getAttribute("src")).toBe("/tilde-mark.svg"),
  );
  expect(within(table).getByRole("row", { name: "Group Drafts" }).querySelector("img")).toBeNull();
  const support = within(table).getByRole("row", { name: "Group Support playbooks" });
  expect(within(support).getByText("Git")).toBeTruthy();
  expect(within(support).getByText("2")).toBeTruthy();
  // Groups have no page of their own; their actions live on the row.
  expect(within(support).queryByRole("link", { name: "Support playbooks" })).toBeNull();
  expect(within(table).getByText("refunds description")).toBeTruthy();
  const status = screen.getByRole("status");
  expect(within(status).getByText("Support playbooks")).toBeTruthy();
  // Only groups the caller edits offer a new skill.
  await within(table).findByRole("button", { name: "New skill in Drafts" });
  expect(
    within(table).queryByRole("button", { name: "New skill in Support playbooks" }),
  ).toBeNull();

  fireEvent.click(within(support).getByRole("button", { name: "Collapse Support playbooks" }));
  expect(within(table).queryByRole("link", { name: "refunds" })).toBeNull();
  expect(within(table).getByRole("link", { name: "tilde-channels" })).toBeTruthy();
  fireEvent.click(within(support).getByRole("button", { name: "Expand Support playbooks" }));

  // The filter bar: words match names and descriptions and keep only groups with a match;
  // group: narrows to named groups; the type tabs narrow by where groups come from.
  const search = screen.getByRole("combobox", { name: "Search skills" });
  const apply = (query: string) => {
    fireEvent.change(search, { target: { value: query } });
    fireEvent.submit(search.closest("form")!);
  };
  apply("REFUNDS desc");
  await waitFor(() => expect(names()).toEqual(["Group Support playbooks", "refunds"]));
  fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
  apply('group:"tilde runtime"');
  await waitFor(() => expect(names()[0]).toBe("Group Tilde runtime"));
  expect(names().some((name) => name.startsWith("Group") && name !== "Group Tilde runtime")).toBe(
    false,
  );
  fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
  await waitFor(() => expect(names()).toContain("Group Drafts"));
  fireEvent.click(screen.getByRole("tab", { name: "Git" }));
  expect(names().filter((name) => name.startsWith("Group"))).toEqual(["Group Support playbooks"]);
  fireEvent.click(screen.getByRole("tab", { name: "All" }));
  apply("nothing");
  await screen.findByText("No skills match your search.");
});

it("stacks the agents given each skill and each whole group", async () => {
  const names = ["Ada", "Bea", "Cy", "Dee", "Eve"];
  // The registry arrives over two pages.
  rpc.agents.listAgents.mockImplementation(async ({ pageToken }: { pageToken: string }) => {
    const page = pageToken ? names.slice(3) : names.slice(0, 3);
    return {
      agents: page.map((name) => ({ id: `a-${name}`, name })),
      nextPageToken: pageToken ? "" : "next",
    };
  });
  rpc.skills.listSkillSources.mockResolvedValue({
    sources: [{ ...git, agentIds: ["a-Ada"] }, catalog, drafts],
  });
  rpc.skills.listSkills.mockResolvedValue({
    skills: [
      { ...skill("s1", "refunds", git), agentIds: names.map((name) => `a-${name}`) },
      { ...skill("s2", "escalation", git), agentIds: ["a-Ada"] },
      skill("s3", "tilde-channels", catalog),
    ],
  });
  renderPage();
  const table = await screen.findByRole("table", { name: "Skills" });
  const refunds = (await within(table).findByRole("link", { name: "refunds" })).closest("tr")!;
  // Ada has the whole group, so the skill row names only the others.
  const stack = await within(refunds).findByLabelText("Agents: Bea, Cy, Dee, Eve");
  expect(stack.querySelectorAll("[data-slot='tooltip-trigger']")).toHaveLength(3);
  expect(within(stack).getByText("+1 more")).toBeTruthy();
  const group = within(table).getByRole("row", { name: "Group Support playbooks" });
  expect(within(group).getByLabelText("Agents: Ada")).toBeTruthy();
  expect(within(group).queryByText(/more$/)).toBeNull();
  // The stack opens every agent with the skill, searchable and filterable by how they have it.
  fireEvent.click(stack);
  const dialog = await screen.findByRole("dialog", { name: "Agents with refunds" });
  const listed = () =>
    within(within(dialog).getByRole("list", { name: "Agents" }))
      .queryAllByRole("link")
      .map((link) => link.textContent);
  expect(listed()).toHaveLength(5);
  fireEvent.click(within(dialog).getByRole("tab", { name: "This skill" }));
  expect(listed()).toHaveLength(4);
  fireEvent.change(within(dialog).getByRole("textbox", { name: "Search agents" }), {
    target: { value: "bea" },
  });
  expect(listed()).toEqual(["BeaThis skill"]);
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  const escalation = within(table).getByRole("link", { name: "escalation" }).closest("tr")!;
  expect(within(escalation).getByText("—")).toBeTruthy();
  const channels = within(table).getByRole("link", { name: "tilde-channels" }).closest("tr")!;
  expect(within(channels).getByText("—")).toBeTruthy();
});

it("creates a skill in the group it was started from, or in a new group", async () => {
  rpc.skills.createSkill.mockResolvedValue({ skill: { id: "s-new" } });
  rpc.skills.createEditorSource.mockResolvedValue({ source: { id: "src-team", name: "Team" } });
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "New skill in Drafts" }));
  const group = await screen.findByRole("combobox", { name: "Group" });
  expect((group as HTMLSelectElement).value).toBe("src-drafts");
  fireEvent.change(screen.getByLabelText("Skill name"), { target: { value: "triage" } });
  fireEvent.click(screen.getByRole("button", { name: "Create skill" }));
  await screen.findByText("Skill s-new");
  expect(rpc.skills.createSkill).toHaveBeenCalledWith(
    expect.objectContaining({ sourceId: "src-drafts", name: "triage" }),
  );
  expect(rpc.skills.createEditorSource).not.toHaveBeenCalled();
});

it("creates a new group for a skill from the editor pill", async () => {
  rpc.skills.createSkill.mockRejectedValueOnce(new Error("Name taken"));
  rpc.skills.createSkill.mockResolvedValue({ skill: { id: "s-new" } });
  rpc.skills.createEditorSource.mockResolvedValue({ source: { id: "src-team", name: "Team" } });
  renderPage();
  await screen.findByRole("button", { name: "New skill in Drafts" });
  fireEvent.click(screen.getByRole("button", { name: "Create in editor" }));
  const group = await screen.findByRole("combobox", { name: "Group" });
  await waitFor(() => expect((group as HTMLSelectElement).value).toBe("src-drafts"));
  fireEvent.change(group, { target: { value: "new" } });
  fireEvent.change(screen.getByLabelText("Skill name"), { target: { value: "triage" } });
  fireEvent.click(screen.getByRole("button", { name: "Create skill" }));
  await screen.findByText("Name the new group.");
  fireEvent.change(screen.getByLabelText("Group name"), { target: { value: " Team " } });
  fireEvent.click(screen.getByRole("button", { name: "Create skill" }));
  await screen.findByText("Name taken");
  // The retry reuses the group the failed attempt created.
  expect((screen.getByRole("combobox", { name: "Group" }) as HTMLSelectElement).value).toBe(
    "src-team",
  );
  fireEvent.click(screen.getByRole("button", { name: "Create skill" }));
  await screen.findByText("Skill s-new");
  expect(rpc.skills.createEditorSource).toHaveBeenCalledTimes(1);
  expect(rpc.skills.createEditorSource).toHaveBeenCalledWith({ name: "Team" });
  expect(rpc.skills.createSkill).toHaveBeenLastCalledWith(
    expect.objectContaining({ sourceId: "src-team", name: "triage" }),
  );
});

it("opens the Tilde Catalog as its own page", async () => {
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Tilde Catalog" }));
  await screen.findByText("Catalog page");
});

it("adds a git source on main by default and opens it", async () => {
  rpc.skills.addGitSource.mockResolvedValue({ source: { id: "src-new" } });
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Add from Git" }));
  fireEvent.change(await screen.findByLabelText("Name"), { target: { value: " Playbooks " } });
  fireEvent.change(screen.getByLabelText("GitHub repository URL"), {
    target: { value: "not a url" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Add repository" }));
  await screen.findByText("Enter a https://github.com/<owner>/<repository> URL.");
  expect(rpc.skills.addGitSource).not.toHaveBeenCalled();

  fireEvent.change(screen.getByLabelText("GitHub repository URL"), {
    target: { value: "https://github.com/acme/skills" },
  });
  fireEvent.change(screen.getByLabelText("Path (optional)"), { target: { value: "support" } });
  fireEvent.click(screen.getByRole("button", { name: "Add repository" }));
  await waitFor(() =>
    expect(rpc.skills.addGitSource).toHaveBeenCalledWith({
      name: "Playbooks",
      repositoryUrl: "https://github.com/acme/skills",
      gitRef: "main",
      gitPath: "support",
    }),
  );
  await screen.findByText("Skills list");
});

it("syncs and deletes a group from its row", async () => {
  rpc.skills.syncSkillSource.mockResolvedValue({});
  rpc.skills.deleteSkillSource.mockResolvedValue({});
  renderPage();
  const table = await screen.findByRole("table", { name: "Skills" });
  const support = await within(table).findByRole("row", { name: "Group Support playbooks" });
  fireEvent.click(within(support).getByRole("button", { name: "Support playbooks actions" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Sync now" }));
  await waitFor(() => expect(rpc.skills.syncSkillSource).toHaveBeenCalledWith({ id: "src-git" }));

  fireEvent.click(within(support).getByRole("button", { name: "Support playbooks actions" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Delete group" }));
  const confirm = await screen.findByRole("alertdialog");
  fireEvent.click(within(confirm).getByRole("button", { name: "Delete group" }));
  await waitFor(() => expect(rpc.skills.deleteSkillSource).toHaveBeenCalledWith({ id: "src-git" }));
  // Editor groups have nothing to sync.
  const drafts = within(table).getByRole("row", { name: "Group Drafts" });
  fireEvent.click(within(drafts).getByRole("button", { name: "Drafts actions" }));
  await screen.findByRole("menuitem", { name: "Delete group" });
  expect(screen.queryByRole("menuitem", { name: "Sync now" })).toBeNull();
});

it("deletes an editor skill from its row once confirmed", async () => {
  rpc.skills.deleteSkill.mockResolvedValue({});
  rpc.skills.listSkills.mockResolvedValue({ skills: [skill("s9", "triage", drafts)] });
  renderPage();
  const table = await screen.findByRole("table", { name: "Skills" });
  fireEvent.click(await within(table).findByRole("button", { name: "Delete triage" }));
  expect(rpc.skills.deleteSkill).not.toHaveBeenCalled();
  fireEvent.click(await screen.findByRole("button", { name: "Delete skill" }));
  await waitFor(() => expect(rpc.skills.deleteSkill).toHaveBeenCalledWith({ id: "s9" }));
});
