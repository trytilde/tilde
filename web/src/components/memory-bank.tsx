import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Controller, useForm } from "react-hook-form";
import { Link, useBlocker, useNavigate, useRouterState } from "@tanstack/react-router";
import { timestampDate, timestampFromDate, type Timestamp } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  BotIcon,
  ChevronRightIcon,
  FolderIcon,
  FolderOpenIcon,
  LayersIcon,
  PencilIcon,
  PlusIcon,
  SparklesIcon,
  UserIcon,
  XIcon,
} from "lucide-react";
import { NativeSelect } from "@trytilde/connection-ui";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  MemoryBankScope,
  MemoryActor,
  MemoryKind,
  MemoryMatch,
  MemoryOperation,
  MemoryOrigin,
  type Memory,
  type MemoryBank,
  type MemoryPath,
  type MemoryPerson,
  type MemoryRevision,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import { memory } from "@/client";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { useAgentRegistry } from "./agent-stack";
import { DashboardNavigation, usePageCrumb } from "./dashboard-breadcrumbs";
import { TildeLoader } from "./loading-screen";
import { MarkdownEditor, saveKey } from "./markdown-editor";
import { KINDS, SCOPES, scopeOf } from "./memory";
import { LinkedAgentsTable, OwnerLink } from "./linked-agents";
import { BankSynthesis, SourceLink } from "./memory-synthesis";
import { RemoveButton } from "./remove-button";
import { ToolbarSelect } from "./toolbar-select";
import { CatalogSearchField } from "./tool-catalog";
import { message } from "./tool-connections";
import { Badge } from "@/components/ui/badge";
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

export const BANK_TABS = ["memories", "settings"] as const;
export type BankTab = (typeof BANK_TABS)[number];
const PAGE = 50;

/** One memory bank: its memories, and its settings and linked agents. */
export function MemoryBankPage({
  bankId,
  tab,
  onTabChange,
}: {
  bankId: string;
  tab: BankTab;
  onTabChange: (tab: BankTab) => void;
}) {
  const [bank, setBank] = useState<MemoryBank>();
  const [error, setError] = useState("");
  // The header row's right side, where the memories tab puts its search, filters and Add fact.
  const [toolbar, setToolbar] = useState<HTMLDivElement | null>(null);
  const registry = useAgentRegistry();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  usePageCrumb(pathname, bank?.name);
  const reload = useCallback(
    (signal?: AbortSignal) =>
      memory
        .getMemoryBank({ id: bankId }, { signal })
        .then(({ bank }) => {
          if (!bank) throw new Error("Memory bank not found.");
          setBank(bank);
        })
        .catch((e) => !signal?.aborted && setError(message(e))),
    [bankId],
  );
  useEffect(() => {
    const abort = new AbortController();
    void reload(abort.signal);
    return () => abort.abort();
  }, [reload]);
  if (!bank)
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
      onValueChange={(value) => onTabChange(value as BankTab)}
    >
      <DashboardNavigation>
        <div className="flex min-w-max items-center gap-3">
          <TabsList aria-label="Memory bank">
            <TabsTrigger value="memories">Memories</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>
          <div
            ref={setToolbar}
            role="group"
            aria-label="Memory filters"
            className="trace-controls ml-auto flex items-center justify-end gap-1.5 empty:hidden"
          />
        </div>
      </DashboardNavigation>
      <TabsContent value="memories" className="flex min-h-0 flex-1 flex-col">
        {tab === "memories" && (
          <BankMemories
            bank={bank}
            registry={registry}
            toolbar={toolbar}
            onChanged={() => void reload()}
          />
        )}
      </TabsContent>
      <TabsContent
        value="settings"
        className="mx-auto grid w-full max-w-5xl gap-6 px-4 py-6 lg:px-8"
      >
        <BankSettings bank={bank} registry={registry} onSaved={setBank} />
      </TabsContent>
    </Tabs>
  );
}

const when = (t?: Timestamp) => (t ? timestampDate(t).toLocaleString() : "");
const ago = (t?: Timestamp) =>
  t ? formatDistanceToNow(timestampDate(t), { addSuffix: true }) : "";
/** How long ago, in a few characters for a table column: 5m, 3h, 2d, then the date. */
function short(t?: Timestamp) {
  if (!t) return "";
  const date = timestampDate(t);
  const minutes = Math.max(0, Math.round((Date.now() - date.getTime()) / 60_000));
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 60 * 24) return `${Math.round(minutes / 60)}h`;
  if (minutes < 60 * 24 * 30) return `${Math.round(minutes / 60 / 24)}d`;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
/** When a memory holds: a moment, a period, ongoing, or always. */
function validity(m: Pick<Memory, "validFrom" | "validTo">) {
  if (!m.validFrom) return "";
  const from = timestampDate(m.validFrom);
  if (!m.validTo) return `Since ${from.toLocaleString()} (ongoing)`;
  const to = timestampDate(m.validTo);
  return to.getTime() === from.getTime()
    ? from.toLocaleString()
    : `${from.toLocaleString()} – ${to.toLocaleString()}`;
}

type Kind = keyof typeof KINDS;
/** A dot per kind, so a column of kinds reads at a glance. */
const KIND_TONES: Record<Kind, string> = {
  [MemoryKind.PROFILE]: "bg-sky-500",
  [MemoryKind.PREFERENCES]: "bg-violet-500",
  [MemoryKind.ENTITIES]: "bg-emerald-500",
  [MemoryKind.EVENTS]: "bg-amber-500",
  [MemoryKind.CASES]: "bg-rose-500",
  [MemoryKind.PATTERNS]: "bg-indigo-500",
  [MemoryKind.TOOLS]: "bg-stone-500",
  [MemoryKind.SKILLS]: "bg-teal-500",
};
function KindLabel({ kind }: { kind: MemoryKind }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span aria-hidden className={`size-1.5 shrink-0 rounded-full ${KIND_TONES[kind as Kind]}`} />
      {KINDS[kind as Kind] ?? "Memory"}
    </span>
  );
}

