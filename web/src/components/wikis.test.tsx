import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Code, ConnectError } from "@connectrpc/connect";
import { create } from "@bufbuild/protobuf";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { AgentSchema } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  WikiAccess,
  WikiOperation,
  WikiPageRevisionSchema,
  WikiPageSchema,
  WikiPageSummarySchema,
  WikiSchema,
} from "@trytilde/contracts/tilde/management/v1/wikis_pb.js";
import { AgentEditor } from "./agent-editor";
import { renderWiki } from "./wiki-test-router";
import { WikiMarkdown } from "./wiki-markdown";

vi.mock("@/components/agent-iam", () => ({ AgentIam: () => null }));
vi.mock("@/components/agent-sandbox", () => ({ AgentSandbox: () => null }));
vi.mock("./agent-connections", () => ({ AgentConnections: () => null }));
vi.mock("./agent-memory", () => ({
  AgentMemory: () => null,
  applyMemoryDraft: vi.fn(),
  NO_MEMORY: { own: null, links: [] },
}));
// Monaco does not run in jsdom; a textarea stands in for the raw Markdown editor.
vi.mock("./code-editor", () => ({
  default: ({ value, onChange }: { value: string; onChange?: (value: string) => void }) => (
    <textarea aria-label="Markdown" value={value} onChange={(e) => onChange?.(e.target.value)} />
  ),
}));
const rpc = vi.hoisted(() => ({
  agents: { createAgent: vi.fn(), updateAgent: vi.fn(), listAgents: vi.fn() },
  deployments: { getDeployment: vi.fn(), setDeployment: vi.fn() },
  wikis: {
    listWikis: vi.fn(),
    listAgentWikis: vi.fn(),
    createWiki: vi.fn(),
    setAgentWiki: vi.fn(),
    getWiki: vi.fn(),
    listWikiPages: vi.fn(),
    getWikiPage: vi.fn(),
    getWikiPageRevision: vi.fn(),
    updateWikiPage: vi.fn(),
  },
}));
vi.mock("@/client", () => rpc);

