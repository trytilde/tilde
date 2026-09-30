import { useCallback, useEffect, useState } from "react";
import { ChevronLeftIcon, ChevronRightIcon, PlusIcon, SearchIcon, XIcon } from "lucide-react";
import { AgentHealthStatus, type Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { useCursorPage, type FetchPage } from "@/hooks/use-cursor-page";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { DataTable } from "@/components/data-table";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Skeleton } from "@/components/ui/skeleton";

const healthFilters = {
  healthy: { label: "Healthy", status: AgentHealthStatus.HEALTHY },
  degraded: { label: "Degraded", status: AgentHealthStatus.DEGRADED },
  unhealthy: { label: "Unhealthy", status: AgentHealthStatus.UNHEALTHY },
  unknown: { label: "Unknown", status: AgentHealthStatus.UNKNOWN },
} as const;
const stateLabels = { all: "All states", active: "Active", paused: "Paused" } as const;

/** Registry filters as kept in the home page URL; the server applies them before paging. */
export type RegistrySearch = {
  q?: string;
  health?: keyof typeof healthFilters;
  state?: "active" | "paused";
};
export function parseRegistrySearch(raw: Record<string, unknown>): RegistrySearch {
  const search: RegistrySearch = {};
  if (typeof raw.q === "string" && raw.q.trim()) search.q = raw.q.slice(0, 200);
  if (typeof raw.health === "string" && raw.health in healthFilters)
    search.health = raw.health as RegistrySearch["health"];
  if (raw.state === "active" || raw.state === "paused") search.state = raw.state;
  return search;
}