const ORIGINS = [
  { value: String(MemoryOrigin.UNSPECIFIED), label: "Any writer" },
  { value: String(MemoryOrigin.EXPLICIT), label: "Written by agents" },
  { value: String(MemoryOrigin.OWNER), label: "Written by people" },
  { value: String(MemoryOrigin.SYNTHESIZED), label: "Synthesized" },
];
const KIND_OPTIONS = [
  { value: String(MemoryKind.UNSPECIFIED), label: "Any kind" },
  ...Object.entries(KINDS).map(([value, label]) => ({ value, label })),
];

type Node = { path: string; name: string; depth: number; count: number };
/** Every path with memories, and its ancestors, with how many memories each holds in total. */
function tree(paths: MemoryPath[]): Node[] {
  const totals = new Map<string, number>();
  for (const { path, count } of paths) {
    const segments = path ? path.split("/") : [];
    for (let i = 1; i <= segments.length; i++) {
      const prefix = segments.slice(0, i).join("/");
      totals.set(prefix, (totals.get(prefix) ?? 0) + count);
    }
  }
  return [...totals.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([path, count]) => ({
      path,
      name: path.slice(path.lastIndexOf("/") + 1),
      depth: path.split("/").length - 1,
      count,
    }));
}
const parentsOf = (path: string) =>
  path
    .split("/")
    .slice(0, -1)
    .map((_, i, all) => all.slice(0, i + 1).join("/"));

/** Who wrote a memory, as an icon and a name. */
function Writer({
  m,
  registry,
  personName,
}: {
  m: Memory;
  registry: Map<string, Agent>;
  personName: (id?: string) => string;
}) {
  const [Icon, name] =
    m.origin === MemoryOrigin.OWNER
      ? [UserIcon, "A person"]
      : m.origin === MemoryOrigin.SYNTHESIZED
        ? [SparklesIcon, "Synthesis"]
        : [BotIcon, registry.get(m.writtenByAgentId ?? "")?.name ?? "An agent"];
  return (
    <span className="flex min-w-0 items-center gap-1.5" title={name}>
      <Icon aria-hidden className="size-3 shrink-0 text-muted-foreground" />
      <span className="truncate">
        {name}
        {m.identityId && (
          <span className="text-muted-foreground"> · for {personName(m.identityId)}</span>
        )}
      </span>
    </span>
  );
}

/**
 * A bank's memories, master-detail: the folder tree its paths make, a dense table of the
 * memories in the chosen folder (or ranked by a search), and the chosen memory's panel, which
 * also edits and adds facts. Search, filters and Add fact sit in the page header's tab row.
 */
