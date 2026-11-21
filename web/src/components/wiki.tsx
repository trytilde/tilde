import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Controller, useForm } from "react-hook-form";
import { Code, ConnectError } from "@connectrpc/connect";
import { Link, useBlocker, useNavigate, useRouterState } from "@tanstack/react-router";
import { timestampDate, type Timestamp } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import {
  ChevronRightIcon,
  FileTextIcon,
  FolderIcon,
  FolderInputIcon,
  HistoryIcon,
  PencilIcon,
  PlusIcon,
} from "lucide-react";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  WikiOperation,
  type Wiki,
  type WikiGrepLine,
  type WikiPage as Page,
  type WikiPageRevision,
  type WikiPageSummary,
  type WikiSearchResult,
} from "@trytilde/contracts/tilde/management/v1/wikis_pb.js";
import { wikis as client } from "@/client";
import { useAgentRegistry } from "./agent-stack";
import { DashboardNavigation, usePageCrumb } from "./dashboard-breadcrumbs";
import { LineDiff } from "./line-diff";
import { TildeLoader } from "./loading-screen";
import { MarkdownEditor, saveKey } from "./markdown-editor";
import { headingsOf, WikiMarkdown } from "./wiki-markdown";
import { LinkedAgentsTable, OwnerLink } from "./linked-agents";
import { RemoveButton } from "./remove-button";
import { ToolbarSelect } from "./toolbar-select";
import { CatalogSearchField } from "./tool-catalog";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export const WIKI_TABS = ["pages", "settings"] as const;
export type WikiTab = (typeof WIKI_TABS)[number];

/** One wiki: its pages, and its settings and linked agents. */
export function WikiPage({
  wikiId,
  tab,
  onTabChange,
  page,
  onPageChange,
}: {
  wikiId: string;
  tab: WikiTab;
  onTabChange: (tab: WikiTab) => void;
  /** The open page's path, from the URL; without one the first page opens. */
  page: string | undefined;
  onPageChange: (path?: string) => void;
}) {
  const [wiki, setWiki] = useState<Wiki>();
  const [error, setError] = useState("");
  // The header row's right side, where the pages tab puts its search and New page.
  const [toolbar, setToolbar] = useState<HTMLDivElement | null>(null);
  const registry = useAgentRegistry();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  usePageCrumb(pathname, wiki?.name);
  const reload = useCallback(
    (signal?: AbortSignal) =>
      client
        .getWiki({ id: wikiId }, { signal })
        .then(({ wiki }) => {
          if (!wiki) throw new Error("Wiki not found.");
          setWiki(wiki);
        })
        .catch((e) => !signal?.aborted && setError(message(e))),
    [wikiId],
  );
  useEffect(() => {
    const abort = new AbortController();
    void reload(abort.signal);
    return () => abort.abort();
  }, [reload]);
  if (!wiki)
    return error ? (
      <p role="alert" className="p-6 text-sm text-destructive">
        {error}
      </p>
    ) : (
      <TildeLoader className="flex-1" />
    );
  return (
    <Tabs
      className="min-h-0 flex-1 gap-0"
      value={tab}
      onValueChange={(value) => onTabChange(value as WikiTab)}
    >
      <DashboardNavigation>
        <div className="flex min-w-max items-center gap-3">
          <TabsList aria-label="Wiki">
            <TabsTrigger value="pages">Pages</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>
          <div
            ref={setToolbar}
            className="trace-controls ml-auto flex items-center justify-end gap-1.5 empty:hidden"
          />
        </div>
      </DashboardNavigation>
      <TabsContent value="pages" className="flex min-h-0 flex-1 flex-col">
        {tab === "pages" && (
          <WikiPages
            wiki={wiki}
            registry={registry}
            toolbar={toolbar}
            path={page}
            onPathChange={onPageChange}
          />
        )}
      </TabsContent>
      <TabsContent
        value="settings"
        className="mx-auto grid w-full max-w-5xl gap-6 px-4 py-6 lg:px-8"
      >
        <WikiSettings wiki={wiki} registry={registry} onSaved={setWiki} />
      </TabsContent>
    </Tabs>
  );
}

const ago = (t?: Timestamp) =>
  t ? formatDistanceToNow(timestampDate(t), { addSuffix: true }) : "";
const OPERATIONS: Record<WikiOperation, string> = {
  [WikiOperation.UNSPECIFIED]: "Changed",
  [WikiOperation.CREATE]: "Created",
  [WikiOperation.UPDATE]: "Edited",
  [WikiOperation.MOVE]: "Moved",
  [WikiOperation.DELETE]: "Deleted",
};
type Row = { key: string; path: string; depth: number; label: string; page?: WikiPageSummary };
/** Pages in path order, indented under the folders their paths name. */
function outline(pages: WikiPageSummary[]) {
  const rows: Row[] = [];
  const shown = new Set<string>();
  for (const page of pages) {
    const segments = page.path.split("/");
    for (let i = 1; i < segments.length; i++) {
      const folder = segments.slice(0, i).join("/");
      if (shown.has(folder) || pages.some((p) => p.path === folder)) continue;
      shown.add(folder);
      rows.push({ key: `folder:${folder}`, path: folder, depth: i - 1, label: segments[i - 1] });
    }
    rows.push({
      key: page.id,
      path: page.path,
      depth: segments.length - 1,
      label: page.title,
      page,
    });
  }
  return rows;
}
const ancestors = (path: string) =>
  path
    .split("/")
    .slice(0, -1)
    .map((_, i, all) => all.slice(0, i + 1).join("/"));