function renderAt(component: () => React.ReactNode) {
  const router = createRouter({
    routeTree: createRootRoute({ component }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return render(<RouterProvider router={router} />);
}
const handbook = create(WikiSchema, { id: "wiki-handbook", name: "Handbook" });
const page = create(WikiPageSchema, {
  id: "p1",
  wikiId: "wiki-handbook",
  path: "runbooks/deploys",
  title: "Deploys",
  content:
    "# Deploys\n\nRun task deploy.\n\n| Step | Command |\n| --- | --- |\n| Ship | `task deploy` |",
  revision: 2,
});
beforeEach(() => {
  rpc.agents.listAgents.mockResolvedValue({ agents: [], nextPageToken: "" });
  rpc.wikis.listWikis.mockResolvedValue({ wikis: [handbook] });
  rpc.wikis.getWiki.mockResolvedValue({ wiki: handbook });
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("gives a new agent its own wiki and links shared wikis once it exists", async () => {
  rpc.agents.createAgent.mockResolvedValue({
    agent: create(AgentSchema, { id: "a1", name: "Ada" }),
  });
  const onSaved = vi.fn();
  renderAt(() => <AgentEditor agent={null} onClose={vi.fn()} onSaved={onSaved} />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada" },
  });
  const section = within(screen.getByRole("region", { name: "Wikis" }));
  fireEvent.change(section.getByLabelText("Its own wiki"), { target: { value: "new" } });
  fireEvent.change(await section.findByRole("combobox", { name: "Shared wiki to link" }), {
    target: { value: "wiki-handbook" },
  });
  fireEvent.change(section.getByRole("combobox", { name: "Access to the wiki to link" }), {
    target: { value: String(WikiAccess.READ_WRITE) },
  });
  fireEvent.click(section.getByRole("button", { name: "Link wiki" }));
  await section.findByRole("link", { name: "Handbook" });
  // Nothing is saved before the agent exists.
  expect(rpc.wikis.createWiki).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Create agent" }));
  await waitFor(() => expect(onSaved).toHaveBeenCalledWith(true));
  expect(rpc.wikis.createWiki).toHaveBeenCalledWith(
    expect.objectContaining({ name: "Ada", agentId: "a1" }),
  );
  expect(rpc.wikis.setAgentWiki).toHaveBeenCalledWith({
    agentId: "a1",
    wikiId: "wiki-handbook",
    access: WikiAccess.READ_WRITE,
  });
});

it("resolves a stale save by saving on top of the latest revision, and restores a revision", async () => {
  rpc.wikis.listWikiPages.mockResolvedValue({
    pages: [create(WikiPageSummarySchema, { id: "p1", path: page.path, title: page.title })],
    more: false,
  });
  const revisions = [
    create(WikiPageRevisionSchema, { revision: 2, operation: WikiOperation.UPDATE }),
    create(WikiPageRevisionSchema, { revision: 1, operation: WikiOperation.CREATE }),
  ];
  const latest = { ...page, revision: 3, content: `${page.content}\n\nSomeone else's line.` };
  rpc.wikis.getWikiPage.mockResolvedValueOnce({ page, revisions });
  rpc.wikis.getWikiPage.mockResolvedValue({ page: latest, revisions });
  rpc.wikis.updateWikiPage
    .mockRejectedValueOnce(new ConnectError("The page changed", Code.Aborted))
    .mockResolvedValue({ page: latest });
  renderWiki("wiki-handbook", "pages");

  await screen.findByRole("heading", { name: "Deploys", level: 2 });
  // GitHub tables render.
  within(screen.getByRole("article", { name: "Deploys" })).getByRole("cell", { name: "Ship" });
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const form = within(await screen.findByRole("form", { name: "Edit page" }));
  fireEvent.change(form.getByRole("textbox", { name: "Title" }), {
    target: { value: "Deploying" },
  });
  fireEvent.click(form.getByRole("button", { name: "Save" }));
  // The raw Markdown goes back unchanged, table included.
  await waitFor(() =>
    expect(rpc.wikis.updateWikiPage).toHaveBeenCalledWith(
      expect.objectContaining({ expectedRevision: 2, title: "Deploying", content: page.content }),
    ),
  );
  // Someone saved first: their change shows, and the draft can go on top of it.
  const conflict = within(await form.findByRole("region", { name: "Changed since you opened it" }));
  expect(conflict.getByLabelText("Changes").textContent).toContain("Added: Someone else's line.");
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  fireEvent.click(conflict.getByRole("button", { name: "Overwrite with mine (revision 3)" }));
  await waitFor(() =>
    expect(rpc.wikis.updateWikiPage).toHaveBeenLastCalledWith(
      expect.objectContaining({ expectedRevision: 3, title: "Deploying" }),
    ),
  );
  // A saved draft leaves without asking about it.
  await waitFor(() => expect(screen.queryByRole("form", { name: "Edit page" })).toBeNull());
  expect(confirm).not.toHaveBeenCalled();
  confirm.mockRestore();

  rpc.wikis.getWikiPageRevision.mockResolvedValue({
    revision: create(WikiPageRevisionSchema, { revision: 1, title: "Deploys (draft)" }),
    content: "# Deploys",
  });
  fireEvent.click(await screen.findByRole("button", { name: "History" }));
  fireEvent.click(await screen.findByRole("button", { name: "1" }));
  // Against the current page: restoring would remove what came since.
  const changes = await screen.findByLabelText("Changes");
  expect(changes.textContent).toContain("Removed: Someone else's line.");
  fireEvent.click(screen.getByRole("button", { name: "Restore this revision" }));
  await waitFor(() =>
    expect(rpc.wikis.updateWikiPage).toHaveBeenLastCalledWith({
      id: "p1",
      expectedRevision: 3,
      title: "Deploys (draft)",
      content: "# Deploys",
    }),
  );
});

it("opens wiki links in the app and never loads images from elsewhere", () => {
  renderAt(() => (
    <WikiMarkdown
      wikiId="wiki-handbook"
      label="Links"
      content={
        "[Deploys](runbooks/deploys) [Docs](https://example.com/docs)\n\n![tracker](https://evil.example/pixel.png)"
      }
    />
  ));
  return waitFor(() => {
    const page = screen.getByRole("link", { name: "Deploys" });
    expect(page.getAttribute("href")).toBe("/wikis/wiki-handbook/pages?page=runbooks%2Fdeploys");
    expect(page.getAttribute("target")).toBeNull();
    expect(screen.getByRole("link", { name: "Docs" }).getAttribute("target")).toBe("_blank");
    // The remote image is a link, not an <img> the browser would fetch.
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByRole("link", { name: "[Image: tracker]" }).getAttribute("href")).toBe(
      "https://evil.example/pixel.png",
    );
  });
});

it("holds creating an agent while its own wiki's name is another wiki's", async () => {
  renderAt(() => <AgentEditor agent={null} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Agent name" }), {
    target: { value: "Handbook" },
  });
  const section = within(screen.getByRole("region", { name: "Wikis" }));
  fireEvent.change(section.getByLabelText("Its own wiki"), { target: { value: "new" } });
  await section.findByText(/A wiki named “Handbook” already exists/);
  expect(screen.getByRole("button", { name: "Create agent" }).hasAttribute("disabled")).toBe(true);
  fireEvent.change(section.getByRole("textbox", { name: "Name of its own wiki" }), {
    target: { value: "Ada's notes" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Create agent" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
});

it("offers to create a page a URL names that does not exist", async () => {
  rpc.wikis.listWikiPages.mockResolvedValue({ pages: [], more: false });
  renderWiki("wiki-handbook", "pages", "/wikis/wiki-handbook/pages?page=runbooks%2Fmissing");
  await screen.findByText(/No page at/);
  screen.getByText("runbooks/missing");
  fireEvent.click(screen.getByRole("button", { name: "Create it" }));
  const path = (await screen.findByRole("textbox", { name: "Path" })) as HTMLInputElement;
  expect(path.value).toBe("runbooks/missing");
});
