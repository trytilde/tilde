import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { Code, ConnectError } from "@connectrpc/connect";
import { Link, useBlocker, useNavigate, useRouterState } from "@tanstack/react-router";
import { timestampDate, type Timestamp } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import { FilePlusIcon, HistoryIcon, PencilIcon } from "lucide-react";
import { NativeSelect } from "@trytilde/connection-ui";
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
import { WikiMarkdown } from "./wiki-markdown";
import { LinkedAgentsTable, OwnerLink } from "./linked-agents";
import { RemoveButton } from "./remove-button";
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
        <TabsList aria-label="Wiki">
          <TabsTrigger value="pages">Pages</TabsTrigger>
          <TabsTrigger value="settings">Settings</TabsTrigger>
        </TabsList>
      </DashboardNavigation>
      <TabsContent value="pages" className="flex min-h-0 flex-1 flex-col">
        {tab === "pages" && (
          <WikiPages wiki={wiki} registry={registry} path={page} onPathChange={onPageChange} />
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
/** Pages in path order, indented under the folders their paths name. */
function outline(pages: WikiPageSummary[]) {
  const rows: { key: string; depth: number; label: string; page?: WikiPageSummary }[] = [];
  const shown = new Set<string>();
  for (const page of pages) {
    const segments = page.path.split("/");
    for (let i = 1; i < segments.length; i++) {
      const folder = segments.slice(0, i).join("/");
      if (shown.has(folder) || pages.some((p) => p.path === folder)) continue;
      shown.add(folder);
      rows.push({ key: `folder:${folder}`, depth: i - 1, label: segments[i - 1] });
    }
    rows.push({ key: page.id, depth: segments.length - 1, label: page.title, page });
  }
  return rows;
}

/** A search excerpt, whose matched words the server brackets with U+E000 and U+E001. */
function Excerpt({ text }: { text: string }) {
  return text.split(/[\uE000\uE001]/).map((part, index) =>
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
const CodeEditor = lazy(() => import("./code-editor"));

type Finding =
  | { kind: "search"; results: WikiSearchResult[] }
  | {
      kind: "grep";
      lines: WikiGrepLine[];
      more: boolean;
    };
function WikiPages({
  wiki,
  registry,
  path,
  onPathChange,
}: {
  wiki: Wiki;
  registry: Map<string, Agent>;
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
  const [creating, setCreating] = useState(false);
  const [moving, setMoving] = useState(false);
  const [history, setHistory] = useState(false);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"search" | "grep" | "regex">("search");
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
  const select = onPathChange;
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

  const rows = useMemo(() => outline(pages), [pages]);
  async function changed(next?: Page) {
    await loadPages();
    if (next) {
      select(next.path);
      await loadPage(next.id);
    }
  }
  return (
    <div className="grid min-h-0 flex-1 lg:grid-cols-[18rem_minmax(0,1fr)]">
      <aside className="flex min-h-0 flex-col gap-3 border-b p-4 lg:border-r lg:border-b-0">
        <form
          className="grid gap-2"
          aria-label="Find in this wiki"
          onSubmit={(event) => {
            event.preventDefault();
            void find();
          }}
        >
          <CatalogSearchField
            label={mode === "search" ? "Search pages" : "Find lines"}
            value={query}
            onChange={(value) => {
              setQuery(value);
              if (!value.trim()) setFinding(undefined);
            }}
          />
          <NativeSelect
            aria-label="How to find"
            value={mode}
            onChange={(event) => setMode(event.target.value as typeof mode)}
          >
            <option value="search">Search words</option>
            <option value="grep">Find exact text</option>
            <option value="regex">Find a regular expression</option>
          </NativeSelect>
        </form>
        {finding ? (
          <ul className="grid gap-1 overflow-auto text-sm" aria-label="Found">
            {finding.kind === "search" ? (
              finding.results.length ? (
                finding.results.map((r) => (
                  <li key={r.page?.id}>
                    <button
                      type="button"
                      className="w-full rounded-md px-2 py-1.5 text-left hover:bg-muted"
                      onClick={() => r.page && select(r.page.path)}
                    >
                      <span className="block font-medium">{r.page?.title}</span>
                      <span className="block text-xs text-muted-foreground">
                        <Excerpt text={r.excerpt} />
                      </span>
                    </button>
                  </li>
                ))
              ) : (
                <li className="text-xs text-muted-foreground">No pages match.</li>
              )
            ) : finding.lines.length ? (
              <>
                {finding.lines.map((l) => (
                  <li key={`${l.pagePath}:${l.lineNumber}`}>
                    <button
                      type="button"
                      className="w-full rounded-md px-2 py-1 text-left font-mono text-xs hover:bg-muted"
                      onClick={() => select(l.pagePath)}
                    >
                      <span className="text-muted-foreground">
                        {l.pagePath}:{l.lineNumber}
                      </span>{" "}
                      {l.line}
                      {l.truncated && <span className="text-muted-foreground">…</span>}
                    </button>
                  </li>
                ))}
                {finding.more && (
                  <li className="text-xs text-muted-foreground">More lines matched.</li>
                )}
              </>
            ) : (
              <li className="text-xs text-muted-foreground">No lines match.</li>
            )}
          </ul>
        ) : (
          <nav className="grid gap-0.5 overflow-auto text-sm" aria-label="Pages">
            {rows.length ? (
              rows.map((row) =>
                row.page ? (
                  <button
                    key={row.key}
                    type="button"
                    aria-current={row.page.id === selected ? "page" : undefined}
                    className="truncate rounded-md px-2 py-1 text-left hover:bg-muted aria-[current=page]:bg-muted aria-[current=page]:font-medium"
                    style={{ paddingLeft: `${0.5 + row.depth}rem` }}
                    onClick={() => select(row.page!.path)}
                  >
                    {row.label}
                  </button>
                ) : (
                  <span
                    key={row.key}
                    className="px-2 py-1 text-xs text-muted-foreground"
                    style={{ paddingLeft: `${0.5 + row.depth}rem` }}
                  >
                    {row.label}/
                  </span>
                ),
              )
            ) : (
              <p className="px-2 text-xs text-muted-foreground">No pages yet.</p>
            )}
            {more && (
              <p className="px-2 text-xs text-muted-foreground">
                Only the first pages are listed; search to find the rest.
              </p>
            )}
          </nav>
        )}
        <Button variant="outline" className="gap-2" onClick={() => setCreating(true)}>
          <FilePlusIcon />
          New page
        </Button>
      </aside>
      <section className="flex min-h-0 flex-col gap-4 overflow-auto p-4 lg:p-6" aria-label="Page">
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        {page ? (
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
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <h2 className="text-lg font-semibold">{page.title}</h2>
                  <p className="font-mono text-xs text-muted-foreground">
                    {page.path} · revision {page.revision} · edited {ago(page.updatedAt)}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    className="gap-1.5"
                    onClick={() => setHistory(true)}
                  >
                    <HistoryIcon />
                    History
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    className="gap-1.5"
                    onClick={() => setEditing(true)}
                  >
                    <PencilIcon />
                    Edit
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => setMoving(true)}>
                    Move
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
                </div>
              </div>
              <WikiMarkdown content={page.content} label={page.title} wikiId={wiki.id} />
            </>
          )
        ) : (
          <p className="text-sm text-muted-foreground">
            {path && loaded && !selected ? (
              <>
                No page at <code className="font-mono">{path}</code>.
                {editable && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="ml-3"
                    onClick={() => setCreating(true)}
                  >
                    Create it
                  </Button>
                )}
              </>
            ) : pages.length ? (
              "Choose a page."
            ) : (
              "This wiki has no pages yet."
            )}
          </p>
        )}
      </section>
      <CreatePageDialog
        wiki={wiki}
        path={selected ? "" : (path ?? "")}
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(next) => void changed(next)}
      />
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
 * Editing a page's title and raw Markdown at the revision it was opened at. When someone else
 * saved first, it shows what changed since and lets the writer save on top of the latest
 * revision or discard their draft.
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
  const form = useForm({ defaultValues: { title: page.title, content: page.content } });
  const [error, setError] = useState("");
  // The revision the draft saves on top of: the one it opened, or the latest after a conflict.
  const [base, setBase] = useState(page.revision);
  const [latest, setLatest] = useState<Page>();
  const dirty = form.formState.isDirty;
  // Leaving with unsaved changes asks first: other pages, tabs, links, history and the browser.
  // Set once the draft is saved or deliberately discarded, so the navigation that follows is not
  // asked about.
  const leaving = useRef(false);
  useBlocker({
    shouldBlockFn: () =>
      !leaving.current && dirty && !window.confirm("Discard your unsaved changes to this page?"),
    enableBeforeUnload: () => !leaving.current && dirty,
  });
  // Saves at `revision`: the one the draft opened at, or the latest once the writer chose to.
  const save = (revision: number) =>
    form.handleSubmit(async (values) => {
      setError("");
      try {
        const { page: next } = await client.updateWikiPage({
          id: page.id,
          expectedRevision: revision,
          title: values.title.trim(),
          content: values.content,
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
  return (
    <form className="grid gap-4" aria-label="Edit page" onSubmit={save(base)}>
      {latest && (
        <section
          aria-label="Changed since you opened it"
          className="grid gap-3 rounded-lg border border-amber-500/50 bg-amber-500/5 p-3"
        >
          <p className="text-sm">
            Someone saved revision {latest.revision} while you were editing. Their changes since you
            opened the page:
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
      <div className="space-y-2">
        <Label htmlFor="page-title">Title</Label>
        <Input
          id="page-title"
          aria-invalid={!!form.formState.errors.title}
          {...form.register("title", { validate: (value) => !!value.trim() })}
        />
      </div>
      <div className="space-y-2">
        <Label>Markdown</Label>
        <div className="h-[60vh] overflow-hidden rounded-lg border">
          <Controller
            control={form.control}
            name="content"
            render={({ field }) => (
              <Suspense fallback={<TildeLoader className="h-full" />}>
                <CodeEditor
                  path={`${page.path}.md`}
                  value={field.value}
                  readOnly={false}
                  onChange={field.onChange}
                />
              </Suspense>
            )}
          />
        </div>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button
          type="button"
          variant="outline"
          disabled={form.formState.isSubmitting}
          onClick={() => {
            if (dirty && !window.confirm("Discard your unsaved changes to this page?")) return;
            onCancel();
          }}
        >
          Cancel
        </Button>
        <Button type="submit" disabled={form.formState.isSubmitting || !!latest}>
          {form.formState.isSubmitting ? "Saving…" : "Save"}
        </Button>
      </div>
    </form>
  );
}

function CreatePageDialog({
  wiki,
  path,
  open,
  onClose,
  onCreated,
}: {
  wiki: Wiki;
  /** Filled in when creating a page a link or URL named. */
  path: string;
  open: boolean;
  onClose: () => void;
  onCreated: (page: Page) => void;
}) {
  const form = useForm({ defaultValues: { path, title: "", content: "" } });
  const [error, setError] = useState("");
  useEffect(() => {
    if (open) form.reset({ path, title: "", content: "" });
  }, [open, path, form]);
  function close() {
    onClose();
    form.reset();
    setError("");
  }
  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>New page</DialogTitle>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { page } = await client.createWikiPage({
                wikiId: wiki.id,
                path: values.path.trim(),
                title: values.title.trim(),
                content: values.content,
              });
              if (page) onCreated(page);
              close();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="new-page-path">Path</Label>
            <Input
              id="new-page-path"
              placeholder="runbooks/deploys"
              aria-invalid={!!form.formState.errors.path}
              {...form.register("path", { validate: (value) => !!value.trim() })}
            />
            <p className="text-xs text-muted-foreground">
              Slash-separated segments of letters, digits, '-', '_' and '.'.
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="new-page-title">Title</Label>
            <Input
              id="new-page-title"
              aria-invalid={!!form.formState.errors.title}
              {...form.register("title", { validate: (value) => !!value.trim() })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="new-page-content">Content</Label>
            <Textarea
              id="new-page-content"
              className="min-h-40 font-mono text-xs"
              placeholder="Markdown"
              {...form.register("content")}
            />
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              {form.formState.isSubmitting ? "Creating…" : "Create page"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
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
                          to="/agent/$agentId/sessions"
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
      void navigate({ to: "/wikis" });
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