/** A search excerpt, whose matched words the server brackets with U+E000 and U+E001. */
function Excerpt({ text }: { text: string }) {
  return text.split(/[]/).map((part, index) =>
    index % 2 ? (
      <mark
        key={index}
        className="rounded-sm bg-yellow-200/70 px-0.5 text-foreground dark:bg-yellow-500/30"
      >
        {part}
      </mark>
    ) : (
      part
    ),
  );
}

type Finding =
  | { kind: "search"; results: WikiSearchResult[] }
  | {
      kind: "grep";
      lines: WikiGrepLine[];
      more: boolean;
    };
type Mode = "search" | "grep" | "regex";
const MODES: { value: Mode; label: string }[] = [
  { value: "search", label: "Words" },
  { value: "grep", label: "Exact text" },
  { value: "regex", label: "Regex" },
];

/**
 * A wiki's pages: the page tree (or what a search found), the open page as a document with its
 * outline and the pages linking to it, and editing and creating pages in place with the rich
 * editor. Search and New page sit in the page header's tab row.
 */
function WikiPages({
  wiki,
  registry,
  toolbar,
  path,
  onPathChange,
}: {
  wiki: Wiki;
  registry: Map<string, Agent>;
  toolbar: HTMLDivElement | null;
  path: string | undefined;
  onPathChange: (path?: string) => void;
}) {
  let editable = true;
  const [pages, setPages] = useState<WikiPageSummary[]>([]);
  const [more, setMore] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [page, setPage] = useState<Page>();
  const [revisions, setRevisions] = useState<WikiPageRevision[]>([]);
  const [editing, setEditing] = useState(false);
  // The path a new page starts with, while creating one.
  const [creating, setCreating] = useState<string>();
  const [moving, setMoving] = useState(false);
  const [history, setHistory] = useState(false);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<Mode>("search");
  const [finding, setFinding] = useState<Finding>();
  const loadPages = useCallback(
    (signal?: AbortSignal) =>
      client
        .listWikiPages({ wikiId: wiki.id }, { signal })
        .then(({ pages, more }) => {
          setPages(pages);
          setMore(more);
          setLoaded(true);
        })
        .catch((e) => !signal?.aborted && setError(message(e))),
    [wiki.id],
  );
  useEffect(() => {
    const abort = new AbortController();
    void loadPages(abort.signal);
    return () => abort.abort();
  }, [loadPages]);
  const selected = path ? pages.find((p) => p.path === path)?.id : pages[0]?.id;
  // An unsaved edit is guarded by the editor's navigation blocker.
  const select = (next?: string) => {
    setCreating(undefined);
    onPathChange(next);
  };
  const loadPage = useCallback(async (id: string, signal?: AbortSignal) => {
    const found = await client.getWikiPage({ id }, { signal });
    setPage(found.page);
    setRevisions(found.revisions);
  }, []);
  useEffect(() => {
    if (!selected) {
      setPage(undefined);
      return;
    }
    const abort = new AbortController();
    setEditing(false);
    loadPage(selected, abort.signal).catch((e) => !abort.signal.aborted && setError(message(e)));
    return () => abort.abort();
  }, [selected, loadPage]);
  async function find() {
    const text = query.trim();
    if (!text) return setFinding(undefined);
    setError("");
    try {
      if (mode === "search") {
        const { results } = await client.searchWikiPages({ wikiId: wiki.id, query: text });
        setFinding({ kind: "search", results });
      } else {
        const { lines, more } = await client.grepWikiPages({
          wikiId: wiki.id,
          pattern: text,
          regex: mode === "regex",
        });
        setFinding({ kind: "grep", lines, more });
      }
    } catch (e) {
      setError(message(e));
    }
  }
  async function changed(next?: Page) {
    await loadPages();
    if (next) {
      select(next.path);
      await loadPage(next.id);
    }
  }
  const tools = (
    <>
      <form
        className="flex items-center gap-1.5"
        aria-label="Find in this wiki"
        onSubmit={(event) => {
          event.preventDefault();
          void find();
        }}
      >
        <div className="w-56">
          <CatalogSearchField
            label={mode === "search" ? "Search pages" : "Find lines"}
            value={query}
            onChange={(value) => {
              setQuery(value);
              if (!value.trim()) setFinding(undefined);
            }}
          />
        </div>
        <ToolbarSelect label="How to find" value={mode} options={MODES} onChange={setMode} />
      </form>
      <Button
        size="sm"
        className="h-[26px] gap-1.5 px-2.5 text-[11px]"
        onClick={() => setCreating(page ? folderOf(page.path) : "")}
      >
        <PlusIcon className="size-3.5" />
        New page
      </Button>
    </>
  );
  const reading = page && !editing && creating === undefined;
  return (
    <div
      className={`grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)] lg:grid-cols-[16rem_minmax(0,1fr)] ${reading ? "xl:grid-cols-[16rem_minmax(0,1fr)_15rem]" : ""}`}
    >
      {toolbar ? createPortal(tools, toolbar) : null}
      <aside className="trace-controls flex min-h-0 flex-col overflow-y-auto border-b bg-muted/20 p-2 lg:border-r lg:border-b-0">
        {finding ? (
          <Found finding={finding} onSelect={select} onClear={() => setFinding(undefined)} />
        ) : (
          <PageTree
            pages={pages}
            more={more}
            selected={selected}
            open={page?.path}
            onSelect={select}
          />
        )}
      </aside>
      <section className="flex min-h-0 flex-col overflow-auto" aria-label="Page">
        {error && (
          <p role="alert" className="px-6 pt-4 text-sm text-destructive">
            {error}
          </p>
        )}
        {creating !== undefined ? (
          <PageCreator
            wiki={wiki}
            path={creating}
            folders={[...new Set(pages.flatMap((p) => ancestors(p.path)))]}
            onCancel={() => setCreating(undefined)}
            onCreated={(next) => void changed(next)}
          />
        ) : page ? (
          editing ? (
            <PageEditor
              page={page}
              onCancel={() => setEditing(false)}
              onSaved={async (next) => {
                setEditing(false);
                await changed(next);
              }}
            />
          ) : (
            <>
              <div className="sticky top-0 z-10 flex h-10 shrink-0 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur lg:px-6">
                <PathCrumbs path={page.path} />
                <span className="ml-auto hidden shrink-0 text-[11px] text-muted-foreground md:inline">
                  Revision {page.revision} · edited {ago(page.updatedAt)}
                  {revisions[0] &&
                    ` by ${revisions[0].agentId ? (registry.get(revisions[0].agentId)?.name ?? "an agent") : "a person"}`}
                </span>
                <div className="ml-auto flex shrink-0 items-center gap-1 md:ml-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 gap-1.5"
                    onClick={() => setHistory(true)}
                  >
                    <HistoryIcon />
                    History
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Move"
                    title="Move to another path"
                    onClick={() => setMoving(true)}
                  >
                    <FolderInputIcon />
                  </Button>
                  <RemoveButton
                    label={`Delete ${page.title}`}
                    title={`Delete ${page.title}?`}
                    description="The page is removed for people and agents. Its past revisions stay in the database, but no page shows them."
                    confirmLabel="Delete"
                    onConfirm={() =>
                      client
                        .deleteWikiPage({ id: page.id, expectedRevision: page.revision })
                        .then(async () => {
                          select(undefined);
                          await loadPages();
                        })
                        .catch((e) => setError(message(e)))
                    }
                  />
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 gap-1.5"
                    onClick={() => setEditing(true)}
                  >
                    <PencilIcon />
                    Edit
                  </Button>
                </div>
              </div>
              <div className="mx-auto w-full max-w-3xl px-6 py-8">
                {/* Most pages open with their title as a heading; it is not shown twice. */}
                {untitled(page.content, page.title) === undefined && (
                  <h2 className="mb-6 text-2xl font-semibold tracking-tight">{page.title}</h2>
                )}
                <WikiMarkdown content={page.content} label={page.title} wikiId={wiki.id} />
              </div>
            </>
          )
        ) : (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center text-sm text-muted-foreground">
            <FileTextIcon className="size-8 opacity-40" />
            {path && loaded && !selected ? (
              <>
                <p>
                  No page at <code className="font-mono">{path}</code>.
                </p>
                {editable && (
                  <Button variant="outline" size="sm" onClick={() => setCreating(path)}>
                    Create it
                  </Button>
                )}
              </>
            ) : pages.length ? (
              <p>Choose a page.</p>
            ) : (
              <>
                <p>This wiki has no pages yet.</p>
                {editable && loaded && (
                  <Button variant="outline" size="sm" onClick={() => setCreating("")}>
                    Write the first page
                  </Button>
                )}
              </>
            )}
          </div>
        )}
      </section>
      {reading && <PageRail wiki={wiki} page={page} onSelect={select} />}
      {page && (
        <MovePageDialog
          page={page}
          open={moving}
          onClose={() => setMoving(false)}
          onMoved={(next) => void changed(next)}
        />
      )}
      {page && (
        <HistoryDialog
          page={page}
          revisions={revisions}
          registry={registry}
          editable={editable}
          open={history}
          onClose={() => setHistory(false)}
          onRestored={(next) => void changed(next)}
        />
      )}
    </div>
  );
}

