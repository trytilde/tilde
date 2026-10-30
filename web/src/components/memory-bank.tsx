import { useCallback, useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { timestampDate, timestampFromDate, type Timestamp } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import { PencilIcon, PlusIcon } from "lucide-react";
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
import { KINDS, SCOPES, scopeOf } from "./memory";
import { LinkedAgentsTable, OwnerLink } from "./linked-agents";
import { BankSynthesis, SourceLink } from "./memory-synthesis";
import { RemoveButton } from "./remove-button";
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
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

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
        <TabsList aria-label="Memory bank">
          <TabsTrigger value="memories">Memories</TabsTrigger>
          <TabsTrigger value="settings">Settings</TabsTrigger>
        </TabsList>
      </DashboardNavigation>
      <TabsContent value="memories" className="flex min-h-0 flex-1 flex-col">
        {tab === "memories" && (
          <BankMemories bank={bank} registry={registry} onChanged={() => void reload()} />
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

/** Every path with memories, and its ancestors, with how many memories each holds in total. */
function tree(paths: MemoryPath[]) {
  const totals = new Map<string, number>();
  for (const { path, count } of paths) {
    const segments = path ? path.split("/") : [];
    for (let i = 1; i <= segments.length; i++) {
      const prefix = segments.slice(0, i).join("/");
      totals.set(prefix, (totals.get(prefix) ?? 0) + count);
    }
  }
  return [...totals.entries()].sort(([a], [b]) => a.localeCompare(b));
}

function BankMemories({
  bank,
  registry,
  onChanged,
}: {
  bank: MemoryBank;
  registry: Map<string, Agent>;
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
    void Promise.all([
      page("", abort.signal),
      memory.listMemoryPaths({ bankId: bank.id }, { signal: abort.signal }),
      perPerson
        ? memory.listMemoryPeople({ bankId: bank.id }, { signal: abort.signal })
        : Promise.resolve({ people: [] }),
    ])
      .then(([list, found, who]) => {
        setMemories(list.memories);
        setNext(list.nextPageToken);
        setPaths(found.paths);
        setPeople(who.people);
        setError("");
      })
      .catch((e) => !abort.signal.aborted && setError(message(e)))
      .finally(() => !abort.signal.aborted && setLoading(false));
    return () => abort.abort();
  }, [page, bank.id, perPerson, version]);
  const nodes = useMemo(() => tree(paths), [paths]);
  const total = paths.reduce((sum, p) => sum + p.count, 0);
  const personName = (id?: string) => people.find((p) => p.identityId === id)?.name || "A person";
  return (
    <div className="grid min-h-0 flex-1 lg:grid-cols-[14rem_minmax(0,1fr)_26rem]">
      <nav aria-label="Memory paths" className="overflow-y-auto border-r p-3 text-sm">
        <PathButton label="All memories" count={total} active={!path} onClick={() => setPath("")} />
        {nodes.map(([prefix, count]) => (
          <PathButton
            key={prefix}
            label={prefix.slice(prefix.lastIndexOf("/") + 1)}
            title={prefix}
            depth={prefix.split("/").length}
            count={count}
            active={path === prefix}
            onClick={() => setPath(prefix)}
          />
        ))}
      </nav>
      <section aria-label="Memories" className="flex min-h-0 flex-col border-r">
        <div
          className="trace-controls flex min-h-10 flex-wrap items-center gap-2 border-b bg-background px-3 py-1.5"
          role="group"
          aria-label="Memory filters"
        >
          <CatalogSearchField label="Search memories" value={query} onChange={setQuery} />
          <NativeSelect
            aria-label="Kind"
            className="w-auto"
            value={String(kind)}
            onChange={(event) => setKind(Number(event.target.value) as MemoryKind)}
          >
            <option value={MemoryKind.UNSPECIFIED}>Any kind</option>
            {Object.entries(KINDS).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </NativeSelect>
          <NativeSelect
            aria-label="Written by"
            className="w-auto"
            value={String(origin)}
            onChange={(event) => setOrigin(Number(event.target.value) as MemoryOrigin)}
          >
            <option value={MemoryOrigin.UNSPECIFIED}>Agents, people and synthesis</option>
            <option value={MemoryOrigin.EXPLICIT}>Written by agents</option>
            <option value={MemoryOrigin.OWNER}>Written by people</option>
            <option value={MemoryOrigin.SYNTHESIZED}>Synthesized</option>
          </NativeSelect>
          {perPerson && (
            <NativeSelect
              aria-label="Person"
              className="w-auto"
              value={person}
              onChange={(event) => setPerson(event.target.value)}
            >
              <option value="">Everyone</option>
              {people.map((p) => (
                <option key={p.identityId} value={p.identityId}>
                  {p.name || "Unnamed person"} ({p.count})
                </option>
              ))}
            </NativeSelect>
          )}
          <Button size="sm" className="ml-auto gap-2" onClick={() => setEditing(null)}>
            <PlusIcon />
            Add fact
          </Button>
        </div>
        {error && (
          <p role="alert" className="px-4 pt-3 text-sm text-destructive">
            {error}
          </p>
        )}
        <ul className="min-h-0 flex-1 divide-y overflow-y-auto" aria-label="Memory list">
          {memories.map((m) => (
            <li key={m.id}>
              <button
                type="button"
                aria-current={selected === m.id}
                className="w-full px-4 py-3 text-left hover:bg-muted/50 aria-[current=true]:bg-muted"
                onClick={() => setSelected(m.id)}
              >
                <span className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate text-sm font-medium">{m.title}</span>
                  {m.matched.map((how) => (
                    <Badge key={how} variant="secondary">
                      {how === MemoryMatch.MEANING ? "Meaning" : "Keywords"}
                    </Badge>
                  ))}
                </span>
                <span className="mt-1 line-clamp-2 block text-xs text-muted-foreground">
                  {m.content}
                </span>
                <span className="mt-1 flex flex-wrap gap-x-3 text-xs text-muted-foreground">
                  <span>{KINDS[m.kind as keyof typeof KINDS]}</span>
                  {m.path && <code>{m.path}</code>}
                  <span>
                    {m.origin === MemoryOrigin.OWNER
                      ? "Written by a person"
                      : m.origin === MemoryOrigin.SYNTHESIZED
                        ? "Synthesized"
                        : (registry.get(m.writtenByAgentId ?? "")?.name ?? "An agent")}
                  </span>
                  {m.identityId && <span>For {personName(m.identityId)}</span>}
                </span>
              </button>
            </li>
          ))}
          {!memories.length && (
            <li className="p-8 text-center text-sm text-muted-foreground">
              {loading
                ? "Loading memories…"
                : search || path || kind || origin || person
                  ? "No memories match."
                  : "No memories yet. Agents linked to this bank write them as they learn."}
            </li>
          )}
          {next && (
            <li className="p-3 text-center">
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
            </li>
          )}
        </ul>
      </section>
      <aside aria-label="Memory detail" className="min-h-0 overflow-y-auto p-4">
        {selected ? (
          <MemoryDetail
            key={`${selected}-${version}`}
            id={selected}
            registry={registry}
            personName={personName}
            editable={editable}
            onSelect={setSelected}
            onEdit={setEditing}
            onDeleted={() => {
              setSelected(undefined);
              changed();
            }}
          />
        ) : (
          <p className="text-sm text-muted-foreground">Select a memory to see it in full.</p>
        )}
      </aside>
      <FactDialog
        open={editing !== undefined}
        bank={bank}
        memory={editing ?? undefined}
        people={people}
        path={path}
        onClose={() => setEditing(undefined)}
        onSaved={(m) => {
          setEditing(undefined);
          setSelected(m.id);
          changed();
        }}
      />
    </div>
  );
}

function PathButton({
  label,
  title,
  depth = 0,
  count,
  active,
  onClick,
}: {
  label: string;
  title?: string;
  depth?: number;
  count: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-current={active}
      className="flex w-full items-center gap-2 rounded-md py-1 pr-2 text-left hover:bg-muted aria-[current=true]:bg-muted aria-[current=true]:font-medium"
      style={{ paddingLeft: `${0.5 + depth * 0.75}rem` }}
      onClick={onClick}
    >
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="text-xs tabular-nums text-muted-foreground">{count}</span>
    </button>
  );
}

const OPERATIONS: Record<MemoryOperation, string> = {
  [MemoryOperation.UNSPECIFIED]: "Changed",
  [MemoryOperation.CREATE]: "Written",
  [MemoryOperation.UPDATE]: "Edited",
  [MemoryOperation.SUPERSEDE]: "Replaced",
  [MemoryOperation.FORGET]: "Forgotten",
};

function MemoryDetail({
  id,
  registry,
  personName,
  editable,
  onSelect,
  onEdit,
  onDeleted,
}: {
  id: string;
  registry: Map<string, Agent>;
  personName: (id?: string) => string;
  editable: boolean;
  onSelect: (id: string) => void;
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
      <p role="alert" className="text-sm text-destructive">
        {error}
      </p>
    ) : null;
  const { memory: m, revisions } = found;
  const agentName = (agent?: string) => registry.get(agent ?? "")?.name ?? "An agent";
  const facts: [string, React.ReactNode][] = [
    ["Kind", KINDS[m.kind as keyof typeof KINDS]],
    ["Path", m.path ? <code>{m.path}</code> : "The bank's root"],
    ["Holds", validity(m) || "Always"],
    ["Importance", m.importance.toFixed(2)],
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
    ["Created", when(m.createdAt)],
  ];
  return (
    <article className="space-y-4" aria-label={m.title}>
      <div className="flex items-start gap-2">
        <h2 className="min-w-0 flex-1 text-base font-semibold">{m.title}</h2>
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
      </div>
      {m.supersededBy && (
        <p className="rounded-md bg-muted px-3 py-2 text-xs">
          Replaced by a newer memory, so agents no longer recall it.{" "}
          <button type="button" className="underline" onClick={() => onSelect(m.supersededBy!)}>
            Show the current one
          </button>
        </p>
      )}
      <p className="whitespace-pre-wrap text-sm">{m.content}</p>
      <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-xs">
        {facts.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-muted-foreground">{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
        {m.threadId && m.writtenByAgentId && (
          <div className="contents">
            <dt className="text-muted-foreground">Source</dt>
            <dd>
              <Link
                to="/agent/$agentId/sessions"
                params={{ agentId: m.writtenByAgentId }}
                search={{ session: m.threadId }}
                className="underline"
              >
                The conversation it came from
              </Link>
            </dd>
          </div>
        )}
        {m.evidence.length > 0 && (
          <div className="contents">
            <dt className="text-muted-foreground">Derived from</dt>
            <dd>
              <ul className="space-y-0.5">
                {m.evidence.map((e, index) => (
                  <li key={index}>
                    <SourceLink
                      source={e}
                      agentName={(id) => registry.get(id)?.name ?? "an agent"}
                    />
                  </li>
                ))}
              </ul>
            </dd>
          </div>
        )}
        {m.supersedesId && (
          <div className="contents">
            <dt className="text-muted-foreground">Replaces</dt>
            <dd>
              <button type="button" className="underline" onClick={() => onSelect(m.supersedesId!)}>
                The memory it replaced
              </button>
            </dd>
          </div>
        )}
      </dl>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <section aria-label="History" className="space-y-2 border-t pt-4">
        <h3 className="text-sm font-medium">History</h3>
        <ol className="space-y-2">
          {revisions.map((r, index) => (
            <li key={index} className="text-xs">
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
      </section>
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
 * A person's fact, new or edited. Agents recall it but never change it; editing an agent's
 * memory makes it the person's.
 */
function FactDialog({
  open,
  bank,
  memory: editing,
  people,
  path,
  onClose,
  onSaved,
}: {
  open: boolean;
  bank: MemoryBank;
  memory?: Memory;
  people: MemoryPerson[];
  path: string;
  onClose: () => void;
  onSaved: (memory: Memory) => void;
}) {
  const form = useForm<FactValues>({ defaultValues: factValues(editing, path) });
  const [error, setError] = useState("");
  useEffect(() => {
    if (open) form.reset(factValues(editing, path));
  }, [open, editing, path, form]);
  const ongoing = form.watch("ongoing");
  const perPerson = scopeOf(bank.scope) === MemoryBankScope.PER_PERSON;
  function close() {
    onClose();
    setError("");
  }
  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{editing ? "Edit memory" : "Add a fact"}</DialogTitle>
        </DialogHeader>
        {editing?.origin === MemoryOrigin.EXPLICIT && (
          <p className="text-xs text-muted-foreground">
            Saving makes this memory yours: agents still recall it but can no longer change it.
          </p>
        )}
        <form
          className="space-y-4"
          aria-label="Memory"
          onSubmit={form.handleSubmit(async (values) => {
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
              if (saved) onSaved(saved);
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="fact-title">Title</Label>
            <Input
              id="fact-title"
              placeholder="The fact in a few words"
              aria-invalid={!!form.formState.errors.title}
              {...form.register("title", { validate: (value) => !!value.trim() })}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="fact-content">Content</Label>
            <Textarea
              id="fact-content"
              rows={4}
              aria-invalid={!!form.formState.errors.content}
              {...form.register("content", { validate: (value) => !!value.trim() })}
            />
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="fact-kind">Kind</Label>
              <NativeSelect id="fact-kind" {...form.register("kind")}>
                {Object.entries(KINDS).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </NativeSelect>
            </div>
            <div className="space-y-2">
              <Label htmlFor="fact-path">Path</Label>
              <Input id="fact-path" placeholder="team/conventions" {...form.register("path")} />
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="fact-from">Holds from</Label>
              <Input id="fact-from" type="datetime-local" {...form.register("validFrom")} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="fact-to">Until</Label>
              <Input
                id="fact-to"
                type="datetime-local"
                disabled={ongoing}
                {...form.register("validTo")}
              />
            </div>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" {...form.register("ongoing")} />
            Still holds (no end yet)
          </label>
          <p className="text-xs text-muted-foreground">
            With only a start, the fact is a single moment. Leave both empty for a fact that always
            holds.
          </p>
          <div className="space-y-2">
            <Label htmlFor="fact-importance">Importance</Label>
            <Input
              id="fact-importance"
              type="number"
              min={0}
              max={1}
              step={0.1}
              {...form.register("importance", { valueAsNumber: true, min: 0, max: 1 })}
            />
            <p className="text-xs text-muted-foreground">
              Orders memories that match a search equally well; 0.5 by default.
            </p>
          </div>
          {perPerson && !editing && (
            <div className="space-y-2">
              <Label htmlFor="fact-person">For</Label>
              <NativeSelect id="fact-person" {...form.register("identityId")}>
                <option value="">Everyone</option>
                {people.map((p) => (
                  <option key={p.identityId} value={p.identityId}>
                    {p.name || "Unnamed person"}
                  </option>
                ))}
              </NativeSelect>
            </div>
          )}
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
              {editing ? "Save" : "Add fact"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
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
      void navigate({ to: "/memory" });
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
