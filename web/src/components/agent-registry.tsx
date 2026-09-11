import { useEffect, useState } from "react";
import { ChevronLeftIcon, ChevronRightIcon, PlusIcon } from "lucide-react";
import type { Agent } from "@/gen/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { useCursorPage, type FetchPage } from "@/hooks/use-cursor-page";
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

const fetchPage: FetchPage<Agent> = async (request, signal) => {
  const response = await agents.listAgents(request, { signal });
  return { items: response.agents, nextPageToken: response.nextPageToken };
};
export function AgentRegistry({
  onOpen,
  onCreate,
}: {
  onOpen: (agent: Agent) => void;
  onCreate: () => void;
}) {
  const page = useCursorPage(fetchPage);
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
  const [retryStop, setRetryStop] = useState<Agent | null>(null);
  const [actionError, setActionError] = useState("");
  async function changePaused(agent: Agent, paused: boolean) {
    setBusy(true);
    setNotice("");
    setActionError("");
    try {
      setRetryStop(null);
      if (paused) {
        const response = await agents.pauseAgent({ id: agent.id });
        if (!response.stopAcknowledged) setRetryStop(agent);
        setNotice(
          response.stopAcknowledged
            ? "Agent paused. Health checks continue."
            : "Agent paused. Health checks continue, but the host did not acknowledge Stop. Retry pause to request cancellation again.",
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
      if (retryStop?.id === deleting.id) setRetryStop(null);
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
      {retryStop && (
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => void changePaused(retryStop, true)}
        >
          Retry stop
        </Button>
      )}
      <DataTable
        data={page.items}
        loading={page.loading}
        loaded={page.loaded}
        onEdit={onOpen}
        busy={busy}
        onPause={(agent) => void changePaused(agent, true)}
        onResume={(agent) => void changePaused(agent, false)}
        onDelete={(agent) => {
          setDeleteError("");
          setDeleting(agent);
        }}
      />
      <div className="flex flex-wrap items-center justify-between gap-4 px-1">
        <p className="text-sm text-muted-foreground" role="status">
          {page.loading
            ? "Loading…"
            : `${page.items.length} ${page.items.length === 1 ? "agent" : "agents"} on this page`}
        </p>
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
              This removes the agent from your registry and deletes its stored signing key.
              Conversation history is retained.
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
  );
}