function BankMemories({
  bank,
  registry,
  toolbar,
  onChanged,
}: {
  bank: MemoryBank;
  registry: Map<string, Agent>;
  toolbar: HTMLDivElement | null;
  onChanged: () => void;
}) {
  let editable = true;
  const perPerson = scopeOf(bank.scope) === MemoryBankScope.PER_PERSON;
  const [paths, setPaths] = useState<MemoryPath[]>([]);
  const [people, setPeople] = useState<MemoryPerson[]>([]);
  const [path, setPath] = useState("");
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const [kind, setKind] = useState(MemoryKind.UNSPECIFIED);
  const [origin, setOrigin] = useState(MemoryOrigin.UNSPECIFIED);
  const [person, setPerson] = useState("");
  const [memories, setMemories] = useState<Memory[]>([]);
  const [next, setNext] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<string>();
  // A link to one memory (from a synthesis batch, say) opens its panel.
  const wanted = useRouterState({
    select: (state) => (state.location.search as { memory?: string }).memory,
  });
  useEffect(() => {
    if (wanted) setSelected(wanted);
  }, [wanted]);
  const navigate = useNavigate();
  /** Opens a memory's panel (or closes it), dropping a `?memory=` that named another. */
  function pick(id?: string) {
    setSelected(id);
    if (wanted && id !== wanted)
      void navigate({ to: ".", search: (prev) => ({ ...prev, memory: undefined }), replace: true });
  }
  // Below lg the folder tree folds away behind a toggle.
  const [folders, setFolders] = useState(false);
  // The memory being edited; null adds a fact.
  const [editing, setEditing] = useState<Memory | null>();
  const [version, setVersion] = useState(0);
  const changed = () => {
    setVersion((v) => v + 1);
    onChanged();
  };
  const page = useCallback(
    (pageToken: string, signal?: AbortSignal) =>
      memory.listMemories(
        {
          bankId: bank.id,
          query: search || undefined,
          path: path || undefined,
          kind,
          origin,
          identityId: person || undefined,
          limit: PAGE,
          pageToken,
        },
        { signal },
      ),
    [bank.id, search, path, kind, origin, person],
  );
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    page("", abort.signal)
      .then((list) => {
        setMemories(list.memories);
        setNext(list.nextPageToken);
        setError("");
      })
      .catch((e) => !abort.signal.aborted && setError(message(e)))
      .finally(() => !abort.signal.aborted && setLoading(false));
    return () => abort.abort();
  }, [page, version]);
  // The folders and people change only with the bank's memories, not with the filters.
  useEffect(() => {
    const abort = new AbortController();
    void Promise.all([
      memory.listMemoryPaths({ bankId: bank.id }, { signal: abort.signal }),
      perPerson
        ? memory.listMemoryPeople({ bankId: bank.id }, { signal: abort.signal })
        : Promise.resolve({ people: [] }),
    ])
      .then(([found, who]) => {
        setPaths(found.paths);
        setPeople(who.people);
      })
      .catch((e) => !abort.signal.aborted && setError(message(e)));
    return () => abort.abort();
  }, [bank.id, perPerson, version]);
  const nodes = useMemo(() => tree(paths), [paths]);
  const total = paths.reduce((sum, p) => sum + p.count, 0);
  const personName = (id?: string) => people.find((p) => p.identityId === id)?.name || "A person";
  const filtered = !!(search || kind || origin || person);
  const rows = useRef<HTMLTableSectionElement>(null);
  /** Arrow keys walk the table, opening each memory as they go. */
  function step(by: number) {
    const index = memories.findIndex((m) => m.id === selected);
    const target = memories[Math.min(Math.max(index + by, 0), memories.length - 1)];
    if (!target) return;
    pick(target.id);
    rows.current
      ?.querySelector<HTMLElement>(`[data-id="${target.id}"]`)
      ?.focus({ preventScroll: false });
  }
  const tools = (
    <>
      <div className="w-56">
        <CatalogSearchField label="Search memories" value={query} onChange={setQuery} />
      </div>
      <ToolbarSelect
        label="Kind"
        value={String(kind)}
        options={KIND_OPTIONS}
        onChange={(value) => setKind(Number(value) as MemoryKind)}
      />
      <ToolbarSelect
        label="Written by"
        value={String(origin)}
        options={ORIGINS}
        onChange={(value) => setOrigin(Number(value) as MemoryOrigin)}
      />
      {perPerson && (
        <ToolbarSelect
          label="Person"
          value={person}
          icon={<UserIcon className="text-muted-foreground" />}
          options={[
            { value: "", label: "Everyone" },
            ...people.map((p) => ({
              value: p.identityId,
              label: `${p.name || "Unnamed person"} (${p.count})`,
            })),
          ]}
          onChange={setPerson}
        />
      )}
      <Button
        size="sm"
        className="h-[26px] gap-1.5 px-2.5 text-[11px]"
        onClick={() => setEditing(null)}
      >
        <PlusIcon className="size-3.5" />
        Add fact
      </Button>
    </>
  );
  const panel = editing !== undefined || !!selected;
  return (
    <div
      className={`grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)] ${
        editing !== undefined
          ? "lg:grid-cols-[15rem_minmax(0,1fr)_minmax(0,1.3fr)]"
          : panel
            ? "lg:grid-cols-[15rem_minmax(0,1fr)_27rem]"
            : "lg:grid-cols-[15rem_minmax(0,1fr)]"
      }`}
    >
      {toolbar ? createPortal(tools, toolbar) : null}
      <FolderTree
        nodes={nodes}
        total={total}
        path={path}
        open={folders}
        onPath={(next) => {
          setPath(next);
          setFolders(false);
        }}
      />
      <section aria-label="Memories" className="@container flex min-h-0 flex-col">
        <div className="trace-controls flex h-8 shrink-0 items-center gap-2 border-b bg-muted/30 px-3 text-muted-foreground">
          <button
            type="button"
            aria-expanded={folders}
            aria-label={folders ? "Hide folders" : "Show folders"}
            className="flex shrink-0 items-center hover:text-foreground lg:hidden"
            onClick={() => setFolders(!folders)}
          >
            <FolderIcon className="size-3.5" />
          </button>
          <FolderCrumbs path={path} onPath={setPath} />
          <span className="ml-auto shrink-0 tabular-nums">
            {loading
              ? "Loading…"
              : search
                ? `${memories.length}${next ? "+" : ""} ranked by relevance`
                : `${memories.length}${next ? "+" : ""} ${memories.length === 1 ? "memory" : "memories"}`}
          </span>
        </div>
        {error && (
          <p role="alert" className="px-4 pt-3 text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="min-h-0 flex-1 overflow-y-auto">
          <table
            aria-label="Memory list"
            className="tracing-table w-full table-fixed border-collapse text-[11px]"
          >
            <thead className="sticky top-0 z-10 bg-background shadow-[inset_0_-1px_0_var(--border)]">
              <tr className="h-7 text-left text-muted-foreground [&>th]:px-3 [&>th]:font-semibold">
                <th>Memory</th>
                <th className="w-28">Kind</th>
                <th className="hidden w-32 @2xl:table-cell">Folder</th>
                <th className="hidden w-40 @xl:table-cell">Written by</th>
                <th className="w-20 text-right">Updated</th>
              </tr>
            </thead>
            <tbody
              ref={rows}
              onKeyDown={(event) => {
                if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                event.preventDefault();
                step(event.key === "ArrowDown" ? 1 : -1);
              }}
            >
              {memories.map((m) => (
                <tr
                  key={m.id}
                  data-id={m.id}
                  tabIndex={0}
                  aria-selected={selected === m.id}
                  className="h-8 cursor-pointer border-b border-border/50 outline-none hover:bg-muted/50 focus-visible:bg-accent aria-selected:bg-muted [&>td]:px-3 [&>td]:whitespace-nowrap"
                  onClick={() => pick(m.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") pick(m.id);
                  }}
                >
                  <td className="max-w-0 truncate">
                    <span
                      className={`font-semibold text-foreground ${m.supersededBy ? "line-through opacity-60" : ""}`}
                    >
                      {m.title}
                    </span>
                    {m.matched.map((how) => (
                      <Badge
                        key={how}
                        variant="secondary"
                        className="ml-1.5 h-4 px-1 align-middle text-[10px]"
                      >
                        {how === MemoryMatch.MEANING ? "meaning" : "keywords"}
                      </Badge>
                    ))}
                    <span className="ml-2 text-muted-foreground">{m.content}</span>
                  </td>
                  <td>
                    <KindLabel kind={m.kind} />
                  </td>
                  <td className="hidden max-w-0 truncate text-muted-foreground @2xl:table-cell">
                    {m.path || "/"}
                  </td>
                  <td className="hidden max-w-0 overflow-hidden @xl:table-cell">
                    <Writer m={m} registry={registry} personName={personName} />
                  </td>
                  <td
                    className="text-right text-muted-foreground tabular-nums"
                    title={when(m.updatedAt)}
                  >
                    {short(m.updatedAt)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {!memories.length && (
            <p className="p-8 text-center text-sm text-muted-foreground">
              {loading
                ? "Loading memories…"
                : filtered || path
                  ? "No memories match."
                  : "No memories yet. Agents linked to this bank write them as they learn."}
            </p>
          )}
          {next && (
            <div className="p-3 text-center">
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  void page(next)
                    .then((list) => {
                      setMemories((current) => [...current, ...list.memories]);
                      setNext(list.nextPageToken);
                    })
                    .catch((e) => setError(message(e)))
                }
              >
                Load more
              </Button>
            </div>
          )}
        </div>
      </section>
      {panel && (
        <aside
          aria-label={editing !== undefined ? "Edit memory" : "Memory detail"}
          className="flex min-h-0 flex-col border-t lg:border-t-0 lg:border-l"
        >
          {editing !== undefined ? (
            <MemoryEditor
              key={editing?.id ?? "new"}
              bank={bank}
              memory={editing ?? undefined}
              people={people}
              paths={nodes.map((n) => n.path)}
              path={path}
              onClose={() => setEditing(undefined)}
              onSaved={(m) => {
                setEditing(undefined);
                pick(m.id);
                changed();
              }}
            />
          ) : (
            selected && (
              <MemoryDetail
                key={`${selected}-${version}`}
                id={selected}
                registry={registry}
                personName={personName}
                editable={editable}
                onSelect={pick}
                onPath={setPath}
                onClose={() => pick(undefined)}
                onEdit={setEditing}
                onDeleted={() => {
                  pick(undefined);
                  changed();
                }}
              />
            )
          )}
        </aside>
      )}
    </div>
  );
}

/** The folders memory paths make, collapsible, each with how many memories it holds. */
function FolderTree({
  nodes,
  total,
  path,
  open,
  onPath,
}: {
  nodes: Node[];
  total: number;
  path: string;
  /** Shown below lg; it is always shown beside the table from lg up. */
  open: boolean;
  onPath: (path: string) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const parents = useMemo(() => new Set(nodes.flatMap((n) => parentsOf(n.path))), [nodes]);
  const toggle = (folder: string) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (!next.delete(folder)) next.add(folder);
      return next;
    });
  return (
    <nav
      aria-label="Memory paths"
      className={`trace-controls max-h-72 min-h-0 overflow-y-auto border-b bg-muted/20 p-2 lg:block lg:max-h-none lg:border-r lg:border-b-0 ${open ? "block" : "hidden"}`}
    >
      <p className="px-2 pt-1 pb-1.5 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
        Folders
      </p>
      <FolderRow
        label="All memories"
        icon={<LayersIcon className="size-3.5" />}
        count={total}
        depth={0}
        active={!path}
        onClick={() => onPath("")}
      />
      {nodes
        .filter((n) => !parentsOf(n.path).some((p) => collapsed.has(p)))
        .map((n) => (
          <FolderRow
            key={n.path}
            label={n.name}
            title={n.path}
            depth={n.depth + 1}
            count={n.count}
            active={path === n.path}
            open={parents.has(n.path) ? !collapsed.has(n.path) : undefined}
            onToggle={() => toggle(n.path)}
            icon={
              path === n.path ? (
                <FolderOpenIcon className="size-3.5" />
              ) : (
                <FolderIcon className="size-3.5" />
              )
            }
            onClick={() => onPath(n.path)}
          />
        ))}
      {!nodes.length && (
        <p className="px-2 py-2 text-muted-foreground">
          Memories with a path, like <code>team/conventions</code>, group into folders here.
        </p>
      )}
    </nav>
  );
}

function FolderRow({
  label,
  title,
  icon,
  depth,
  count,
  active,
  open,
  onToggle,
  onClick,
}: {
  label: string;
  title?: string;
  icon: ReactNode;
  depth: number;
  count: number;
  active: boolean;
  /** Whether a folder with subfolders is expanded; undefined for a leaf. */
  open?: boolean;
  onToggle?: () => void;
  onClick: () => void;
}) {
  return (
    <div
      className="group flex h-7 items-center rounded-md pr-2 hover:bg-muted aria-[current=true]:bg-muted aria-[current=true]:font-semibold"
      aria-current={active}
      style={{ paddingLeft: `${depth * 0.75}rem` }}
    >
      {open === undefined ? (
        <span className="w-5 shrink-0" />
      ) : (
        <button
          type="button"
          aria-label={open ? `Collapse ${label}` : `Expand ${label}`}
          aria-expanded={open}
          className="flex size-5 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-foreground"
          onClick={onToggle}
        >
          <ChevronRightIcon className={`size-3 transition-transform ${open ? "rotate-90" : ""}`} />
        </button>
      )}
      <button
        type="button"
        title={title}
        aria-current={active}
        className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
        onClick={onClick}
      >
        <span className="shrink-0 text-muted-foreground">{icon}</span>
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <span className="text-[10px] text-muted-foreground tabular-nums">{count}</span>
      </button>
    </div>
  );
}

/** The chosen folder as clickable segments. */
function FolderCrumbs({ path, onPath }: { path: string; onPath: (path: string) => void }) {
  const segments = path ? path.split("/") : [];
  return (
    <nav aria-label="Folder" className="flex min-w-0 items-center gap-1 truncate">
      <button
        type="button"
        className="shrink-0 hover:text-foreground"
        onClick={() => onPath("")}
        aria-current={!path}
      >
        All memories
      </button>
      {segments.map((segment, i) => {
        const prefix = segments.slice(0, i + 1).join("/");
        return (
          <span key={prefix} className="flex min-w-0 items-center gap-1">
            <ChevronRightIcon aria-hidden className="size-3 shrink-0" />
            <button
              type="button"
              className="truncate hover:text-foreground aria-[current=true]:font-semibold aria-[current=true]:text-foreground"
              aria-current={prefix === path}
              onClick={() => onPath(prefix)}
            >
              {segment}
            </button>
          </span>
        );
      })}
    </nav>
  );
}

/** A memory's text: agents and people often write Markdown. Raw HTML is not rendered. */
function MemoryContent({ content }: { content: string }) {
  return (
    <div className="wiki-prose">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          a: ({ children, href }) => (
            <a href={href} target="_blank" rel="noopener noreferrer">
              {children}
            </a>
          ),
          img: ({ src, alt }) => (
            <a href={String(src ?? "")} target="_blank" rel="noopener noreferrer">
              [Image{alt ? `: ${alt}` : ""}]
            </a>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}

const OPERATIONS: Record<MemoryOperation, string> = {
  [MemoryOperation.UNSPECIFIED]: "Changed",
  [MemoryOperation.CREATE]: "Written",
  [MemoryOperation.UPDATE]: "Edited",
  [MemoryOperation.SUPERSEDE]: "Replaced",
  [MemoryOperation.FORGET]: "Forgotten",
};

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="space-y-2 border-t px-4 py-3">
      <h3 className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
        {title}
      </h3>
      {children}
    </section>
  );
}

/** Another memory by id, named by its title once loaded. */
function MemoryLink({
  id,
  prefix,
  onSelect,
}: {
  id: string;
  prefix: string;
  onSelect: (id: string) => void;
}) {
  const [title, setTitle] = useState<string>();
  useEffect(() => {
    const abort = new AbortController();
    memory
      .getMemory({ id }, { signal: abort.signal })
      .then(({ memory }) => setTitle(memory?.title))
      .catch(() => undefined);
    return () => abort.abort();
  }, [id]);
  return (
    <button
      type="button"
      className="flex w-full items-baseline gap-2 rounded-md px-2 py-1 text-left text-xs hover:bg-muted"
      onClick={() => onSelect(id)}
    >
      <span className="shrink-0 text-muted-foreground">{prefix}</span>
      <span className="min-w-0 truncate underline-offset-2 hover:underline">
        {title ?? "Another memory"}
      </span>
    </button>
  );
}

/** Up to five other current memories in the same folder (or below it). */
function SameFolder({
  m,
  onSelect,
  onPath,
}: {
  m: Memory;
  onSelect: (id: string) => void;
  onPath: (path: string) => void;
}) {
  const [others, setOthers] = useState<Memory[]>([]);
  useEffect(() => {
    if (!m.path) return;
    const abort = new AbortController();
    memory
      .listMemories({ bankId: m.bankId, path: m.path, limit: 6 }, { signal: abort.signal })
      .then(({ memories }) => setOthers(memories.filter((o) => o.id !== m.id).slice(0, 5)))
      .catch(() => undefined);
    return () => abort.abort();
  }, [m.bankId, m.path, m.id]);
  if (!m.path || !others.length) return null;
  return (
    <Section title={`Also in ${m.path}`}>
      <ul className="-mx-2">
        {others.map((o) => (
          <li key={o.id}>
            <button
              type="button"
              className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-xs hover:bg-muted"
              onClick={() => onSelect(o.id)}
            >
              <span
                aria-hidden
                className={`size-1.5 shrink-0 rounded-full ${KIND_TONES[o.kind as Kind]}`}
              />
              <span className="min-w-0 truncate">{o.title}</span>
            </button>
          </li>
        ))}
      </ul>
      <button
        type="button"
        className="text-xs text-muted-foreground underline-offset-2 hover:underline"
        onClick={() => onPath(m.path)}
      >
        Show the whole folder
      </button>
    </Section>
  );
}

function MemoryDetail({
  id,
  registry,
  personName,
  editable,
  onSelect,
  onPath,
  onClose,
  onEdit,
  onDeleted,
}: {
  id: string;
  registry: Map<string, Agent>;
  personName: (id?: string) => string;
  editable: boolean;
  onSelect: (id: string) => void;
  onPath: (path: string) => void;
  onClose: () => void;
  onEdit: (memory: Memory) => void;
  onDeleted: () => void;
}) {
  const [found, setFound] = useState<{ memory: Memory; revisions: MemoryRevision[] }>();
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    memory
      .getMemory({ id }, { signal: abort.signal })
      .then(({ memory, revisions }) => memory && setFound({ memory, revisions }))
      .catch((e) => !abort.signal.aborted && setError(message(e)));
    return () => abort.abort();
  }, [id]);
  if (!found)
    return error ? (
      <p role="alert" className="p-4 text-sm text-destructive">
        {error}
      </p>
    ) : null;
  const { memory: m, revisions } = found;
  const agentName = (agent?: string) => registry.get(agent ?? "")?.name ?? "An agent";
  const facts: [string, ReactNode][] = [
    [
      "Folder",
      m.path ? (
        <button
          type="button"
          className="font-mono underline-offset-2 hover:underline"
          onClick={() => onPath(m.path)}
        >
          {m.path}
        </button>
      ) : (
        "The bank's root"
      ),
    ],
    ["Holds", validity(m) || "Always"],
    [
      "Importance",
      <span className="inline-flex items-center gap-2">
        <span className="h-1 w-16 overflow-hidden rounded-full bg-muted">
          <span
            className="block h-full bg-foreground/60"
            style={{ width: `${Math.round(m.importance * 100)}%` }}
          />
        </span>
        <span className="tabular-nums">{m.importance.toFixed(2)}</span>
      </span>,
    ],
    [
      "Written by",
      m.origin === MemoryOrigin.OWNER
        ? "A person (agents and synthesis cannot change it)"
        : m.origin === MemoryOrigin.SYNTHESIZED
          ? "Synthesis, from the bank's sources"
          : agentName(m.writtenByAgentId),
    ],
    ...(m.identityId ? [["For", personName(m.identityId)] as [string, string]] : []),
    ["Embedded", m.embedded ? "Yes" : "Not yet"],
    ["Created", <span title={when(m.createdAt)}>{ago(m.createdAt)}</span>],
  ];
  const sources = m.evidence.length > 0 || (m.threadId && m.writtenByAgentId);
  return (
    <article className="min-h-0 flex-1 overflow-y-auto" aria-label={m.title}>
      <header className="sticky top-0 z-10 border-b bg-background/95 px-4 py-3 backdrop-blur">
        <div className="flex items-start gap-2">
          <div className="min-w-0 flex-1 space-y-1">
            <p className="text-[11px] text-muted-foreground">
              <KindLabel kind={m.kind} />
            </p>
            <h2 className="text-base leading-snug font-semibold">{m.title}</h2>
          </div>
          {editable && (
            <>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={m.origin === MemoryOrigin.OWNER ? "Edit memory" : "Take over and edit"}
                title={
                  m.origin === MemoryOrigin.OWNER
                    ? "Edit memory"
                    : "Take over and edit: the memory becomes a person's, which agents cannot change"
                }
                onClick={() => onEdit(m)}
              >
                <PencilIcon />
              </Button>
              <RemoveButton
                label="Delete memory"
                title={`Delete ${m.title}?`}
                description="Agents no longer recall it. Its history is kept."
                confirmLabel="Delete"
                onConfirm={() =>
                  memory
                    .deleteMemory({ id: m.id })
                    .then(onDeleted)
                    .catch((e) => setError(message(e)))
                }
              />
            </>
          )}
          <Button variant="ghost" size="icon-sm" aria-label="Close" onClick={onClose}>
            <XIcon />
          </Button>
        </div>
      </header>
      {m.supersededBy && (
        <p className="mx-4 mt-3 rounded-md bg-muted px-3 py-2 text-xs">
          Replaced by a newer memory, so agents no longer recall it.{" "}
          <button type="button" className="underline" onClick={() => onSelect(m.supersededBy!)}>
            Show the current one
          </button>
        </p>
      )}
      <div className="px-4 py-3">
        <MemoryContent content={m.content} />
      </div>
      {error && (
        <p role="alert" className="px-4 pb-3 text-sm text-destructive">
          {error}
        </p>
      )}
      <Section title="Properties">
        <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-xs">
          {facts.map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-muted-foreground">{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      </Section>
      {(m.supersedesId || m.supersededBy) && (
        <Section title="Versions">
          <div className="-mx-2">
            {m.supersededBy && (
              <MemoryLink id={m.supersededBy} prefix="Replaced by" onSelect={onSelect} />
            )}
            {m.supersedesId && (
              <MemoryLink id={m.supersedesId} prefix="Replaces" onSelect={onSelect} />
            )}
          </div>
        </Section>
      )}
      {sources && (
        <Section title="Sources">
          <ul className="space-y-1 text-xs">
            {m.threadId && m.writtenByAgentId && (
              <li>
                <Link
                  to="/registry/agent/$agentId/sessions"
                  params={{ agentId: m.writtenByAgentId }}
                  search={{ session: m.threadId }}
                  className="underline"
                >
                  The conversation it came from
                </Link>
              </li>
            )}
            {m.evidence.map((e, index) => (
              <li key={index}>
                <SourceLink source={e} agentName={(id) => registry.get(id)?.name ?? "an agent"} />
              </li>
            ))}
          </ul>
        </Section>
      )}
      <SameFolder m={m} onSelect={onSelect} onPath={onPath} />
      <Section title="History">
        <ol className="relative space-y-3 border-l pl-4">
          {revisions.map((r, index) => (
            <li key={index} className="relative text-xs">
              <span
                aria-hidden
                className="absolute top-1 -left-[1.3rem] size-2 rounded-full border-2 border-background bg-muted-foreground/60"
              />
              <span className="font-medium">{OPERATIONS[r.operation]}</span>{" "}
              <span className="text-muted-foreground">
                by{" "}
                {r.actor === MemoryActor.SYNTHESIZER
                  ? "synthesis"
                  : r.agentId
                    ? agentName(r.agentId)
                    : "a person"}
                {r.at && `, ${formatDistanceToNow(timestampDate(r.at), { addSuffix: true })}`}
              </span>
              {r.operation !== MemoryOperation.FORGET && (
                <span className="mt-0.5 block truncate text-muted-foreground" title={r.content}>
                  {r.title}: {r.content}
                </span>
              )}
            </li>
          ))}
        </ol>
      </Section>
    </article>
  );
}

type FactValues = {
  kind: string;
  path: string;
  title: string;
  content: string;
  importance: number;
  /** datetime-local values; empty when unset. */
  validFrom: string;
  validTo: string;
  ongoing: boolean;
  identityId: string;
};
const local = (t?: Timestamp) => {
  if (!t) return "";
  const date = timestampDate(t);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
};
const stamp = (value: string) => (value ? timestampFromDate(new Date(value)) : undefined);
function factValues(m: Memory | undefined, path: string): FactValues {
  const moment = m?.validFrom && m.validTo && local(m.validFrom) === local(m.validTo);
  return {
    kind: String(m?.kind || MemoryKind.ENTITIES),
    path: m?.path ?? path,
    title: m?.title ?? "",
    content: m?.content ?? "",
    importance: m?.importance ?? 0.5,
    validFrom: local(m?.validFrom),
    validTo: moment ? "" : local(m?.validTo),
    ongoing: !!m?.validFrom && !m.validTo,
    identityId: m?.identityId ?? "",
  };
}

/**
 * A person's fact, new or edited, in the detail pane. Agents recall it but never change it;
 * editing an agent's memory makes it the person's.
 */
function MemoryEditor({
  bank,
  memory: editing,
  people,
  paths,
  path,
  onClose,
  onSaved,
}: {
  bank: MemoryBank;
  memory?: Memory;
  people: MemoryPerson[];
  /** Existing folders, offered while typing a path. */
  paths: string[];
  path: string;
  onClose: () => void;
  onSaved: (memory: Memory) => void;
}) {
  const form = useForm<FactValues>({ defaultValues: factValues(editing, path) });
  const [error, setError] = useState("");
  const ongoing = form.watch("ongoing");
  const importance = form.watch("importance");
  const perPerson = scopeOf(bank.scope) === MemoryBankScope.PER_PERSON;
  const dirty = form.formState.isDirty;
  // Set once saved, so the navigation that follows is not asked about.
  const leaving = useRef(false);
  useBlocker({
    shouldBlockFn: () =>
      !leaving.current && dirty && !window.confirm("Discard your unsaved changes to this memory?"),
    enableBeforeUnload: () => !leaving.current && dirty,
  });
  function cancel() {
    if (dirty && !window.confirm("Discard your unsaved changes to this memory?")) return;
    onClose();
  }
  const submit = form.handleSubmit(async (values) => {
    setError("");
    const fields = {
      kind: Number(values.kind) as MemoryKind,
      path: values.path.trim(),
      title: values.title.trim(),
      content: values.content.trim(),
      importance: Number(values.importance),
      validFrom: stamp(values.validFrom),
      validTo: values.ongoing ? undefined : stamp(values.validTo),
      ongoing: values.ongoing && !!values.validFrom,
    };
    try {
      const saved = editing
        ? (
            await memory.updateMemory({
              id: editing.id,
              ...fields,
              clearValidity: !values.validFrom,
            })
          ).memory
        : (
            await memory.createMemory({
              bankId: bank.id,
              ...fields,
              identityId: values.identityId || undefined,
            })
          ).memory;
      if (saved) {
        leaving.current = true;
        onSaved(saved);
      }
    } catch (e) {
      setError(message(e));
    }
  });
  const errors = form.formState.errors;
  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      aria-label="Memory"
      onSubmit={(event) => void submit(event)}
      onKeyDown={(event) => {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
          event.preventDefault();
          void submit();
        }
        // Menus inside the editor handle (and prevent) their own Escape.
        if (event.key === "Escape" && !event.defaultPrevented) cancel();
      }}
    >
      <header className="flex h-11 shrink-0 items-center gap-2 border-b px-4">
        <h2 className="min-w-0 flex-1 truncate text-sm font-semibold">
          {editing ? "Edit memory" : "New fact"}
        </h2>
        <Button type="button" variant="ghost" size="sm" onClick={cancel}>
          Cancel
        </Button>
        <Button
          type="submit"
          size="sm"
          className="gap-2"
          disabled={form.formState.isSubmitting}
          title={`Save (${saveKey()})`}
        >
          {form.formState.isSubmitting ? "Saving…" : editing ? "Save" : "Add fact"}
        </Button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {editing?.origin === MemoryOrigin.EXPLICIT && (
          <p className="mx-4 mt-3 rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground">
            Saving makes this memory yours: agents still recall it but can no longer change it.
          </p>
        )}
        <div className="space-y-3 px-4 pt-4">
          <label className="block">
            <span className="sr-only">Title</span>
            <input
              autoFocus
              aria-label="Title"
              aria-invalid={!!errors.title}
              placeholder="The fact in a few words"
              className="w-full bg-transparent text-lg font-semibold outline-none placeholder:text-muted-foreground/60 aria-invalid:placeholder:text-destructive/70"
              {...form.register("title", { validate: (value) => !!value.trim() })}
            />
          </label>
          <div className="grid grid-cols-[6rem_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-xs">
            <Label htmlFor="fact-kind" className="text-xs text-muted-foreground">
              Kind
            </Label>
            <NativeSelect id="fact-kind" className="h-7 text-xs" {...form.register("kind")}>
              {Object.entries(KINDS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </NativeSelect>
            <Label htmlFor="fact-path" className="text-xs text-muted-foreground">
              Folder
            </Label>
            <Input
              id="fact-path"
              list="fact-paths"
              className="h-7 font-mono text-xs"
              placeholder="team/conventions"
              {...form.register("path")}
            />
            <datalist id="fact-paths">
              {paths.map((p) => (
                <option key={p} value={p} />
              ))}
            </datalist>
            <Label htmlFor="fact-importance" className="text-xs text-muted-foreground">
              Importance
            </Label>
            <div className="flex items-center gap-2">
              <input
                id="fact-importance"
                type="range"
                min={0}
                max={1}
                step={0.05}
                className="h-1 flex-1 accent-foreground"
                title="Orders memories that match a search equally well"
                {...form.register("importance", { valueAsNumber: true, min: 0, max: 1 })}
              />
              <span className="w-8 text-right tabular-nums">{Number(importance).toFixed(2)}</span>
            </div>
            <Label htmlFor="fact-from" className="text-xs text-muted-foreground">
              Holds from
            </Label>
            <Input
              id="fact-from"
              type="datetime-local"
              className="h-7 text-xs"
              {...form.register("validFrom")}
            />
            <Label htmlFor="fact-to" className="text-xs text-muted-foreground">
              Until
            </Label>
            <div className="flex items-center gap-3">
              <Input
                id="fact-to"
                type="datetime-local"
                className="h-7 flex-1 text-xs"
                disabled={ongoing}
                {...form.register("validTo")}
              />
              <label className="flex shrink-0 items-center gap-1.5">
                <input type="checkbox" {...form.register("ongoing")} />
                Still holds
              </label>
            </div>
            {perPerson && !editing && (
              <>
                <Label htmlFor="fact-person" className="text-xs text-muted-foreground">
                  For
                </Label>
                <NativeSelect
                  id="fact-person"
                  className="h-7 text-xs"
                  {...form.register("identityId")}
                >
                  <option value="">Everyone</option>
                  {people.map((p) => (
                    <option key={p.identityId} value={p.identityId}>
                      {p.name || "Unnamed person"}
                    </option>
                  ))}
                </NativeSelect>
              </>
            )}
          </div>
          <p className="text-[11px] text-muted-foreground">
            With only a start the fact is a single moment; leave both empty for one that always
            holds.
          </p>
        </div>
        <div className="p-4">
          <Controller
            control={form.control}
            name="content"
            rules={{ validate: (value) => !!value.trim() }}
            render={({ field, fieldState }) => (
              <MarkdownEditor
                label="Content"
                value={field.value}
                onChange={field.onChange}
                onSave={() => void submit()}
                placeholder="What agents should remember. Type / for lists, tables and code…"
                minHeight="min-h-[220px]"
                contentClassName="wiki-prose"
                className={fieldState.invalid ? "border-destructive" : ""}
              />
            )}
          />
        </div>
        {error && (
          <p role="alert" className="px-4 pb-4 text-sm text-destructive">
            {error}
          </p>
        )}
      </div>
    </form>
  );
}

function BankSettings({
  bank,
  registry,
  onSaved,
}: {
  bank: MemoryBank;
  registry: Map<string, Agent>;
  onSaved: (bank: MemoryBank) => void;
}) {
  const navigate = useNavigate();
  const form = useForm({ defaultValues: { name: bank.name, description: bank.description } });
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [busy, setBusy] = useState(false);
  async function remove() {
    setBusy(true);
    try {
      await memory.deleteMemoryBank({ id: bank.id });
      void navigate({ to: "/registry/memory" });
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
        <h2 className="text-lg font-semibold">{bank.name}</h2>
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <Badge variant="outline">{SCOPES[scopeOf(bank.scope)].label}</Badge>
          <span className="text-muted-foreground">
            Owner:{" "}
            <OwnerLink agentId={bank.agentId} name={registry.get(bank.agentId ?? "")?.name} />
          </span>
        </div>
        <p className="max-w-2xl text-xs text-muted-foreground">
          {SCOPES[scopeOf(bank.scope)].description}
        </p>
      </section>
      <form
        className="grid max-w-2xl gap-4"
        aria-label="Memory bank settings"
        onSubmit={form.handleSubmit(async (values) => {
          setError("");
          try {
            const { bank: next } = await memory.updateMemoryBank({
              id: bank.id,
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
          <Label htmlFor="bank-settings-name">Name</Label>
          <Input
            id="bank-settings-name"
            aria-invalid={!!form.formState.errors.name}
            {...form.register("name", { validate: (value) => !!value.trim() })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="bank-settings-description">Description</Label>
          <Textarea id="bank-settings-description" {...form.register("description")} />
          <p className="text-xs text-muted-foreground">
            Agents see it in their memory tools, so say what belongs here.
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
      <BankAgents bank={bank} registry={registry} onChanged={onSaved} />
      <BankSynthesis bank={bank} registry={registry} />
      <section className="grid max-w-2xl gap-3 border-t pt-6" aria-label="Delete memory bank">
        <h3 className="text-sm font-medium">Delete memory bank</h3>
        <p className="text-xs text-muted-foreground">
          Its memories are deleted and linked agents lose it.
        </p>
        <div>
          <Button variant="destructive" disabled={busy} onClick={() => setDeleting(true)}>
            Delete memory bank
          </Button>
        </div>
      </section>
      <AlertDialog open={deleting} onOpenChange={(next) => !busy && setDeleting(next)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {bank.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Its {bank.memoryCount} memories are deleted and linked agents lose it.
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

/** The agents linked to the bank, and how. */
function BankAgents({
  bank,
  registry,
  onChanged,
}: {
  bank: MemoryBank;
  registry: Map<string, Agent>;
  onChanged: (bank: MemoryBank) => void;
}) {
  let editable = true;
  const [error, setError] = useState("");
  const change = (write: () => Promise<unknown>) =>
    write()
      .then(() => memory.getMemoryBank({ id: bank.id }))
      .then(({ bank }) => bank && onChanged(bank))
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
        links={bank.links}
        own={bank.agentId}
        registry={registry}
        editable={editable}
        hint={
          bank.agentId
            ? "This is an agent's own bank; no other agent can be linked to it."
            : "Agents recall from the banks they are linked to, and write to those they may write to."
        }
        unlinkDescription="It no longer recalls from or writes to this bank."
        onSet={(agentId, access) =>
          change(() => memory.setAgentMemoryBank({ agentId, bankId: bank.id, access }))
        }
        onUnlink={(agentId) =>
          change(() => memory.removeAgentMemoryBank({ agentId, bankId: bank.id }))
        }
      />
    </>
  );
}
