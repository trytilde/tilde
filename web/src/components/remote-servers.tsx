import { useCallback, useEffect, useMemo, useState } from "react";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import type { ToolHost } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { connections, toolHosts, tools } from "@/client";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { HealthHistory } from "./agent-health";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { SkillAgentsDialog } from "./skill-agents-dialog";
import { AddServers } from "./add-servers";
import { loadToolConnections, loadToolProviders, message } from "./tool-connections";
import {
  CatalogSearchField,
  ToolProviderDialogs,
  hostEntry,
  providerEntry,
  type CatalogProvider,
} from "./tool-catalog";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

/** A tool host, or an MCP server added by URL; both carry a 12-hour health history. */
type ServerRow = {
  id: string;
  kind: Kind;
  name: string;
  health?: ToolHost["healthHistory"];
  tools: number;
  authMethods: string[];
  /** Agents using a tool host directly; MCP servers are used through their accounts. */
  agentIds: string[];
};
type Kind = "mcp" | "host";
const KINDS = { all: "All", mcp: "MCP", host: "Tilde tool server" } as const;
const features = tableFeatures({});
const helper = createColumnHelper<typeof features, ServerRow>();

/**
 * Remote tool servers: customer-hosted backends and MCP servers added by URL, each serving any
 * number of tools. A row opens the catalog's detail dialog: a server that publishes a provider,
 * and every MCP server, is used through that provider's accounts; any other server is its own
 * single account.
 */
