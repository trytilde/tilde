import { useRef } from "react";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { LogRecord } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { TableHeader, TableHead, TableBody, TableRow, TableCell } from "@/components/ui/table";
import { date } from "../tracing/format";
import { TableSkeletonRows } from "@/components/table-skeleton";
const features = tableFeatures({});
const helper = createColumnHelper<typeof features, LogRecord>();
const columns = helper.columns([
  helper.accessor("timestamp", { header: "Time", cell: ({ row }) => date(row.original.timestamp) }),
  helper.accessor("severity", {
    header: "Level",
    cell: ({ row }) => {
      const { severity, severityNumber } = row.original;
      const level =
        severity ||
        (severityNumber >= 21
          ? "FATAL"
          : severityNumber >= 17
            ? "ERROR"
            : severityNumber >= 13
              ? "WARN"
              : severityNumber >= 9
                ? "INFO"
                : severityNumber >= 5
                  ? "DEBUG"
                  : severityNumber >= 1
                    ? "TRACE"
                    : "—");
      return (
        <span
          className={
            severityNumber >= 17
              ? "font-bold text-red-600 dark:text-red-400"
              : severityNumber >= 13
                ? "font-bold text-yellow-600 dark:text-yellow-400"
                : "text-muted-foreground"
          }
        >
          {level}
        </span>
      );
    },
  }),
  helper.accessor("body", {
    header: "Messages",
    cell: ({ row }) => (
      <span
        className={`block truncate ${row.original.severityNumber >= 17 ? "text-destructive" : ""}`}
      >
        {row.original.body || "—"}
      </span>
    ),
  }),
]);
export function LogTable({
  records,
  busy,
  error,
  hasMore,
  onMore,
  onSelect,
}: {
  records: LogRecord[];
  busy: boolean;
  error: string;
  hasMore: boolean;
  onMore: () => void;
  onSelect: (record: LogRecord) => void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const table = useTable({ features, columns, data: records, getRowId: (row) => row.id });
  const rows = table.getRowModel().rows;
  const virtual = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 28,
    paddingStart: 32,
    overscan: 8,
    initialRect: { width: 1000, height: 600 },
  });
  const items = virtual.getVirtualItems();
  function more() {
    const e = scroll.current;
    if (e && hasMore && !busy && e.scrollHeight - e.scrollTop - e.clientHeight < 224) onMore();
  }
  const top = items.length ? Math.max(0, items[0].start - 32) : 0;
  const bottom = items.length
    ? Math.max(0, virtual.getTotalSize() - items[items.length - 1].end)
    : 0;
  return (
    <div
      ref={scroll}
      role="region"
      aria-label="Log records scroll area"
      aria-busy={busy}
      className="min-h-0 flex-1 overflow-auto"
      tabIndex={0}
      onScroll={more}
      onWheel={(event) => {
        if (event.deltaY > 0) more();
      }}
    >
      <table
        className="tracing-table w-full table-fixed border-collapse text-[11px]"
        aria-label="Agent log records"
        aria-rowcount={hasMore ? -1 : records.length + 1}
      >
        <colgroup>
          <col style={{ width: 180 }} />
          <col style={{ width: 80 }} />
          <col />
        </colgroup>
        <TableHeader className="sticky top-0 z-10 bg-muted shadow-[inset_0_-1px_0_var(--border)]">
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id} className="h-8 hover:bg-transparent">
              {group.headers.map((header) => (
                <TableHead
                  key={header.id}
                  className="h-8 border-r border-border/60 px-2 py-0 text-[11px] font-semibold last:border-r-0"
                >
                  <FlexRender header={header} />
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {busy && !records.length && <TableSkeletonRows columns={3} label="Loading logs" />}
          {top > 0 && (
            <TableRow aria-hidden="true" className="border-0">
              <TableCell colSpan={3} style={{ height: top, padding: 0 }} />
            </TableRow>
          )}
          {items.map((item) => {
            const row = rows[item.index];
            return (
              <TableRow
                key={row.id}
                aria-rowindex={item.index + 2}
                tabIndex={0}
                aria-label={`Inspect log ${row.id}`}
                className="h-7 cursor-pointer border-b border-border/50 hover:bg-muted/60 focus-visible:bg-accent"
                onClick={() => onSelect(row.original)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect(row.original);
                  }
                }}
              >
                {row.getAllCells().map((cell) => (
                  <TableCell
                    key={cell.id}
                    className="h-7 overflow-hidden border-r border-border/60 px-2 py-0 text-[11px] whitespace-nowrap last:border-r-0"
                  >
                    <FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            );
          })}
          {bottom > 0 && (
            <TableRow aria-hidden="true" className="border-0">
              <TableCell colSpan={3} style={{ height: bottom, padding: 0 }} />
            </TableRow>
          )}
        </TableBody>
      </table>
      {!busy && !records.length && !error && (
        <p className="p-4 text-xs text-muted-foreground">No logs match these filters.</p>
      )}
      {busy && !!records.length && (
        <p role="status" className="px-3 py-2 text-[11px] text-muted-foreground">
          Loading more logs…
        </p>
      )}
    </div>
  );
}