export function AgentRegistry({
  onOpen,
  onCreate,
  search = {},
  onSearch = () => {},
}: {
  onOpen: (agent: Agent) => void;
  onCreate: () => void;
  search?: RegistrySearch;
  onSearch?: (patch: RegistrySearch) => void;
}) {
  const { q = "", health, state } = search;
  const fetchPage = useCallback<FetchPage<Agent>>(
    async (request, signal) => {
      const response = await agents.listAgents(
        {
          ...request,
          search: q,
          health: health ? healthFilters[health].status : AgentHealthStatus.UNSPECIFIED,
          paused: state ? state === "paused" : undefined,
        },
        { signal },
      );
      return { items: response.agents, nextPageToken: response.nextPageToken };
    },
    [q, health, state],
  );
  const page = useCursorPage(fetchPage);
  const [query, setQuery] = useState(q);
  const settled = useDebouncedValue(query.trim(), 300);
  useEffect(() => {
    if (settled !== q) onSearch({ q: settled || undefined });
  }, [settled]);
  // Back/forward navigation changes the URL under the field.
  useEffect(() => {
    setQuery((current) => (current.trim() === q ? current : q));
  }, [q]);
  const filtered = !!(q || health || state);
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (!document.hidden && !page.loading) page.refresh();
    }, 30_000);
    return () => window.clearInterval(timer);
  }, [page.refresh, page.loading]);
  const [deleting, setDeleting] = useState<Agent | null>(null);
  const [busy, setBusy] = useState(false);
  const [deleteError, setDeleteError] = useState("");
  const [notice, setNotice] = useState("");
  const [actionError, setActionError] = useState("");
  async function changePaused(agent: Agent, paused: boolean) {
    setBusy(true);
    setNotice("");
    setActionError("");
    try {
      if (paused) {
        await agents.pauseAgent({ id: agent.id });
        setNotice(
          "Agent paused. Cancellation requested for running invocations. Health checks continue.",
        );
      } else {
        await agents.resumeAgent({ id: agent.id });
        setNotice("Agent resumed.");
      }
      page.refresh();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Unable to update agent state.");
    } finally {
      setBusy(false);
    }
  }
  async function remove() {
    if (!deleting) return;
    setBusy(true);
    setDeleteError("");
    try {
      await agents.deleteAgent({ id: deleting.id });
      setDeleting(null);
      setNotice("Agent deleted.");
      page.reset();
    } catch (error) {
      setDeleteError(error instanceof Error ? error.message : "Unable to delete agent.");
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="flex flex-1 flex-col">
      <div className="trace-controls shrink-0 border-b bg-background">
        <div
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
          role="group"
          aria-label="Agent filter bar"
        >
          <Tabs
            value={health ?? "all"}
            onValueChange={(value) =>
              onSearch({
                health: value === "all" ? undefined : (value as RegistrySearch["health"]),
              })
            }
            className="shrink-0 gap-0"
          >
            <TabsList aria-label="Health filters" className="h-[26px] gap-0.5 rounded-md p-0.5">
              {[
                ["all", "All"] as const,
                ...Object.entries(healthFilters).map(([id, filter]) => [id, filter.label] as const),
              ].map(([id, label]) => (
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
          <form
            role="search"
            aria-label="Search agents"
            className="relative h-[26px] min-w-40 flex-1"
            onSubmit={(event) => {
              event.preventDefault();
              onSearch({ q: query.trim() || undefined });
            }}
          >
            <SearchIcon
              aria-hidden
              className="pointer-events-none absolute top-1.5 left-1.5 size-3 text-muted-foreground"
            />
            <Input
              aria-label="Search agents"
              placeholder="Search name or description…"
              value={query}
              maxLength={200}
              onChange={(event) => setQuery(event.target.value)}
              className="h-[26px] rounded-md py-0 pr-7 pl-6 text-[11px] shadow-none md:text-[11px]"
            />
            {query && (
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="absolute top-0 right-0.5 size-6"
                aria-label="Clear search"
                onClick={() => {
                  setQuery("");
                  onSearch({ q: undefined });
                }}
              >
                <XIcon className="size-3" />
              </Button>
            )}
          </form>
          <Select
            value={state ?? "all"}
            onValueChange={(value) =>
              onSearch({ state: value === "active" || value === "paused" ? value : undefined })
            }
            items={Object.entries(stateLabels).map(([value, label]) => ({ value, label }))}
          >
            <SelectTrigger
              size="sm"
              aria-label="Agent state"
              className="trace-controls ml-auto h-[26px] gap-2 rounded-none border-border bg-background px-2 text-[11px] shadow-none hover:bg-muted data-[size=sm]:rounded-none data-popup-open:bg-muted"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent
              className="trace-controls min-w-36 rounded-none border border-border bg-popover p-0 shadow-lg ring-0"
              alignItemWithTrigger={false}
              side="bottom"
              sideOffset={4}
              align="end"
            >
              {Object.entries(stateLabels).map(([value, label]) => (
                <SelectItem
                  key={value}
                  value={value}
                  className="h-7 rounded-none border-b border-border/50 pl-2 text-[11px] last:border-b-0 data-selected:bg-muted data-selected:font-semibold"
                >
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <section className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6">
        <div className="flex items-center justify-end gap-4">
          <Button onClick={onCreate}>
            <PlusIcon />
            Create agent
          </Button>
        </div>
        {page.error && (
          <div
            role="alert"
            className="flex items-center justify-between gap-3 rounded-lg border border-destructive/30 p-3 text-sm"
          >
            <span className="text-destructive">{page.error}</span>
            <Button variant="outline" onClick={page.retry} disabled={page.loading}>
              Retry
            </Button>
          </div>
        )}
        {actionError && (
          <p role="alert" className="text-sm text-destructive">
            {actionError}
          </p>
        )}
        {notice && (
          <p role="status" className="text-sm text-muted-foreground">
            {notice}
          </p>
        )}

        <DataTable
          data={page.items}
          loading={page.loading}
          loaded={page.loaded}
          onEdit={onOpen}
          busy={busy}
          empty={filtered ? "No agents match these filters." : undefined}
          onPause={(agent) => void changePaused(agent, true)}
          onResume={(agent) => void changePaused(agent, false)}
          onDelete={(agent) => {
            setDeleteError("");
            setDeleting(agent);
          }}
        />
        <div className="flex flex-wrap items-center justify-between gap-4 px-1">
          {page.loading ? (
            <Skeleton className="h-4 w-36" role="status" aria-label="Loading agents" />
          ) : (
            <p className="text-sm text-muted-foreground" role="status">
              {`${page.items.length} ${page.items.length === 1 ? "agent" : "agents"} on this page`}
            </p>
          )}
          <div className="flex flex-wrap items-center gap-6">
            <div className="flex items-center gap-2">
              <Label htmlFor="page-size" className="text-sm font-normal">
                Rows per page
              </Label>
              <Select
                value={String(page.size)}
                onValueChange={(value) => {
                  if (value) page.resize(Number(value));
                }}
                disabled={page.loading}
                items={[10, 20, 50, 100].map((size) => ({
                  value: String(size),
                  label: String(size),
                }))}
              >
                <SelectTrigger id="page-size" className="w-20">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {[10, 20, 50, 100].map((size) => (
                    <SelectItem key={size} value={String(size)}>
                      {size}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <span className="text-sm tabular-nums">Page {page.index + 1}</span>
            <nav aria-label="Pagination" className="flex gap-2">
              <Button
                variant="outline"
                size="icon"
                aria-label="Previous page"
                disabled={page.loading || !page.history.length}
                onClick={page.previous}
              >
                <ChevronLeftIcon />
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label="Next page"
                disabled={page.loading || !page.nextToken}
                onClick={page.next}
              >
                <ChevronRightIcon />
              </Button>
            </nav>
          </div>
        </div>
        <AlertDialog
          open={!!deleting}
          onOpenChange={(open) => {
            if (!open && !busy) setDeleting(null);
          }}
        >
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Delete {deleting?.name}?</AlertDialogTitle>
              <AlertDialogDescription>
                This removes the agent from your registry and retires its deployments. Conversation
                history is retained.
              </AlertDialogDescription>
            </AlertDialogHeader>
            {deleteError && (
              <p role="alert" className="text-sm text-destructive">
                {deleteError}
              </p>
            )}
            <AlertDialogFooter>
              <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
              <Button variant="destructive" onClick={remove} disabled={busy}>
                {busy ? "Deleting…" : "Delete agent"}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </section>
    </div>
  );
}
