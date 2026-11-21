import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  EllipsisIcon,
  ExternalLinkIcon,
  KeyRoundIcon,
  LayoutGridIcon,
  PlusIcon,
  UnplugIcon,
  ZapIcon,
} from "lucide-react";
import { connections, toolHosts, tools } from "@/client";
import { FilterSearchInput } from "@/features/observability/filter-search-input";
import { quote, tokens, unquote } from "@/features/observability/query";
import { AddServers } from "./add-servers";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { NewConnectionDialog, SANDBOX_PROVIDERS } from "./new-connection-dialog";
import { ProviderIcon } from "./provider-icon";
import { Pill } from "./skill-common";
import { SkillAgentsDialog } from "./skill-agents-dialog";
import { isMcpServer, message, statusLabel } from "./tool-connections";
import { Button, buttonVariants } from "./ui/button";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "./ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";

const COLUMNS = 4;
/**
 * What a connection is for, as the page's top tabs split them: sandbox providers, then
 * inference (and embedding) providers, then tools and chat, then anything else found (data
 * sources for ontologies, say).
 */
const TYPES = {
  tools: "Tools & chat",
  inference: "Inference & embedding",
  sandboxes: "Sandboxes",
  other: "Other",
} as const;
type Type = keyof typeof TYPES;
/** What the tabs with an add button of their own add: its label, dialog title and capability. */
const ADDS: Partial<Record<Type, { label: string; title: string; capability: Capability }>> = {
  inference: {
    label: "Add inference connection",
    title: "New inference connection",
    capability: Capability.INFERENCE,
  },
  sandboxes: {
    label: "Add sandbox connection",
    title: "New sandbox connection",
    capability: Capability.TOOL,
  },
};
function typeOf(connection: Connection): Type {
  if ((SANDBOX_PROVIDERS as readonly string[]).includes(connection.providerId)) return "sandboxes";
  if (connection.capabilities.includes(Capability.INFERENCE)) return "inference";
  if (
    connection.capabilities.includes(Capability.TOOL) ||
    connection.capabilities.includes(Capability.CHANNEL)
  )
    return "tools";
  return "other";
}
async function loadProviders(signal?: AbortSignal) {
  const providers: Provider[] = [];
  let pageToken = "";
  do {
    const page = await connections.listProviders({ pageSize: 100, pageToken }, { signal });
    providers.push(...page.providers);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return providers;
}
async function loadConnections(query: { search?: string }, signal?: AbortSignal) {
  const all: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections(
      { ...query, pageSize: 100, pageToken },
      { signal },
    );
    all.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return all;
}
type Kind = "catalog" | "mcp" | "host";
const KINDS = { all: "All", catalog: "Catalog", mcp: "MCP", host: "Tilde tool server" } as const;
/** A provider's connections: a catalog provider, an MCP server, or a remote server's provider. */
type Group = {
  id: string;
  name: string;
  kind: Kind;
  iconUrl?: string;
  /** The remote server publishing the provider, whose page manages it. */
  hostId?: string;
  connections: Connection[];
};

/** Free words, matched against connection, account and provider names. */
function parseQuery(query: string) {
  const scanned = tokens(query);
  if (scanned.unfinished)
    throw new Error('Close the quoted value and escape embedded quotes with \\".');
  return scanned.items.map(({ text }) => unquote(text).toLowerCase());
}

/**
 * Every connection, split by what it is for (tools and chat, inference, sandboxes, anything
 * else) and grouped by the provider it connects to, as the Skills page groups skills. Each row
 * carries the agents using it. New connections come from the Tilde Catalog or a remote server,
 * or for inference and sandboxes straight from their tab, with no agent needed first.
 */
export function ConnectionsPage() {
  const navigate = useNavigate();
  let empty = "No connections yet. Add one from the Tilde Catalog or a remote server.";
  const [groups, setGroups] = useState<Group[]>([]);
  // Agents using each connection, by connection ID.
  const [users, setUsers] = useState<Map<string, string[]>>(() => new Map());
  const registry = useAgentRegistry();
  const [words, setWords] = useState<string[]>([]);
  const [type, setType] = useState<Type>("tools");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [adding, setAdding] = useState(false);
  const [kind, setKind] = useState<keyof typeof KINDS>("all");
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [viewing, setViewing] = useState<{ title: string; ids: string[] }>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // The server matches the search against connection, account and provider names.
  const search = words.join(" ");
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [catalog, listed, hosts] = await Promise.all([
        loadProviders(signal),
        loadConnections({ search: search || undefined }, signal),
        toolHosts.listToolHosts({ withProvider: true }, { signal }),
      ]);
      if (signal?.aborted) return;
      // Every agent's own Tilde chat connection is managed on the agent, not here.
      const all = listed.filter((connection) => connection.providerId !== "tilde");
      setProviders(catalog);
      const providers = new Map(catalog.map((p) => [p.id, p]));
      const servers = new Map(
        hosts.toolHosts.flatMap((h) => (h.providerId ? [[h.providerId, h]] : [])),
      );
      const byProvider = new Map<string, Group>();
      for (const connection of all) {
        let group = byProvider.get(connection.providerId);
        if (!group) {
          const provider = providers.get(connection.providerId);
          const host = servers.get(connection.providerId);
          group = {
            id: connection.providerId,
            name: host?.name ?? provider?.name ?? connection.providerId,
            kind: host ? "host" : provider && isMcpServer(provider) ? "mcp" : "catalog",
            iconUrl: provider?.iconUrl,
            hostId: host?.id,
            connections: [],
          };
          byProvider.set(group.id, group);
        }
        group.connections.push(connection);
      }
      setGroups(
        [...byProvider.values()]
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((group) => ({
            ...group,
            connections: group.connections.sort((a, b) => a.name.localeCompare(b.name)),
          })),
      );
      // Agents assigned the connection (chat, inference) and, for a tool connection, those using
      // it as a tool source; ListToolSources takes one connection at a time.
      const used = await Promise.all(
        all.map(async (connection): Promise<[string, string[]]> => {
          const assigned = connection.associatedAgents.map((a) => a.id);
          if (!connection.capabilities.includes(Capability.TOOL))
            return [connection.id, [...new Set(assigned)]];
          const sources = await tools
            .listToolSources({ connectionId: connection.id }, { signal })
            .then(
              ({ sources }) => sources,
              () => [],
            );
          // Blueprint-owned sources have no agent; this column lists agents.
          const tooled = sources.filter((s) => s.agentId).map((s) => s.agentId);
          return [connection.id, [...new Set([...assigned, ...tooled])]];
        }),
      );
      if (!signal?.aborted) setUsers(new Map(used));
    },
    [search],
  );
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  const reload = () => void refresh().catch((e) => setError(message(e)));
  // Tabs only filter what is loaded: counts ignore the source tab, which narrows tools alone.
  const { shown, counts } = useMemo(() => {
    const counts = Object.fromEntries(Object.keys(TYPES).map((t) => [t, 0])) as Record<
      Type,
      number
    >;
    for (const group of groups)
      for (const connection of group.connections) counts[typeOf(connection)] += 1;
    const shown = groups
      .filter((group) => type !== "tools" || kind === "all" || group.kind === kind)
      .map((group) => ({
        ...group,
        connections: group.connections.filter((c) => typeOf(c) === type),
      }))
      .filter((group) => group.connections.length > 0);
    return { shown, counts };
  }, [groups, type, kind]);
  const filtered = (type === "tools" && kind !== "all") || words.length > 0;
  const shownTypes = (Object.keys(TYPES) as Type[]).filter((t) => t !== "other" || counts.other);
  // Inference and sandbox providers are added from their tab; tools come from the catalog.
  let addable =
    type === "inference"
      ? providers.filter((p) =>
          p.connectionTypes.some((t) => t.capabilities.includes(Capability.INFERENCE)),
        )
      : type === "sandboxes"
        ? SANDBOX_PROVIDERS.flatMap((id) => providers.filter((p) => p.id === id))
        : [];
  const add = ADDS[type];
  function toggle(id: string) {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  return (
    <div className="flex flex-col">
      <div className="relative z-20 shrink-0 border-b bg-background">
        <div
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
          role="group"
          aria-label="Connection filter bar"
        >
          <Tabs
            value={type}
            onValueChange={(value) => setType(value as Type)}
            className="shrink-0 gap-0"
          >
            <TabsList
              aria-label="What connections are for"
              className="h-[26px] gap-0.5 rounded-md p-0.5"
            >
              {shownTypes.map((id) => (
                <TabsTrigger
                  key={id}
                  value={id}
                  className="h-[22px] rounded px-2 text-xs font-normal data-active:font-bold"
                >
                  {TYPES[id]}
                  <span className="ml-1 tabular-nums text-muted-foreground">{counts[id]}</span>
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
          {type === "tools" && <span aria-hidden="true" className="mx-1 h-5 shrink-0 border-l" />}
          {type === "tools" && (
            <Tabs
              value={kind}
              onValueChange={(value) => setKind(value as keyof typeof KINDS)}
              className="shrink-0 gap-0"
            >
              <TabsList aria-label="Connection types" className="h-[26px] gap-0.5 rounded-md p-0.5">
                {Object.entries(KINDS).map(([id, label]) => (
                  <TabsTrigger
                    key={id}
                    value={id}
                    className="h-[22px] rounded px-2 text-xs font-normal data-active:font-bold"
                  >
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
          )}
          <span aria-hidden="true" className="mx-1 h-5 shrink-0 border-l" />
          <Link
            to="/connections/secrets"
            className={buttonVariants({
              variant: "ghost",
              size: "sm",
              className: "order-last ml-auto",
            })}
          >
            <KeyRoundIcon /> Secrets
          </Link>
          <FilterSearchInput
            label="Search connections"
            canonical={words.map(quote).join(" ")}
            disabled={loading}
            validate={parseQuery}
            suggest={() => []}
            onApply={(query) => setWords(parseQuery(query))}
          />
        </div>
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap gap-3">
          {type === "tools" ? (
            <>
              <Pill
                icon={<LayoutGridIcon />}
                label="Tilde Catalog"
                onClick={() => void navigate({ to: "/registry/tools/catalog" })}
              />
              <AddServers existing onChanged={reload} />
            </>
          ) : (
            add &&
            addable.length > 0 && (
              <Pill icon={<PlusIcon />} label={add.label} onClick={() => setAdding(true)} />
            )
          )}
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-xl border">
          <Table aria-label="Tool connections">
            <TableHeader className="bg-muted/50">
              <TableRow>
                <TableHead className="h-11 px-5">Name</TableHead>
                <TableHead>Account</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="w-72 px-5">Agents</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {shown.length ? (
                shown.map((group) => {
                  const expanded = !collapsed.has(group.id);
                  return (
                    <Fragment key={group.id}>
                      <TableRow
                        aria-label={`Provider ${group.name}`}
                        className="bg-muted/40 hover:bg-muted/40"
                      >
                        <TableCell colSpan={COLUMNS} className="px-3 py-2.5">
                          <div className="flex min-w-0 items-center gap-2">
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              aria-label={`${expanded ? "Collapse" : "Expand"} ${group.name}`}
                              aria-expanded={expanded}
                              onClick={() => toggle(group.id)}
                            >
                              {expanded ? <ChevronDownIcon /> : <ChevronRightIcon />}
                            </Button>
                            {group.kind === "host" && !group.iconUrl ? (
                              <ZapIcon
                                aria-hidden="true"
                                className="size-5 shrink-0 text-muted-foreground"
                              />
                            ) : (
                              <ProviderIcon iconUrl={group.iconUrl} />
                            )}
                            {group.hostId ? (
                              <Link
                                to="/registry/tools/$toolId"
                                params={{ toolId: group.hostId }}
                                search={{ kind: "host" }}
                                className="truncate text-sm font-semibold hover:underline"
                              >
                                {group.name}
                              </Link>
                            ) : (
                              <span className="truncate text-sm font-semibold">{group.name}</span>
                            )}
                            <span className="rounded-md border bg-background px-1.5 text-xs tabular-nums text-muted-foreground">
                              {group.connections.length}
                            </span>
                            <span className="rounded bg-muted px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
                              {KINDS[group.kind]}
                            </span>
                          </div>
                        </TableCell>
                      </TableRow>
                      {expanded &&
                        group.connections.map((connection) => {
                          const agentIds = users.get(connection.id) ?? [];
                          return (
                            <TableRow
                              key={connection.id}
                              className="cursor-pointer"
                              onClick={() =>
                                void navigate({
                                  to: "/registry/tools/$toolId",
                                  params: { toolId: connection.id },
                                  search: { kind: "connection" },
                                })
                              }
                            >
                              <TableCell className="pl-14">
                                <Link
                                  to="/registry/tools/$toolId"
                                  params={{ toolId: connection.id }}
                                  search={{ kind: "connection" }}
                                  className="font-medium hover:underline"
                                  onClick={(event) => event.stopPropagation()}
                                >
                                  {connection.name}
                                </Link>
                              </TableCell>
                              <TableCell className="text-muted-foreground">
                                {connection.accountLabel || "—"}
                              </TableCell>
                              <TableCell>
                                <span
                                  className={
                                    connection.status === "ready"
                                      ? "text-emerald-700 dark:text-emerald-400"
                                      : "text-muted-foreground"
                                  }
                                >
                                  {statusLabel(connection.status)}
                                </span>
                              </TableCell>
                              <TableCell className="px-5">
                                <div className="flex items-center justify-between gap-2">
                                  <AgentStack
                                    ids={agentIds}
                                    registry={registry}
                                    onOpen={() =>
                                      setViewing({ title: connection.name, ids: agentIds })
                                    }
                                  />
                                  {/* Dialogs portal out, but their clicks still bubble to the row. */}
                                  <span onClick={(event) => event.stopPropagation()}>
                                    <ConnectionActions connection={connection} onChanged={reload} />
                                  </span>
                                </div>
                              </TableCell>
                            </TableRow>
                          );
                        })}
                    </Fragment>
                  );
                })
              ) : (
                <TableRow>
                  <TableCell colSpan={COLUMNS} className="h-24 text-center text-muted-foreground">
                    {loading
                      ? "Loading connections…"
                      : error
                        ? "Unable to load connections."
                        : filtered
                          ? "No connections match your search."
                          : type === "tools"
                            ? empty
                            : `No ${TYPES[type].toLowerCase()} connections yet.`}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
        <NewConnectionDialog
          open={adding}
          title={add?.title ?? ""}
          providers={addable}
          capability={add?.capability ?? Capability.TOOL}
          onClose={() => setAdding(false)}
          onConnected={reload}
        />
        <SkillAgentsDialog
          open={!!viewing}
          onOpenChange={(open) => !open && setViewing(undefined)}
          title={viewing?.title ?? ""}
          agents={(viewing?.ids ?? []).map((id) => ({ id }))}
          registry={registry}
          tab="tools"
        />
      </div>
    </div>
  );
}

/**
 * Opening and disconnecting one connection, from its row.
 */
function ConnectionActions({
  connection,
  onChanged,
}: {
  connection: Connection;
  onChanged: () => void;
}) {
  const navigate = useNavigate();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function disconnect() {
    setBusy(true);
    setError("");
    try {
      await connections.disconnect({ id: connection.id });
      setConfirming(false);
      onChanged();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <span className="flex items-center gap-1">
      <DropdownMenu>
        <DropdownMenuTrigger
          disabled={busy}
          render={
            <Button variant="ghost" size="icon-sm" aria-label={`${connection.name} actions`} />
          }
        >
          <EllipsisIcon />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-48">
          <DropdownMenuItem
            onClick={() =>
              void navigate({
                to: "/registry/tools/$toolId",
                params: { toolId: connection.id },
                search: { kind: "connection" },
              })
            }
          >
            <ExternalLinkIcon />
            Open
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="text-destructive focus:text-destructive [&_svg]:text-destructive"
            onClick={() => setConfirming(true)}
          >
            <UnplugIcon />
            Disconnect
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirming} onOpenChange={(open) => !busy && setConfirming(open)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Disconnect {connection.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Its credentials are removed and its tools disappear from every agent using it.
            </AlertDialogDescription>
          </AlertDialogHeader>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void disconnect()}>
              Disconnect
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </span>
  );
}
