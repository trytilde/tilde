import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate } from "@tanstack/react-router";
import { PlusIcon, Trash2Icon } from "lucide-react";
import { GroupSource, type Group } from "@trytilde/contracts/tilde/management/v1/iam_pb.js";
import { iam } from "@/client";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const message = (cause: unknown, fallback: string) =>
  cause instanceof Error ? cause.message : fallback;
type CreateForm = { name: string };
/** Group IDs are derived from the name: lowercase kebab-case, as the server requires. */
export function slugify(name: string) {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 150)
    .replace(/-+$/, "");
}

/** Administrators manage groups here; agents are shared with groups from their Share button. */
export function GroupsPage() {
  const navigate = useNavigate();
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [deleting, setDeleting] = useState<Group | null>(null);
  const form = useForm<CreateForm>({ defaultValues: { name: "" } });
  const load = useCallback(async () => {
    try {
      setGroups((await iam.listGroups({ pageSize: 100 })).groups);
      setError("");
    } catch (cause) {
      setError(message(cause, "Unable to load groups."));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => void load(), [load]);
  // A new group is empty, so creation lands on its page to start adding people.
  async function create(values: CreateForm) {
    setBusy(true);
    try {
      const { group } = await iam.createGroup({
        source: GroupSource.LOCAL,
        slug: slugify(values.name),
        name: values.name.trim(),
      });
      setCreating(false);
      form.reset();
      await navigate({ to: "/groups/$groupId", params: { groupId: group?.id ?? "" } });
    } catch (cause) {
      setError(message(cause, "Unable to create group."));
    } finally {
      setBusy(false);
    }
  }
  async function remove() {
    if (!deleting) return;
    setBusy(true);
    try {
      await iam.deleteGroup({ id: deleting.id });
      setDeleting(null);
      await load();
    } catch (cause) {
      setError(message(cause, "Unable to delete group."));
    } finally {
      setBusy(false);
    }
  }
  const openGroup = (group: Group) =>
    void navigate({ to: "/groups/$groupId", params: { groupId: group.id } });
  return (
    <section className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6">
      <div className="flex flex-wrap items-center justify-end gap-4">
        <Button onClick={() => setCreating(true)}>
          <PlusIcon />
          Create group
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border" aria-busy={loading}>
        <Table aria-label="Groups">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead className="px-4">Name</TableHead>
              <TableHead className="px-4 text-center">Members</TableHead>
              <TableHead className="px-4">
                <span className="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {groups.map((group) => (
              <TableRow
                key={group.id}
                tabIndex={0}
                aria-label={`Open ${group.name}`}
                className="cursor-pointer hover:bg-muted/50 focus-visible:bg-muted/50 focus-visible:outline focus-visible:outline-ring"
                onClick={() => openGroup(group)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    openGroup(group);
                  }
                }}
              >
                <TableCell className="px-4 font-medium">{group.name}</TableCell>
                <TableCell className="px-4 text-center">{String(group.memberCount)}</TableCell>
                <TableCell className="px-4 text-right">
                  {group.source !== GroupSource.SYSTEM && (
                    <Button
                      variant="ghost"
                      size="icon"
                      disabled={busy}
                      aria-label={`Delete ${group.name}`}
                      onClick={(event) => {
                        event.stopPropagation();
                        setDeleting(group);
                      }}
                    >
                      <Trash2Icon />
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
            {!groups.length && (
              <TableRow>
                <TableCell colSpan={3} className="h-24 text-center text-muted-foreground">
                  {loading ? "Loading groups…" : "No groups."}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <Dialog open={creating} onOpenChange={(next) => !busy && setCreating(next)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create group</DialogTitle>
            <DialogDescription>
              A local group you manage here. Identity provider groups appear on their own when
              members sign in.
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={form.handleSubmit(create)} className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="group-name">Name</Label>
              <Input
                id="group-name"
                maxLength={100}
                {...form.register("name", {
                  required: true,
                  validate: (value) => slugify(value).length > 0 || "Use letters or digits.",
                })}
              />
              {form.formState.errors.name?.message && (
                <p role="alert" className="text-destructive">
                  {form.formState.errors.name.message}
                </p>
              )}
            </div>
            <DialogFooter>
              <Button type="submit" disabled={busy}>
                Create group
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
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
              Members lose every role the group holds on agents. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void remove()}>
              {busy ? "Deleting…" : "Delete group"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