export function RemoteServersPage() {
  const [data, setData] = useState<ServerRow[]>([]);
  const [entries, setEntries] = useState<Map<string, CatalogProvider>>(new Map());
  const [openId, setOpenId] = useState<string | null>(null);
  const registry = useAgentRegistry();
  const [viewing, setViewing] = useState<ServerRow>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [kind, setKind] = useState<keyof typeof KINDS>("all");
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  // The server filters by type and name; a type the tab leaves out is not requested at all.
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const filter = search || undefined;
      const [hosts, servers, list] = await Promise.all([
        kind === "mcp"
          ? { toolHosts: [] }
          : toolHosts.listToolHosts({ search: filter }, { signal }),
        kind === "host"
          ? []
          : loadToolProviders({ source: ProviderSource.MCP_SERVER, search: filter }, signal).then(
              (catalog) => catalog.providers,
            ),
        loadToolConnections({}, signal),
      ]);
      // A host that publishes a provider is opened through that provider's accounts.
      const published = await Promise.all(
        hosts.toolHosts.flatMap((host) =>
          host.providerId ? [connections.getProvider({ id: host.providerId }, { signal })] : [],
        ),
      );
      const providers = published.flatMap(({ provider }) => (provider ? [provider] : []));
      const [discovered, health, used] = await Promise.all([
        Promise.all(
          servers.map((provider) =>
            tools.listProviderTools({ connectionId: "", providerId: provider.id }, { signal }),
          ),
        ),
        servers.length
          ? tools.listMcpServerHealth({ providerIds: servers.map((p) => p.id) }, { signal })
          : Promise.resolve({ servers: [] }),
        // ListToolSources takes one tool host at a time.
        Promise.all(
          hosts.toolHosts.map((host) =>
            tools.listToolSources({ toolHostId: host.id }, { signal }).then(
              // Blueprint-owned sources have no agent; this column lists agents.
              ({ sources }) => [...new Set(sources.filter((s) => s.agentId).map((s) => s.agentId))],
              () => [],
            ),
          ),
        ),
      ]);
      setData(
        [
          ...hosts.toolHosts.map((host, index) => ({
            id: host.id,
            kind: "host" as const,
            name: host.name,
            health: host.healthHistory,
            tools: host.tools.length,
            authMethods: host.authMethods,
            agentIds: used[index],
          })),
          ...servers.map((provider, index) => ({
            id: `provider:${provider.id}`,
            kind: "mcp" as const,
            name: provider.name,
            health: health.servers.find((server) => server.providerId === provider.id)
              ?.healthHistory,
            tools: discovered[index].tools.length,
            authMethods: provider.connectionTypes.map((type) => type.name),
            agentIds: [],
          })),
        ].sort((a, b) => a.name.localeCompare(b.name)),
      );
      setEntries(
        new Map([
          ...hosts.toolHosts.map((host) => [host.id, hostEntry(host, providers, list)] as const),
          ...servers.map(
            (provider) => [`provider:${provider.id}`, providerEntry(provider, list)] as const,
          ),
        ]),
      );
      setError("");
    },
    [kind, search],
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
  const columns = useMemo(
    () =>
      helper.columns([
        helper.accessor("name", {
          header: "Name",
          cell: ({ row }) => (
            // Rows match the registry's height, which its 36px avatars set.
            <span className="flex min-h-9 items-center font-medium">{row.original.name}</span>
          ),
        }),
        helper.display({
          id: "health",
          header: () => (
            <span title="UTC hourly buckets. Any failed check makes the hour red; gray means no data.">
              Health · 12 hours
            </span>
          ),
          cell: ({ row }) =>
            row.original.health ? (
              <HealthHistory hours={row.original.health} />
            ) : (
              <span className="text-muted-foreground">—</span>
            ),
        }),
        helper.accessor("tools", { header: "# of available tools" }),
        helper.accessor((server) => server.authMethods.join(", "), {
          id: "auth",
          header: "Supported auth methods",
          cell: ({ getValue }) => getValue() || <span className="text-muted-foreground">None</span>,
        }),
        helper.display({
          id: "agents",
          header: "Agents",
          cell: ({ row }) => (
            <AgentStack
              ids={row.original.agentIds}
              registry={registry}
              onOpen={() => setViewing(row.original)}
            />
          ),
        }),
      ]),
    [registry],
  );
  const open = openId ? entries.get(openId) : undefined;
  const table = useTable({ features, columns, data, getRowId: (row) => row.id });
  let empty =
    kind === "all"
      ? "No remote servers yet. Add one to serve your own tools."
      : kind === "mcp"
        ? "No MCP servers yet."
        : "No Tilde tool servers yet.";
  return (
    <div className="flex flex-col">
      <div
        className="trace-controls flex min-h-10 items-center gap-2 border-b bg-background px-3 py-1.5"
        role="group"
        aria-label="Remote server filter bar"
      >
        <Tabs
          value={kind}
          onValueChange={(value) => setKind(value as keyof typeof KINDS)}
          className="shrink-0 gap-0"
        >
          <TabsList aria-label="Server type" className="h-[26px] gap-0.5 rounded-md p-0.5">
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
        <CatalogSearchField label="Search remote servers" value={query} onChange={setQuery} />
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap gap-3">
          <AddServers
            existing={false}
            onChanged={() => void refresh().catch((e) => setError(message(e)))}
          />
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-lg border">
          <Table aria-label="Remote servers">
            <TableHeader className="bg-muted/50">
              {table.getHeaderGroups().map((group) => (
                <TableRow key={group.id}>
                  {group.headers.map((header) => (
                    <TableHead key={header.id}>
                      <FlexRender {...header.getContext()} />
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {table.getRowModel().rows.length ? (
                table.getRowModel().rows.map((row) => (
                  <TableRow
                    key={row.id}
                    className="cursor-pointer"
                    onClick={() => setOpenId(row.original.id)}
                  >
                    {row.getAllCells().map((cell) => (
                      <TableCell key={cell.id}>
                        <FlexRender {...cell.getContext()} />
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell
                    colSpan={columns.length}
                    className="h-24 text-center text-muted-foreground"
                  >
                    {loading
                      ? "Loading remote servers…"
                      : error
                        ? "Unable to load remote servers."
                        : search
                          ? "No remote servers match this search."
                          : empty}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
        <SkillAgentsDialog
          open={!!viewing}
          onOpenChange={(next) => !next && setViewing(undefined)}
          title={viewing?.name ?? ""}
          agents={(viewing?.agentIds ?? []).map((id) => ({ id }))}
          registry={registry}
          tab="tools"
        />
        {open ? (
          <ToolProviderDialogs
            entry={open}
            key={open.id}
            refresh={() => refresh()}
            onClose={() => setOpenId(null)}
          />
        ) : null}
      </div>
    </div>
  );
}
