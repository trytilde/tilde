import { Skeleton } from "@/components/ui/skeleton";
import { TableCell, TableRow } from "@/components/ui/table";

// Varied widths so placeholder rows read as data rather than a solid block.
const WIDTHS = ["w-3/4", "w-1/2", "w-2/3", "w-2/5"];

/** Placeholder rows for a table whose first page is loading; screen readers hear one status. */
export function TableSkeletonRows({
  columns,
  rows = 5,
  label = "Loading",
}: {
  columns: number;
  rows?: number;
  label?: string;
}) {
  return Array.from({ length: rows }, (_, row) => (
    <TableRow key={row}>
      {Array.from({ length: columns }, (_, column) => (
        <TableCell key={column}>
          {row === 0 && column === 0 && (
            <span role="status" className="sr-only">
              {label}
            </span>
          )}
          <Skeleton
            aria-hidden="true"
            className={`h-4 ${WIDTHS[(row + column) % WIDTHS.length]}`}
          />
        </TableCell>
      ))}
    </TableRow>
  ));
}

/** Placeholder lines for a list or block of content that is loading. */
export function SkeletonLines({ rows = 3, label = "Loading" }: { rows?: number; label?: string }) {
  return (
    <div role="status" aria-label={label} className="grid gap-2 py-2">
      {Array.from({ length: rows }, (_, row) => (
        <Skeleton key={row} className={`h-4 ${WIDTHS[row % WIDTHS.length]}`} />
      ))}
    </div>
  );
}
