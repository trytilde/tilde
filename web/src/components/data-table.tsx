import { AgentAvatar } from "./agent-avatar";
import { useMemo } from "react";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import { PauseIcon, PlayIcon, Trash2Icon } from "lucide-react";
import type { Agent } from "@/gen/tilde/types/v1/agent_pb.js";
import { HealthBadge, HealthHistory, responseTime } from "@/components/agent-health";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, Agent>();
export function DataTable({
  data,
  loading,
  loaded,
  onEdit,
  onDelete,
  onPause,
  onResume,
  busy,
}: {
  data: Agent[];
  loading: boolean;
  loaded: boolean;
  onEdit: (agent: Agent) => void;
  onDelete: (agent: Agent) => void;
  onPause: (agent: Agent) => void;
  onResume: (agent: Agent) => void;
  busy: boolean;
}) {
  const columns = useMemo(
    () =>
      columnHelper.columns([
        columnHelper.accessor("name", {
          header: "Name",
          cell: ({ row }) => (
            <div className="flex items-center gap-2">
              <AgentAvatar agent={row.original} animated />
              <span className="font-medium">{row.original.name}</span>
              {row.original.paused && (
                <span className="rounded bg-muted px-2 py-0.5 text-xs">Paused</span>
              )}
              <HealthBadge metrics={row.original.metrics} />
            </div>
          ),
        }),
        columnHelper.display({
          id: "health",
          header: () => (
            <span title="UTC hourly buckets. Any failed check makes the hour red; gray means no data.">
              Health · 12 hours
            </span>
          ),
          cell: ({ row }) => <HealthHistory metrics={row.original.metrics} />,
        }),
        columnHelper.display({
          id: "threads",
          header: () => (
            <span title="Threads this agent participates in, including threads without a reply yet.">
              Threads
            </span>
          ),
          cell: ({ row }) => (
            <span className="font-mono text-xs tabular-nums">
              {row.original.metrics?.threadCount.toLocaleString() ?? "—"}
            </span>
          ),
        }),
        columnHelper.display({
          id: "turns",
          header: () => (
            <span title="Completed non-empty agent replies divided by participating threads, across all time.">
              Avg. turns / thread
            </span>
          ),
          cell: ({ row }) => (
            <span className="font-mono text-xs tabular-nums">
              {row.original.metrics?.averageTurnsPerThread?.toLocaleString(undefined, {
                maximumFractionDigits: 1,
              }) ?? "—"}
            </span>
          ),
        }),
        columnHelper.display({
          id: "response",
          header: () => (
            <span title="Average invocation start to first non-empty visible reply. Only measured replies are included.">
              Avg. response time
            </span>
          ),
          cell: ({ row }) => (
            <span className="font-mono text-xs tabular-nums">
              {responseTime(row.original.metrics?.averageResponseMs)}
            </span>
          ),
        }),
        columnHelper.display({
          id: "actions",
          header: () => <span className="sr-only">Actions</span>,
          cell: ({ row }) => (
            <div className="flex justify-end gap-1">
              <Button
                variant="ghost"
                size="icon"
                aria-label={`${row.original.paused ? "Resume" : "Pause"} ${row.original.name}`}
                disabled={loading || busy}
                onClick={(event) => {
                  event.stopPropagation();
                  if (row.original.paused) onResume(row.original);
                  else onPause(row.original);
                }}
              >
                {row.original.paused ? <PlayIcon /> : <PauseIcon />}
              </Button>
              {row.original.paused && (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Delete ${row.original.name}`}
                  disabled={loading || busy}
                  onClick={(event) => {
                    event.stopPropagation();
                    onDelete(row.original);
                  }}
                >
                  <Trash2Icon />
                </Button>
              )}
            </div>
          ),
        }),
      ]),
    [loading, busy, onPause, onResume, onDelete],
  );
  // Pagination belongs to the server. Only the returned page is given to the table.
  const table = useTable({
    features,
    columns,
    data,
    getRowId: (row) => row.id,
  });
  return (
    <div className="overflow-hidden rounded-lg border" aria-busy={loading}>
      <Table aria-label="Agents">
        <TableHeader className="bg-muted/50">
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id}>
              {group.headers.map((header) => (
                <TableHead key={header.id} className="px-4">
                  {header.isPlaceholder ? null : <FlexRender header={header} />}
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
                tabIndex={0}
                aria-label={`Edit ${row.original.name}`}
                className="cursor-pointer hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:outline focus-visible:outline-ring"
                onClick={() => onEdit(row.original)}
                onKeyDown={(event) => {
                  if (
                    event.target === event.currentTarget &&
                    (event.key === "Enter" || event.key === " ")
                  ) {
                    event.preventDefault();
                    onEdit(row.original);
                  }
                }}
              >
                {row.getAllCells().map((cell) => (
                  <TableCell key={cell.id} className="h-14 px-4">
                    <FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className="h-44 text-center text-muted-foreground"
              >
                {loading
                  ? "Loading agents…"
                  : loaded
                    ? "No agents yet. Create your first agent to get started."
                    : "Unable to load agents."}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
