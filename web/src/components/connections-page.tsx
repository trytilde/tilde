import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { Fragment, useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  EllipsisIcon,
  ExternalLinkIcon,
  LayoutGridIcon,
  UnplugIcon,
  ZapIcon,
} from "lucide-react";
import type { Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, toolHosts, tools } from "@/client";
import { FilterSearchInput } from "@/features/observability/filter-search-input";
import { quote, tokens, unquote } from "@/features/observability/query";
import { AddServers } from "./add-servers";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { ProviderIcon } from "./provider-icon";
import { Pill } from "./skill-common";
import { SkillAgentsDialog } from "./skill-agents-dialog";
import {
  isMcpServer,
  loadToolConnections,
  loadToolProviders,
  message,
  statusLabel,
} from "./tool-connections";
import { Button } from "./ui/button";
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
type Kind = "catalog" | "mcp" | "host";
const KINDS = { all: "All", catalog: "Catalog", mcp: "MCP", host: "Tilde tool server" } as const;
/** The server-side filter each type tab asks for. */
const SOURCES = {
  all: undefined,
  catalog: ProviderSource.CATALOG,
  mcp: ProviderSource.MCP_SERVER,
  host: ProviderSource.TOOL_HOST,
} as const;
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
 * The tool connections in use, grouped by the provider they connect to, as the Skills page
 * groups skills. Each row carries the agents using it; new connections come
 * from the Tilde Catalog or a remote server.
 */
export function ConnectionsPage() {
  const navigate = useNavigate();
  let empty = "No connections yet. Add one from the Tilde Catalog or a remote server.";
  const [groups, setGroups] = useState<Group[]>([]);
  // Agents using each connection, by connection ID.
  const [users, setUsers] = useState<Map<string, string[]>>(() => new Map());
  const registry = useAgentRegistry();
  const [words, setWords] = useState<string[]>([]);
  const [kind, setKind] = useState<keyof typeof KINDS>("all");
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [viewing, setViewing] = useState<{ title: string; ids: string[] }>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // The server matches the search against connection, account and provider names.
  const search = words.join(" ");
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [catalog, list, hosts] = await Promise.all([
        loadToolProviders({}, signal),
        loadToolConnections({ search: search || undefined, source: SOURCES[kind] }, signal),
        toolHosts.listToolHosts({ withProvider: true }, { signal }),
      ]);
      if (signal?.aborted) return;
      const providers = new Map(catalog.providers.map((p) => [p.id, p]));
      const servers = new Map(
        hosts.toolHosts.flatMap((h) => (h.providerId ? [[h.providerId, h]] : [])),
      );
      const byProvider = new Map<string, Group>();
      for (const connection of list) {
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
      // ListToolSources takes one connection at a time, so this asks once per listed connection.
      const used = await Promise.all(
        list.map((connection) =>
          tools.listToolSources({ connectionId: connection.id }, { signal }).then(
            ({ sources }): [string, string[]] => [
              connection.id,
              [...new Set(sources.map((s) => s.agentId))],
            ],
            (): [string, string[]] => [connection.id, []],
          ),
        ),
      );
      if (!signal?.aborted) setUsers(new Map(used));
    },
    [search, kind],
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
  const shown = groups;
  const filtered = kind !== "all" || words.length > 0;
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
          <span aria-hidden="true" className="mx-1 h-5 shrink-0 border-l" />
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
            <Pill
              icon={<LayoutGridIcon />}
              label="Tilde Catalog"
              onClick={() => void navigate({ to: "/tools/catalog" })}
            />
            <AddServers existing onChanged={reload} />
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
                                to="/tools/$toolId"
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
                                  to: "/tools/$toolId",
                                  params: { toolId: connection.id },
                                  search: { kind: "connection" },
                                })
                              }
                            >
                              <TableCell className="pl-14">
                                <Link
                                  to="/tools/$toolId"
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
                          : empty}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
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
      <DropdownMenu
      >
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
                to: "/tools/$toolId",
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