/**
 * A page's Markdown without its leading `# Title` heading, when that heading repeats the title;
 * undefined otherwise. Reading and editing show the title once.
 */
function untitled(content: string, title: string) {
  const heading = /^\s*#[ \t]+[^\n]*(\n|$)/.exec(content);
  if (!heading || headingsOf(heading[0].trim())[0]?.text !== title) return undefined;
  return content.slice(heading[0].length).replace(/^\n+/, "");
}

/** The folder a page sits in, with a trailing slash, for a sibling's starting path. */
const folderOf = (path: string) =>
  path.includes("/") ? `${path.slice(0, path.lastIndexOf("/"))}/` : "";

function PathCrumbs({ path }: { path: string }) {
  const segments = path.split("/");
  return (
    <nav
      aria-label="Page path"
      className="flex min-w-0 items-center gap-1 truncate font-mono text-[11px] text-muted-foreground"
    >
      {segments.map((segment, i) => (
        <span key={i} className="flex min-w-0 items-center gap-1">
          {i > 0 && <ChevronRightIcon aria-hidden className="size-3 shrink-0" />}
          <span className={i === segments.length - 1 ? "truncate text-foreground" : "truncate"}>
            {segment}
          </span>
        </span>
      ))}
    </nav>
  );
}

/** Pages under collapsible folders. */
function PageTree({
  pages,
  more,
  selected,
  open,
  onSelect,
}: {
  pages: WikiPageSummary[];
  more: boolean;
  selected?: string;
  /** The open page's path; its folders stay expanded. */
  open?: string;
  onSelect: (path: string) => void;
}) {
  const rows = useMemo(() => outline(pages), [pages]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const toggle = (folder: string) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (!next.delete(folder)) next.add(folder);
      return next;
    });
  const hasChildren = useMemo(() => new Set(pages.flatMap((p) => ancestors(p.path))), [pages]);
  const visible = rows.filter(
    (row) =>
      !ancestors(row.path).some(
        (folder) => collapsed.has(folder) && !(open && open.startsWith(`${folder}/`)),
      ),
  );
  return (
    <nav className="grid gap-px" aria-label="Pages">
      <p className="px-2 pt-1 pb-1.5 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
        Pages
      </p>
      {visible.length ? (
        visible.map((row) => {
          const expandable = hasChildren.has(row.path);
          const expanded = expandable && !collapsed.has(row.path);
          return (
            <div
              key={row.key}
              className="flex h-7 items-center rounded-md pr-2 hover:bg-muted has-aria-[current=page]:bg-muted has-aria-[current=page]:font-semibold"
              style={{ paddingLeft: `${row.depth * 0.75}rem` }}
            >
              {expandable ? (
                <button
                  type="button"
                  aria-label={expanded ? `Collapse ${row.label}` : `Expand ${row.label}`}
                  aria-expanded={expanded}
                  className="flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-foreground"
                  onClick={() => toggle(row.path)}
                >
                  <ChevronRightIcon
                    className={`size-3 transition-transform ${expanded ? "rotate-90" : ""}`}
                  />
                </button>
              ) : (
                <span className="w-5 shrink-0" />
              )}
              {row.page ? (
                <button
                  type="button"
                  title={row.path}
                  aria-current={row.page.id === selected ? "page" : undefined}
                  className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
                  onClick={() => onSelect(row.page!.path)}
                >
                  <FileTextIcon className="size-3.5 shrink-0 text-muted-foreground" />
                  <span className="truncate">{row.label}</span>
                </button>
              ) : (
                <button
                  type="button"
                  className="flex min-w-0 flex-1 items-center gap-1.5 text-left text-muted-foreground"
                  onClick={() => toggle(row.path)}
                >
                  <FolderIcon className="size-3.5 shrink-0" />
                  <span className="truncate">{row.label}</span>
                </button>
              )}
            </div>
          );
        })
      ) : (
        <p className="px-2 text-muted-foreground">No pages yet.</p>
      )}
      {more && (
        <p className="px-2 pt-2 text-muted-foreground">
          Only the first pages are listed; search to find the rest.
        </p>
      )}
    </nav>
  );
}

