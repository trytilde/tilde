import { useMemo, useState } from "react";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import { ChevronLeftIcon, ChevronRightIcon, PlusIcon } from "lucide-react";
import { timestampDate } from "@bufbuild/protobuf/wkt";
import type { ApiKey } from "@trytilde/contracts/tilde/management/v1/api_keys_pb.js";
import type { Role } from "@trytilde/contracts/tilde/types/v1/authorization_pb.js";
import { AGENT_ROLES, reachesEveryAgent, roleLabels, roleSlug } from "@/lib/access";
import { apiKeys } from "@/client";
import { useCursorPage, type FetchPage } from "@/hooks/use-cursor-page";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/** One phrase per role: "Editor: all agents" or "Deployer: 3 agents". */
function summary(roles: Role[]) {
  return AGENT_ROLES.flatMap((slug) => {
    const held = roles.filter((role) => roleSlug(role.id) === slug);
    if (!held.length) return [];
    const reach = held.some((role) => reachesEveryAgent(role.id))
      ? "all agents"
      : `${held.length} agent${held.length === 1 ? "" : "s"}`;
    return [`${roleLabels[slug]}: ${reach}`];
  }).join(", ");
}
const features = tableFeatures({});
const helper = createColumnHelper<typeof features, ApiKey>();
const fetchPage: FetchPage<ApiKey> = async (request, signal) => {
  const response = await apiKeys.listApiKeys(request, { signal });
  return { items: response.apiKeys, nextPageToken: response.nextPageToken };
};
export function ApiKeysPage({ onCreate }: { onCreate?: () => void }) {
  const page = useCursorPage(fetchPage);
  const [revoking, setRevoking] = useState<ApiKey | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const columns = useMemo(
    () =>
      helper.columns([
        helper.accessor("name", {
          header: "Name",
          cell: ({ row }) => <span className="font-medium">{row.original.name}</span>,
        }),
        helper.accessor("prefix", {
          header: "Key",
          cell: ({ row }) => <code className="text-xs">{row.original.prefix}…</code>,
        }),
        helper.display({
          id: "access",
          header: "Access",
          cell: ({ row }) => summary(row.original.roles) || "None",
        }),
        helper.display({
          id: "created",
          header: "Created",
          cell: ({ row }) =>
            row.original.createdAt ? timestampDate(row.original.createdAt).toLocaleString() : "—",
        }),
        helper.display({
          id: "status",
          header: "Status",
          cell: ({ row }) =>
            row.original.revokedAt ? (
              <span
                className="text-muted-foreground"
                title={timestampDate(row.original.revokedAt).toLocaleString()}
              >
                Revoked
              </span>
            ) : (
              "Active"
            ),
        }),
        helper.display({
          id: "actions",
          header: () => <span className="sr-only">Actions</span>,
          cell: ({ row }) => (
            <div className="flex justify-end">
              {!row.original.revokedAt && (
                <Button
                  variant="ghost"
                  disabled={busy}
                  aria-label={`Revoke ${row.original.name}`}
                  onClick={() => {
                    setError("");
                    setRevoking(row.original);
                  }}
                >
                  Revoke
                </Button>
              )}
            </div>
          ),
        }),
      ]),
    [busy],
  );
  const table = useTable({ features, columns, data: page.items, getRowId: (row) => row.id });
  async function revoke() {
    if (!revoking) return;
    setBusy(true);
    setError("");
    try {
      await apiKeys.revokeApiKey({ id: revoking.id });
      setNotice(`Revoked ${revoking.name}.`);
      setRevoking(null);
      page.refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to revoke API key.");
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6">
      <div className="flex flex-wrap items-center justify-end gap-4">
        {onCreate && (
          <Button onClick={onCreate}>
            <PlusIcon />
            Create API key
          </Button>
        )}
      </div>
      {page.error && (
        <div
          role="alert"
          className="flex items-center justify-between gap-3 rounded-lg border border-destructive/30 p-3 text-sm"
        >
          <span>{page.error}</span>
          <Button variant="outline" onClick={page.retry} disabled={page.loading}>
            Retry
          </Button>
        </div>
      )}
      {notice && (
        <p role="status" className="text-sm text-muted-foreground">
          {notice}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border" aria-busy={page.loading}>
        <Table aria-label="API keys">
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
                <TableRow key={row.id}>
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
                  {page.loading
                    ? "Loading API keys…"
                    : page.loaded
                      ? "No API keys yet."
                      : "Unable to load API keys."}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-muted-foreground">{page.items.length} keys on this page</p>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="icon"
            aria-label="Previous page"
            disabled={page.loading || !page.history.length}
            onClick={page.previous}
          >
            <ChevronLeftIcon />
          </Button>
          <span className="text-sm">Page {page.index + 1}</span>
          <Button
            variant="outline"
            size="icon"
            aria-label="Next page"
            disabled={page.loading || !page.nextToken}
            onClick={page.next}
          >
            <ChevronRightIcon />
          </Button>
        </div>
      </div>
      <AlertDialog
        open={!!revoking}
        onOpenChange={(open) => {
          if (!open && !busy) {
            setRevoking(null);
            setError("");
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Revoke {revoking?.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Applications using this key will lose management access. This cannot be undone.
              Revocation can take up to 30 seconds across servers.
            </AlertDialogDescription>
          </AlertDialogHeader>
          {error && (
            <p role="alert" className="text-destructive">
              {error}
            </p>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void revoke()}>
              {busy ? "Revoking…" : "Revoke key"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
