import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
  useParams,
} from "@tanstack/react-router";
import {
  SkillSchema,
  SkillSourceKind,
  SkillVersionSchema,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { SkillDetail } from "./skill-detail";

const rpc = vi.hoisted(() => ({
  skills: {
    getSkill: vi.fn(),
    getSkillVersion: vi.fn(),
    updateSkill: vi.fn(),
    listSkillSources: vi.fn(),
    createEditorSource: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);
// Monaco needs a real browser; a textarea stands in for it.
vi.mock("./code-editor", () => ({
  default: ({ value, onChange }: { value: string; onChange?: (value: string) => void }) => (
    <textarea aria-label="Code" value={value} onChange={(e) => onChange?.(e.target.value)} />
  ),
}));
// Tiptap needs a real layout engine; a textarea stands in for the markdown editor.
vi.mock("./markdown-editor", () => ({
  MarkdownEditor: ({
    value,
    onChange,
    readOnly,
    label,
  }: {
    value: string;
    onChange?: (value: string) => void;
    readOnly?: boolean;
    label?: string;
  }) =>
    readOnly ? (
      <div data-testid="markdown">{value}</div>
    ) : (
      <textarea aria-label={label} value={value} onChange={(e) => onChange?.(e.target.value)} />
    ),
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

const v1 = create(SkillVersionSchema, { id: "v1", number: 1 });
const v2 = create(SkillVersionSchema, { id: "v2", number: 2 });
const skill = create(SkillSchema, {
  id: "s1",
  name: "refunds",
  sourceId: "src-editor",
  sourceName: "Drafts",
  sourceKind: SkillSourceKind.EDITOR,
  latest: v2,
});
const files = {
  v1: [{ path: "SKILL.md", content: "# refunds\n", sha256: "a", text: true }],
  v2: [
    { path: "SKILL.md", content: "# refunds\nBe kind.\n", sha256: "b", text: true },
    {
      path: "assets/logo.png",
      mediaType: "image/png",
      sizeBytes: 2048n,
      sha256: "c",
      downloadUrl: "https://files.example/logo.png",
    },
  ],
};

function renderDetail() {
  const root = createRootRoute({ component: () => <SkillDetail id="s1" /> });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}

/** A DataTransfer for drag and drop in jsdom, which has none. */
function transfer() {
  const data: Record<string, string> = {};
  return {
    data,
    types: [] as string[],
    items: [],
    files: [],
    setData(type: string, value: string) {
      data[type] = value;
      this.types.push(type);
    },
    getData: (type: string) => data[type] ?? "",
    dropEffect: "",
    effectAllowed: "",
  };
}

it("manages files like VS Code's explorer and saves them as the next version", async () => {
  rpc.skills.getSkill.mockResolvedValue({ skill, versions: [v2, v1] });
  rpc.skills.getSkillVersion.mockImplementation(async ({ id }: { id: "v1" | "v2" }) => ({
    version: create(SkillVersionSchema, { id, files: files[id] }),
  }));
  rpc.skills.updateSkill.mockResolvedValue({ skill });
  renderDetail();
  const tree = await screen.findByRole("tree", { name: "Files" });
  // Only the latest version's files load; no history is shown. Folders come first.
  await within(tree).findByRole("treeitem", { name: "SKILL.md" });
  expect(
    within(tree)
      .getAllByRole("treeitem")
      .map((row) => row.textContent),
  ).toEqual(["assets", "logo.png", "SKILL.md"]);
  expect(rpc.skills.getSkillVersion).toHaveBeenCalledTimes(1);
  fireEvent.click(within(tree).getByRole("treeitem", { name: "logo.png" }));
  expect(screen.getByRole("img", { name: "assets/logo.png" }).getAttribute("src")).toBe(
    "https://files.example/logo.png",
  );
  // Editors land in editing: nothing to save until something changes.
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);

  // Right-click a folder for a new file inside it.
  fireEvent.contextMenu(within(tree).getByRole("treeitem", { name: "assets" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "New File…" }));
  const name = await within(tree).findByRole("textbox", { name: "File name" });
  fireEvent.change(name, { target: { value: "notes.md" } });
  fireEvent.keyDown(name, { key: "Enter" });
  const notes = await within(tree).findByRole("treeitem", { name: "notes.md" });
  expect(notes.getAttribute("title")).toBe("assets/notes.md");

  // Rename it from the menu; a clashing name is refused inline.
  fireEvent.contextMenu(notes);
  fireEvent.click(await screen.findByRole("menuitem", { name: /Rename/ }));
  const renaming = await within(tree).findByRole("textbox", { name: "File name" });
  fireEvent.change(renaming, { target: { value: "logo.png" } });
  expect(within(tree).getByRole("alert").textContent).toMatch(/already exists/);
  fireEvent.change(renaming, { target: { value: "guide.md" } });
  fireEvent.keyDown(renaming, { key: "Enter" });
  const guide = await within(tree).findByRole("treeitem", { name: "guide.md" });

  // Delete removes the focused file; SKILL.md cannot be deleted.
  fireEvent.click(guide);
  fireEvent.keyDown(tree, { key: "Delete" });
  expect(within(tree).queryByRole("treeitem", { name: "guide.md" })).toBeNull();
  fireEvent.click(within(tree).getByRole("treeitem", { name: "SKILL.md" }));
  fireEvent.keyDown(tree, { key: "Delete" });
  expect(within(tree).getByRole("treeitem", { name: "SKILL.md" })).toBeTruthy();

  // Drag a stored asset to the root: it moves without its bytes being sent again.
  const drag = transfer();
  fireEvent.dragStart(within(tree).getByRole("treeitem", { name: "logo.png" }), {
    dataTransfer: drag,
  });
  fireEvent.dragOver(tree, { dataTransfer: drag });
  fireEvent.drop(tree, { dataTransfer: drag });
  await waitFor(() =>
    expect(within(tree).getByRole("treeitem", { name: "logo.png" }).getAttribute("title")).toBe(
      "logo.png",
    ),
  );

  // The emptied folder stays, as in VS Code, so files can be dragged back into it.
  expect(within(tree).getByRole("treeitem", { name: "assets" })).toBeTruthy();
  const upload = new File([new Uint8Array([1, 2, 3])], "table.csv", { type: "text/csv" });
  fireEvent.change(screen.getByLabelText("Files to upload"), { target: { files: [upload] } });
  await within(tree).findByRole("treeitem", { name: "table.csv" });

  fireEvent.click(within(tree).getByRole("treeitem", { name: "SKILL.md" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Contents of SKILL.md" }), {
    target: { value: "# refunds\nBe kind and quick.\n" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(rpc.skills.updateSkill).toHaveBeenCalledWith({
      id: "s1",
      files: [
        { path: "SKILL.md", content: "# refunds\nBe kind and quick.\n" },
        { path: "logo.png", keep: true, keepPath: "assets/logo.png" },
        { path: "table.csv", data: new Uint8Array([1, 2, 3]), mediaType: "text/csv" },
      ],
    }),
  );
});

it("autosaves an edit once, and again only for the next real change", async () => {
  rpc.skills.getSkill.mockResolvedValue({ skill, versions: [v2, v1] });
  rpc.skills.getSkillVersion.mockImplementation(async ({ id }: { id: "v1" | "v2" }) => ({
    version: create(SkillVersionSchema, { id, files: files[id] }),
  }));
  rpc.skills.updateSkill.mockResolvedValue({ skill });
  renderDetail();
  const tree = await screen.findByRole("tree", { name: "Files" });
  fireEvent.click(await within(tree).findByRole("treeitem", { name: "SKILL.md" }));
  const body = screen.getByRole("textbox", { name: "Contents of SKILL.md" });
  fireEvent.change(body, { target: { value: "# refunds\nFirst.\n" } });
  await waitFor(() => expect(rpc.skills.updateSkill).toHaveBeenCalledTimes(1), { timeout: 3000 });
  // Saved and unchanged: nothing more is sent, and the editor is the same element (focus kept).
  await new Promise((resolve) => setTimeout(resolve, 1500));
  expect(rpc.skills.updateSkill).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("textbox", { name: "Contents of SKILL.md" })).toBe(body);
  fireEvent.change(body, { target: { value: "# refunds\nSecond.\n" } });
  await waitFor(() => expect(rpc.skills.updateSkill).toHaveBeenCalledTimes(2), { timeout: 3000 });
  expect(rpc.skills.updateSkill.mock.calls[1]![0].files[0]).toEqual({
    path: "SKILL.md",
    content: "# refunds\nSecond.\n",
  });
});

it("edits the name and description inline, saving SKILL.md's front matter as the next version", async () => {
  const md =
    "---\nname: refunds\ndescription: >-\n  Handle refunds\n  within policy.\n---\nBe kind.\n";
  rpc.skills.getSkill.mockResolvedValue({ skill, versions: [v2, v1] });
  rpc.skills.getSkillVersion.mockResolvedValue({
    version: create(SkillVersionSchema, {
      id: "v2",
      files: [{ path: "SKILL.md", content: md, text: true }, files.v2[1]],
    }),
  });
  rpc.skills.updateSkill.mockResolvedValue({ skill });
  renderDetail();
  expect(await screen.findByRole("heading", { name: "refunds", level: 1 })).toBeTruthy();
  expect(await screen.findByText("Handle refunds within policy.")).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Skill name" }), {
    target: { value: "Refund Policy" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save name" }));
  expect(await screen.findByText(/Skill names use lowercase/)).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", { name: "Skill name" }), {
    target: { value: "refund-policy" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save name" }));
  await waitFor(
    () =>
      expect(rpc.skills.updateSkill).toHaveBeenCalledWith({
        id: "s1",
        files: [
          {
            path: "SKILL.md",
            content:
              "---\nname: refund-policy\ndescription: >-\n  Handle refunds\n  within policy.\n---\nBe kind.\n",
          },
          { path: "assets/logo.png", keep: true },
        ],
      }),
    { timeout: 3000 },
  );

  fireEvent.click(await screen.findByRole("button", { name: "Edit description" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Skill description" }), {
    target: { value: "Refunds: within 30 days" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save description" }));
  await waitFor(
    () =>
      expect(rpc.skills.updateSkill).toHaveBeenLastCalledWith({
        id: "s1",
        files: [
          {
            path: "SKILL.md",
            content:
              '---\nname: refund-policy\ndescription: "Refunds: within 30 days"\n---\nBe kind.\n',
          },
          { path: "assets/logo.png", keep: true },
        ],
      }),
    { timeout: 3000 },
  );
});

it("offers no editing for skills from git sources, even to the source's editors", async () => {
  rpc.skills.getSkill.mockResolvedValue({
    skill: { ...skill, sourceKind: SkillSourceKind.GIT },
    versions: [v2, v1],
  });
  rpc.skills.getSkillVersion.mockImplementation(async ({ id }: { id: "v1" | "v2" }) => ({
    version: create(SkillVersionSchema, { id, files: files[id] }),
  }));
  renderDetail();
  await screen.findByRole("tree", { name: "Files" });
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  expect(screen.queryByRole("button", { name: "New file" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Delete skill" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Move to group" })).toBeNull();
  expect(screen.getByText("Read-only")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Edit name" })).toBeNull();
});

it("never saves one skill's files over the next skill opened", async () => {
  const other = create(SkillSchema, { ...skill, id: "s2", name: "billing", latest: v1 });
  rpc.skills.getSkill.mockImplementation(async ({ id }: { id: string }) =>
    id === "s1"
      ? { skill, versions: [v2] }
      : {
          skill: other,
          versions: [create(SkillVersionSchema, { id: "b1", number: 1 })],
        },
  );
  rpc.skills.getSkillVersion.mockImplementation(async ({ id }: { id: string }) => ({
    version: create(SkillVersionSchema, {
      id,
      files:
        id === "v2"
          ? [
              { path: "SKILL.md", content: "# refunds\n", text: true },
              { path: "first-only.txt", content: "a", text: true },
            ]
          : [
              { path: "SKILL.md", content: "# billing\n", text: true },
              { path: "second-only.txt", content: "b", text: true },
            ],
    }),
  }));
  rpc.skills.updateSkill.mockResolvedValue({ skill: other });
  const root = createRootRoute();
  const route = createRoute({
    getParentRoute: () => root,
    path: "/skills/$skillId",
    component: function Page() {
      const { skillId } = useParams({ strict: false }) as { skillId: string };
      return <SkillDetail id={skillId} />;
    },
  });
  const router = createRouter({
    routeTree: root.addChildren([route]),
    history: createMemoryHistory({ initialEntries: ["/skills/s1"] }),
  });
  render(<RouterProvider router={router} />);
  await screen.findByRole("treeitem", { name: "first-only.txt" });
  // Leave with an edit still waiting for autosave.
  fireEvent.change(await screen.findByRole("textbox", { name: "Contents of SKILL.md" }), {
    target: { value: "# refunds\nUnsaved.\n" },
  });
  await router.navigate({ to: "/skills/$skillId", params: { skillId: "s2" } });
  await screen.findByRole("treeitem", { name: "second-only.txt" });
  expect(screen.queryByRole("treeitem", { name: "first-only.txt" })).toBeNull();
  fireEvent.change(await screen.findByRole("textbox", { name: "Contents of SKILL.md" }), {
    target: { value: "# billing\nEdited.\n" },
  });
  await waitFor(
    () =>
      expect(rpc.skills.updateSkill).toHaveBeenCalledWith({
        id: "s2",
        files: [
          { path: "SKILL.md", content: "# billing\nEdited.\n" },
          { path: "second-only.txt", content: "b" },
        ],
      }),
    { timeout: 3000 },
  );
  // The first skill's pending autosave died with its editor.
  expect(rpc.skills.updateSkill).toHaveBeenCalledTimes(1);
});