function Found({
  finding,
  onSelect,
  onClear,
}: {
  finding: Finding;
  onSelect: (path: string) => void;
  onClear: () => void;
}) {
  const count = finding.kind === "search" ? finding.results.length : finding.lines.length;
  return (
    <div className="grid gap-1">
      <div className="flex items-center px-2 pt-1 pb-1.5">
        <p className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
          {count} {finding.kind === "search" ? "pages" : "lines"} found
        </p>
        <button
          type="button"
          className="ml-auto text-muted-foreground underline-offset-2 hover:underline"
          onClick={onClear}
        >
          Back to pages
        </button>
      </div>
      <ul className="grid gap-px" aria-label="Found">
        {finding.kind === "search" ? (
          finding.results.length ? (
            finding.results.map((r) => (
              <li key={r.page?.id}>
                <button
                  type="button"
                  className="w-full rounded-md px-2 py-1.5 text-left hover:bg-muted"
                  onClick={() => r.page && onSelect(r.page.path)}
                >
                  <span className="block font-semibold">{r.page?.title}</span>
                  <span className="block text-muted-foreground">
                    <Excerpt text={r.excerpt} />
                  </span>
                </button>
              </li>
            ))
          ) : (
            <li className="px-2 text-muted-foreground">No pages match.</li>
          )
        ) : finding.lines.length ? (
          <>
            {finding.lines.map((l) => (
              <li key={`${l.pagePath}:${l.lineNumber}`}>
                <button
                  type="button"
                  className="w-full rounded-md px-2 py-1 text-left hover:bg-muted"
                  onClick={() => onSelect(l.pagePath)}
                >
                  <span className="text-muted-foreground">
                    {l.pagePath}:{l.lineNumber}
                  </span>{" "}
                  {l.line}
                  {l.truncated && <span className="text-muted-foreground">…</span>}
                </button>
              </li>
            ))}
            {finding.more && <li className="px-2 text-muted-foreground">More lines matched.</li>}
          </>
        ) : (
          <li className="px-2 text-muted-foreground">No lines match.</li>
        )}
      </ul>
    </div>
  );
}

