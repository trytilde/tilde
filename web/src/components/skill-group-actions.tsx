import { useState } from "react";
import { CableIcon, EllipsisIcon, PlusIcon, RefreshCwIcon, Trash2Icon } from "lucide-react";
import type { Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { SkillSourceKind, type SkillSource } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import { listSkillConnections, message } from "./skill-common";
import { RemoveButton } from "./remove-button";
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
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { SkeletonLines } from "@/components/table-skeleton";

/**
 * What a group's own page used to offer, on its row of the Skills table: syncing a catalog or
 * git group, linking it to connections and deleting it.
 */
export function SkillGroupActions({
  group,
  onChanged,
}: {
  group: SkillSource;
  onChanged: () => Promise<unknown>;
}) {
  const [dialog, setDialog] = useState<"connections" | "delete">();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function act(work: () => Promise<unknown>, fallback: string) {
    setBusy(true);
    setError("");
    try {
      await work();
      await onChanged();
      return true;
    } catch (e) {
      setError(message(e, fallback));
      return false;
    } finally {
      setBusy(false);
    }
  }
  return (
    <span className="flex items-center gap-1">
      {error && !dialog && (
        <span role="alert" className="text-xs text-destructive">
          {error}
        </span>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger
          disabled={busy}
          render={<Button variant="ghost" size="icon-sm" aria-label={`${group.name} actions`} />}
        >
          <EllipsisIcon />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-48">
          {group.kind !== SkillSourceKind.EDITOR && (
            <DropdownMenuItem
              onClick={() =>
                void act(
                  () => skills.syncSkillSource({ id: group.id }),
                  "Unable to sync the group.",
                )
              }
            >
              <RefreshCwIcon />
              Sync now
            </DropdownMenuItem>
          )}
          <DropdownMenuItem onClick={() => setDialog("connections")}>
            <CableIcon />
            Connections
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="text-destructive focus:text-destructive [&_svg]:text-destructive"
            onClick={() => setDialog("delete")}
          >
            <Trash2Icon />
            Delete group
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ConnectionsDialog
        group={group}
        open={dialog === "connections"}
        onOpenChange={(open) => setDialog(open ? "connections" : undefined)}
      />
      <AlertDialog
        open={dialog === "delete"}
        onOpenChange={(open) => !busy && setDialog(open ? "delete" : undefined)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {group.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Its skills and their version history are deleted, and every agent given them stops
              seeing them on its next invocation.
            </AlertDialogDescription>
          </AlertDialogHeader>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() =>
                void act(
                  () => skills.deleteSkillSource({ id: group.id }),
                  "Unable to delete the group.",
                ).then((done) => done && setDialog(undefined))
              }
            >
              Delete group
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </span>
  );
}

/** Agents given a connection's skills get every skill in the groups linked to it. */
function ConnectionsDialog({
  group,
  open,
  onOpenChange,
}: {
  group: SkillSource;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [linked, setLinked] = useState<string[]>();
  const [all, setAll] = useState<Connection[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function load() {
    const [response, visible] = await Promise.all([
      skills.getSkillSource({ id: group.id }),
      // Connection names are a nicety; the dialog still lists linked ids without them.
      listSkillConnections().catch(() => [] as Connection[]),
    ]);
    setLinked(response.connectionIds);
    setAll(visible);
  }
  async function act(work: () => Promise<unknown>, fallback: string) {
    setBusy(true);
    setError("");
    try {
      await work();
      await load();
    } catch (e) {
      setError(message(e, fallback));
    } finally {
      setBusy(false);
    }
  }
  const linkable = all.filter((c) => !linked?.includes(c.id));
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (next) {
          setLinked(undefined);
          setError("");
          void load().catch((e) => setError(message(e, "Unable to load the connections.")));
        }
        onOpenChange(next);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{group.name} connections</DialogTitle>
          <DialogDescription>
            Agents given a connection&apos;s skills get every skill in the groups linked to it.
          </DialogDescription>
        </DialogHeader>
        {error && (
          <p role="alert" className="m-0 text-sm text-destructive">
            {error}
          </p>
        )}
        {!linked ? (
          <SkeletonLines rows={2} label="Loading connections" />
        ) : (
          <>
            <ul aria-label="Linked connections" className="m-0 divide-y rounded-lg border p-0">
              {linked.map((connectionId) => {
                const name = all.find((c) => c.id === connectionId)?.name ?? connectionId;
                return (
                  <li key={connectionId} className="flex items-center gap-3 px-3 py-2 text-sm">
                    <CableIcon aria-hidden="true" className="size-4 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate font-medium">{name}</span>
                    <RemoveButton
                      label={`Unlink ${name}`}
                      title={`Unlink ${name}?`}
                      description="Agents that get skills through this connection stop seeing this group's skills. The connection and the group are kept."
                      confirmLabel="Unlink"
                      disabled={busy}
                      onConfirm={() =>
                        act(
                          () =>
                            skills.unlinkConnectionSkillSource({
                              connectionId,
                              sourceId: group.id,
                            }),
                          "Unable to unlink the connection.",
                        )
                      }
                    />
                  </li>
                );
              })}
              {!linked.length && (
                <li className="px-3 py-4 text-center text-sm text-muted-foreground">
                  No connections link this group.
                </li>
              )}
            </ul>
            <DropdownMenu>
              <DropdownMenuTrigger
                disabled={busy || !linkable.length}
                render={<Button variant="outline" size="sm" className="self-start" />}
              >
                <PlusIcon />
                Link connection
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" className="min-w-64">
                {linkable.map((connection) => (
                  <DropdownMenuItem
                    key={connection.id}
                    onClick={() =>
                      void act(
                        () =>
                          skills.linkConnectionSkillSource({
                            connectionId: connection.id,
                            sourceId: group.id,
                          }),
                        "Unable to link the connection.",
                      )
                    }
                  >
                    <CableIcon />
                    {connection.name}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