/**
 * Beside an open page: its outline, and the pages whose Markdown links to it (found by grepping
 * for links that name its path).
 */
function PageRail({
  wiki,
  page,
  onSelect,
}: {
  wiki: Wiki;
  page: Page;
  onSelect: (path: string) => void;
}) {
  const headings = useMemo(() => headingsOf(page.content), [page.content]);
  const [linking, setLinking] = useState<string[]>([]);
  useEffect(() => {
    const abort = new AbortController();
    // A link names the page by its path from the root: `/path`, `./path` or `path`, maybe `.md`.
    const escaped = page.path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const target = new RegExp(`\\]\\((\\.?/)?${escaped}(\\.md)?(#[^)]*)?\\)`);
    Promise.resolve()
      .then(() =>
        client.grepWikiPages(
          { wikiId: wiki.id, pattern: page.path, regex: false },
          { signal: abort.signal },
        ),
      )
      .then(({ lines }) =>
        setLinking([
          ...new Set(
            lines
              .filter((l) => l.pagePath !== page.path && target.test(l.line))
              .map((l) => l.pagePath),
          ),
        ]),
      )
      .catch(() => setLinking([]));
    return () => abort.abort();
  }, [wiki.id, page.path]);
  return (
    <aside
      aria-label="About this page"
      className="trace-controls hidden min-h-0 overflow-y-auto border-l p-4 xl:block"
    >
      {headings.length > 1 && (
        <nav aria-label="On this page" className="mb-6">
          <p className="mb-2 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
            On this page
          </p>
          <ul className="grid gap-1">
            {headings.map((h, index) => (
              <li key={index} style={{ paddingLeft: `${(h.level - 1) * 0.6}rem` }}>
                <button
                  type="button"
                  className="text-left text-muted-foreground hover:text-foreground"
                  onClick={() =>
                    // The page's headings render in the order the outline lists them.
                    document
                      .querySelectorAll("article.wiki-prose :is(h1, h2, h3)")
                      [index]?.scrollIntoView({ behavior: "smooth", block: "start" })
                  }
                >
                  {h.text}
                </button>
              </li>
            ))}
          </ul>
        </nav>
      )}
      <p className="mb-2 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
        Linked from
      </p>
      {linking.length ? (
        <ul className="grid gap-1" aria-label="Linked from">
          {linking.map((p) => (
            <li key={p}>
              <button
                type="button"
                className="flex items-center gap-1.5 text-left hover:underline"
                onClick={() => onSelect(p)}
              >
                <FileTextIcon className="size-3 shrink-0 text-muted-foreground" />
                {p}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground">
          No pages link here. Link one with <code>[text](/{page.path})</code>.
        </p>
      )}
    </aside>
  );
}

/** The bar above a page being written: what it is and its actions. */
function EditorBar({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="sticky top-0 z-20 flex h-10 shrink-0 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur lg:px-6">
      <div className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">{title}</div>
      {children}
    </div>
  );
}

/** A big borderless title field, as the page will show it. */
function TitleField({
  invalid,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & { invalid: boolean }) {
  return (
    <input
      aria-label="Title"
      aria-invalid={invalid}
      placeholder="Untitled"
      className="w-full bg-transparent text-2xl font-semibold tracking-tight outline-none placeholder:text-muted-foreground/50"
      {...props}
    />
  );
}

/** Asks before leaving with unsaved changes; `leaving` is set once saved or discarded. */
function useUnsavedGuard(dirty: boolean) {
  const leaving = useRef(false);
  useBlocker({
    shouldBlockFn: () =>
      !leaving.current && dirty && !window.confirm("Discard your unsaved changes to this page?"),
    enableBeforeUnload: () => !leaving.current && dirty,
  });
  return leaving;
}

/**
 * Editing a page's title and Markdown, in the rich editor, at the revision it was opened at.
 * When someone else saved first, it shows what changed since and lets the writer save on top of
 * the latest revision or discard their draft.
 */
function PageEditor({
  page,
  onCancel,
  onSaved,
}: {
  page: Page;
  onCancel: () => void;
  onSaved: (page: Page) => void;
}) {
  // A page that opens with its title as a heading edits without it, and saves it back on top
  // under the (possibly new) title.
  const body = untitled(page.content, page.title);
  const form = useForm({ defaultValues: { title: page.title, content: body ?? page.content } });
  const [error, setError] = useState("");
  // The revision the draft saves on top of: the one it opened, or the latest after a conflict.
  const [base, setBase] = useState(page.revision);
  const [latest, setLatest] = useState<Page>();
  const dirty = form.formState.isDirty;
  // Leaving with unsaved changes asks first: other pages, tabs, links, history and the browser.
  const leaving = useUnsavedGuard(dirty);
  // Saves at `revision`: the one the draft opened at, or the latest once the writer chose to.
  const save = (revision: number) =>
    form.handleSubmit(async (values) => {
      setError("");
      try {
        const { page: next } = await client.updateWikiPage({
          id: page.id,
          expectedRevision: revision,
          title: values.title.trim(),
          content:
            body === undefined ? values.content : `# ${values.title.trim()}\n\n${values.content}`,
        });
        if (next) {
          leaving.current = true;
          onSaved(next);
        }
      } catch (e) {
        if (ConnectError.from(e).code !== Code.Aborted) return setError(message(e));
        const { page: newer } = await client.getWikiPage({ id: page.id });
        setLatest(newer);
      }
    });
  const submit = () => {
    if (!latest && !form.formState.isSubmitting) void save(base)();
  };
  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      aria-label="Edit page"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <EditorBar
        title={
          <>
            Editing <span className="font-mono text-foreground">{page.path}</span> · revision {base}
          </>
        }
      >
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7"
          disabled={form.formState.isSubmitting}
          onClick={() => {
            if (dirty && !window.confirm("Discard your unsaved changes to this page?")) return;
            leaving.current = true;
            onCancel();
          }}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          size="sm"
          className="h-7"
          title={`Save (${saveKey()})`}
          disabled={form.formState.isSubmitting || !!latest}
        >
          {form.formState.isSubmitting ? "Saving…" : "Save"}
        </Button>
      </EditorBar>
      <div className="mx-auto grid w-full max-w-3xl gap-4 px-6 py-6">
        {latest && (
          <section
            aria-label="Changed since you opened it"
            className="grid gap-3 rounded-lg border border-amber-500/50 bg-amber-500/5 p-3"
          >
            <p className="text-sm">
              Someone saved revision {latest.revision} while you were editing. Their changes since
              you opened the page:
            </p>
            <LineDiff before={page.content} after={latest.content} />
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                disabled={form.formState.isSubmitting}
                onClick={() => {
                  setBase(latest.revision);
                  setLatest(undefined);
                  void save(latest.revision)();
                }}
              >
                Overwrite with mine (revision {latest.revision})
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={() => {
                  leaving.current = true;
                  onSaved(latest);
                }}
              >
                Discard and open latest
              </Button>
            </div>
          </section>
        )}
        <TitleField
          invalid={!!form.formState.errors.title}
          {...form.register("title", { validate: (value) => !!value.trim() })}
        />
        <Controller
          control={form.control}
          name="content"
          render={({ field }) => (
            <MarkdownEditor
              label="Page content"
              value={field.value}
              onChange={field.onChange}
              onSave={submit}
              autoFocus
              contentClassName="wiki-prose"
              minHeight="min-h-[55vh]"
            />
          )}
        />
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
      </div>
    </form>
  );
}

/** Writing a new page in place: its path, title and Markdown. */
function PageCreator({
  wiki,
  path,
  folders,
  onCancel,
  onCreated,
}: {
  wiki: Wiki;
  /** Filled in when creating a page a link or URL named, or beside the open page. */
  path: string;
  /** Existing folders, offered while typing the path. */
  folders: string[];
  onCancel: () => void;
  onCreated: (page: Page) => void;
}) {
  const form = useForm({ defaultValues: { path, title: "", content: "" } });
  const [error, setError] = useState("");
  const leaving = useUnsavedGuard(form.formState.isDirty);
  const submit = form.handleSubmit(async (values) => {
    setError("");
    try {
      const { page } = await client.createWikiPage({
        wikiId: wiki.id,
        path: values.path.trim(),
        title: values.title.trim(),
        content: values.content,
      });
      if (page) {
        leaving.current = true;
        onCreated(page);
      }
    } catch (e) {
      setError(message(e));
    }
  });
  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      aria-label="New page"
      onSubmit={(event) => void submit(event)}
    >
      <EditorBar title="New page">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7"
          onClick={() => {
            if (
              form.formState.isDirty &&
              !window.confirm("Discard your unsaved changes to this page?")
            )
              return;
            leaving.current = true;
            onCancel();
          }}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          size="sm"
          className="h-7"
          title={`Create (${saveKey()})`}
          disabled={form.formState.isSubmitting}
        >
          {form.formState.isSubmitting ? "Creating…" : "Create page"}
        </Button>
      </EditorBar>
      <div className="mx-auto grid w-full max-w-3xl gap-4 px-6 py-6">
        <div className="flex items-center gap-2">
          <Label htmlFor="new-page-path" className="shrink-0 text-xs text-muted-foreground">
            Path
          </Label>
          <Input
            id="new-page-path"
            list="new-page-folders"
            autoFocus
            className="h-7 font-mono text-xs"
            placeholder="runbooks/deploys"
            aria-invalid={!!form.formState.errors.path}
            title="Slash-separated segments of letters, digits, '-', '_' and '.'"
            {...form.register("path", { validate: (value) => !!value.trim() })}
          />
          <datalist id="new-page-folders">
            {folders.map((f) => (
              <option key={f} value={`${f}/`} />
            ))}
          </datalist>
        </div>
        <TitleField
          invalid={!!form.formState.errors.title}
          {...form.register("title", { validate: (value) => !!value.trim() })}
        />
        <Controller
          control={form.control}
          name="content"
          render={({ field }) => (
            <MarkdownEditor
              label="Page content"
              value={field.value}
              onChange={field.onChange}
              onSave={() => void submit()}
              contentClassName="wiki-prose"
              minHeight="min-h-[50vh]"
            />
          )}
        />
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
      </div>
    </form>
  );
}

function MovePageDialog({
  page,
  open,
  onClose,
  onMoved,
}: {
  page: Page;
  open: boolean;
  onClose: () => void;
  onMoved: (page: Page) => void;
}) {
  const form = useForm({ values: { path: page.path } });
  const [error, setError] = useState("");
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Move {page.title}</DialogTitle>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { page: next } = await client.moveWikiPage({
                id: page.id,
                expectedRevision: page.revision,
                path: values.path.trim(),
              });
              if (next) onMoved(next);
              onClose();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="move-page-path">New path</Label>
            <Input id="move-page-path" {...form.register("path")} />
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              Move
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** A page's revisions, newest first; one opens as a diff against the revision before it. */
function HistoryDialog({
  page,
  revisions,
  registry,
  editable,
  open,
  onClose,
  onRestored,
}: {
  page: Page;
  revisions: WikiPageRevision[];
  registry: Map<string, Agent>;
  editable: boolean;
  open: boolean;
  onClose: () => void;
  onRestored: (page: Page) => void;
}) {
  // A chosen revision, against the page as it is now.
  const [diff, setDiff] = useState<{ revision: number; title: string; content: string }>();
  const [error, setError] = useState("");
  const [restoring, setRestoring] = useState(false);
  /** Writes the revision's title and content as a new revision on top of the current one. */
  async function restore(title: string, content: string) {
    setRestoring(true);
    setError("");
    try {
      const { page: next } = await client.updateWikiPage({
        id: page.id,
        expectedRevision: page.revision,
        title,
        content,
      });
      if (next) onRestored(next);
      setDiff(undefined);
      onClose();
    } catch (e) {
      setError(message(e));
    } finally {
      setRestoring(false);
    }
  }
  async function compare(revision: number) {
    setError("");
    try {
      const found = await client.getWikiPageRevision({ pageId: page.id, revision });
      setDiff({ revision, title: found.revision?.title ?? page.title, content: found.content });
    } catch (e) {
      setError(message(e));
    }
  }
  const author = (r: WikiPageRevision) =>
    r.agentId ? (registry.get(r.agentId)?.name ?? "An agent") : "A person";
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setDiff(undefined);
          onClose();
        }
      }}
    >
      <DialogContent className="sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>History of {page.title}</DialogTitle>
        </DialogHeader>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        {diff ? (
          <div className="grid gap-3">
            <div className="flex items-center justify-between">
              <p className="text-sm">
                What restoring revision {diff.revision} would change in the current page
                {diff.title !== page.title && <>, including its title (“{diff.title}”)</>}
              </p>
              <div className="flex gap-2">
                {editable && (diff.content !== page.content || diff.title !== page.title) && (
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={restoring}
                    onClick={() => void restore(diff.title, diff.content)}
                  >
                    {restoring ? "Restoring…" : "Restore this revision"}
                  </Button>
                )}
                <Button variant="outline" size="sm" onClick={() => setDiff(undefined)}>
                  All revisions
                </Button>
              </div>
            </div>
            {diff.content === page.content ? (
              <p className="text-sm text-muted-foreground">Its content is the current page's.</p>
            ) : (
              <LineDiff before={page.content} after={diff.content} />
            )}
          </div>
        ) : (
          <div className="overflow-hidden rounded-lg border">
            <Table aria-label="Revisions">
              <TableHeader className="bg-muted/50">
                <TableRow>
                  <TableHead>Revision</TableHead>
                  <TableHead>Change</TableHead>
                  <TableHead>By</TableHead>
                  <TableHead>When</TableHead>
                  <TableHead>Source</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {revisions.map((r) => (
                  <TableRow key={r.revision}>
                    <TableCell>
                      <button
                        type="button"
                        className="underline"
                        onClick={() => void compare(r.revision)}
                      >
                        {r.revision}
                      </button>
                    </TableCell>
                    <TableCell>
                      {OPERATIONS[r.operation]}
                      {r.operation === WikiOperation.MOVE && (
                        <span className="font-mono text-xs text-muted-foreground">
                          {" "}
                          to {r.path}
                        </span>
                      )}
                    </TableCell>
                    <TableCell>{author(r)}</TableCell>
                    <TableCell>{ago(r.at)}</TableCell>
                    <TableCell>
                      {r.agentId && r.threadId && (
                        <Link
                          to="/registry/agent/$agentId/sessions"
                          params={{ agentId: r.agentId }}
                          search={{ session: r.threadId }}
                          className="underline"
                        >
                          Conversation
                        </Link>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function WikiSettings({
  wiki,
  registry,
  onSaved,
}: {
  wiki: Wiki;
  registry: Map<string, Agent>;
  onSaved: (wiki: Wiki) => void;
}) {
  const navigate = useNavigate();
  const form = useForm({ defaultValues: { name: wiki.name, description: wiki.description } });
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [busy, setBusy] = useState(false);
  async function remove() {
    setBusy(true);
    try {
      await client.deleteWiki({ id: wiki.id });
      void navigate({ to: "/registry/wikis" });
    } catch (e) {
      setError(message(e));
      setDeleting(false);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <section className="space-y-3">
        <h2 className="text-lg font-semibold">{wiki.name}</h2>
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <span className="text-muted-foreground">
            Owner:{" "}
            <OwnerLink agentId={wiki.agentId} name={registry.get(wiki.agentId ?? "")?.name} />
          </span>
        </div>
      </section>
      <form
        className="grid max-w-2xl gap-4"
        aria-label="Wiki settings"
        onSubmit={form.handleSubmit(async (values) => {
          setError("");
          try {
            const { wiki: next } = await client.updateWiki({
              id: wiki.id,
              name: values.name.trim(),
              description: values.description.trim(),
            });
            if (next) {
              onSaved(next);
              form.reset({ name: next.name, description: next.description });
            }
          } catch (e) {
            setError(message(e));
          }
        })}
      >
        <div className="space-y-2">
          <Label htmlFor="wiki-settings-name">Name</Label>
          <Input
            id="wiki-settings-name"
            aria-invalid={!!form.formState.errors.name}
            {...form.register("name", { validate: (value) => !!value.trim() })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="wiki-settings-description">Description</Label>
          <Textarea id="wiki-settings-description" {...form.register("description")} />
          <p className="text-xs text-muted-foreground">
            Agents see it in their wiki tools, so say what this wiki covers.
          </p>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="flex justify-end">
          <Button type="submit" disabled={!form.formState.isDirty || form.formState.isSubmitting}>
            {form.formState.isSubmitting ? "Saving…" : "Save"}
          </Button>
        </div>
      </form>
      <WikiAgents wiki={wiki} registry={registry} onChanged={onSaved} />
      <section className="grid max-w-2xl gap-3 border-t pt-6" aria-label="Delete wiki">
        <h3 className="text-sm font-medium">Delete wiki</h3>
        <p className="text-xs text-muted-foreground">
          Its pages and their history are deleted and linked agents lose it.
        </p>
        <div>
          <Button variant="destructive" disabled={busy} onClick={() => setDeleting(true)}>
            Delete wiki
          </Button>
        </div>
      </section>
      <AlertDialog open={deleting} onOpenChange={(next) => !busy && setDeleting(next)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {wiki.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Its {wiki.pageCount} pages and their history are deleted and linked agents lose it.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void remove()}>
              Delete
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

/** The agents linked to the wiki, and how. */
function WikiAgents({
  wiki,
  registry,
  onChanged,
}: {
  wiki: Wiki;
  registry: Map<string, Agent>;
  onChanged: (wiki: Wiki) => void;
}) {
  let editable = true;
  const [error, setError] = useState("");
  const change = (write: () => Promise<unknown>) =>
    write()
      .then(() => client.getWiki({ id: wiki.id }))
      .then(({ wiki }) => wiki && onChanged(wiki))
      .then(() => setError(""))
      .catch((e) => setError(message(e)));
  return (
    <>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <LinkedAgentsTable
        links={wiki.links}
        own={wiki.agentId}
        registry={registry}
        editable={editable}
        hint={
          wiki.agentId
            ? "This is an agent's own wiki; no other agent can be linked to it."
            : "Agents read the wikis they are linked to, and edit those they may write to."
        }
        unlinkDescription="It no longer reads or edits this wiki."
        onSet={(agentId, access) =>
          change(() => client.setAgentWiki({ agentId, wikiId: wiki.id, access }))
        }
        onUnlink={(agentId) => change(() => client.removeAgentWiki({ agentId, wikiId: wiki.id }))}
      />
    </>
  );
}
