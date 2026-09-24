import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { PlusIcon, Trash2Icon, XIcon } from "lucide-react";
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import {
  GroupSource,
  type Group,
  type GroupMember,
  type IamUser,
} from "@trytilde/contracts/tilde/management/v1/iam_pb.js";
import { iam } from "@/client";
import { useCaller } from "@/hooks/use-caller";
import { useCursorPage, type FetchPage } from "@/hooks/use-cursor-page";
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
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
type Chip = { id: string; label: string };
function userLabel(user: IamUser) {
  return user.displayName ?? user.email ?? user.subject;
}

/** One group: its name, then its members with a way to add and remove them. */
export function GroupDetail({ groupId, onDeleted }: { groupId: string; onDeleted: () => void }) {
  const caller = useCaller();
  const [group, setGroup] = useState<Group>();
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [busy, setBusy] = useState(false);
  const fetchPage: FetchPage<GroupMember> = useCallback(
    async ({ pageToken, pageSize }, signal) => {
      const response = await iam.listGroupMembers({ groupId, pageToken, pageSize }, { signal });
      return { items: response.members, nextPageToken: response.nextPageToken };
    },
    [groupId],
  );
  const page = useCursorPage(fetchPage, 20);
  const loadGroup = useCallback(async () => {
    try {
      setGroup((await iam.getGroup({ id: groupId })).group);
      setError("");
    } catch (cause) {
      setError(message(cause, "Unable to load group."));
    }
  }, [groupId]);
  useEffect(() => void loadGroup(), [loadGroup]);
  // The provider owns external memberships, and every user is already in the all-users group.
  const managed =
    caller.admin &&
    !!group &&
    group.source !== GroupSource.EXTERNAL &&
    group.id !== "tilde_system:user";
  async function remove(member: GroupMember) {
    setBusy(true);
    setError("");
    try {
      await iam.removeGroupMember({ groupId, userId: member.userId });
      page.refresh();
      await loadGroup();
    } catch (cause) {
      setError(message(cause, "Unable to remove member."));
    } finally {
      setBusy(false);
    }
  }
  async function deleteGroup() {
    setBusy(true);
    try {
      await iam.deleteGroup({ id: groupId });
      onDeleted();
    } catch (cause) {
      setError(message(cause, "Unable to delete group."));
      setBusy(false);
    }
  }
  return (
    <section className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="grid gap-1">
          <h1 className="text-2xl font-semibold">{group?.name ?? "Loading group…"}</h1>
        </div>
        <div className="flex items-center gap-2">
          {managed && group.source !== GroupSource.SYSTEM && (
            <Button variant="outline" disabled={busy} onClick={() => setDeleting(true)}>
              Delete group
            </Button>
          )}
          {managed && (
            <Button disabled={busy} onClick={() => setAdding(true)}>
              <PlusIcon />
              Add users
            </Button>
          )}
        </div>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {group && !managed && (
        <p className="text-sm text-muted-foreground">
          {group.source === GroupSource.EXTERNAL
            ? "Membership is set by the identity provider at each sign-in."
            : "Membership of this group is not managed here."}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border" aria-busy={page.loading}>
        <Table aria-label="Members">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead className="px-4">User</TableHead>
              <TableHead className="px-4">
                <span className="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {page.items.map((member) => (
              <TableRow key={member.userId}>
                <TableCell className="h-14 px-4 font-medium">
                  {member.label || member.userId}
                </TableCell>
                <TableCell className="px-4 text-right">
                  {managed && (
                    <Button
                      variant="ghost"
                      size="icon"
                      disabled={busy}
                      aria-label={`Remove ${member.label || member.userId}`}
                      onClick={() => void remove(member)}
                    >
                      <Trash2Icon />
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
            {!page.items.length && (
              <TableRow>
                <TableCell colSpan={2} className="h-24 text-center text-muted-foreground">
                  {page.loading ? "Loading members…" : page.error || "No members yet."}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-muted-foreground">{page.items.length} members on this page</p>
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
        open={!!deleting}
        onOpenChange={(open) => {
          if (!open && !busy) setDeleting(false);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {group?.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Members lose every role the group holds on agents. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void deleteGroup()}>
              {busy ? "Deleting…" : "Delete group"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      {adding && group && (
        <AddUsers
          group={group}
          onClose={(changed) => {
            setAdding(false);
            if (changed) {
              page.refresh();
              void loadGroup();
            }
          }}
        />
      )}
    </section>
  );
}

/**
 * Picks users with a chip input: typing searches, Enter, Tab or a click adds the match as a
 * chip, Backspace on an empty input takes the last chip back.
 */
function AddUsers({ group, onClose }: { group: Group; onClose: (changed: boolean) => void }) {
  const [chips, setChips] = useState<Chip[]>([]);
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<Chip[]>([]);
  const [active, setActive] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const input = useRef<HTMLInputElement>(null);
  // The list hangs off the caret's line, so it moves as chips wrap the input to a new row.
  const [caret, setCaret] = useState({ left: 0, top: 0 });
  useLayoutEffect(() => {
    const element = input.current;
    if (element)
      setCaret({ left: element.offsetLeft, top: element.offsetTop + element.offsetHeight });
  }, [chips, matches.length]);
  useEffect(() => {
    const search = query.trim();
    if (!search) {
      setMatches([]);
      return;
    }
    const abort = new AbortController();
    const timer = setTimeout(() => {
      void iam
        .listUsers({ search, pageSize: 8 }, { signal: abort.signal })
        .then((response) => {
          if (abort.signal.aborted) return;
          const chosen = new Set(chips.map((chip) => chip.id));
          setMatches(
            response.users
              .filter((user) => !chosen.has(user.id))
              .map((user) => ({ id: user.id, label: userLabel(user) })),
          );
          setActive(0);
        })
        .catch(() => undefined);
    }, 150);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query, chips]);
  function add(chip: Chip) {
    setChips((current) => (current.some((c) => c.id === chip.id) ? current : [...current, chip]));
    setQuery("");
    setMatches([]);
    input.current?.focus();
  }
  function onKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if ((event.key === "Enter" || event.key === "Tab") && matches[active]) {
      event.preventDefault();
      add(matches[active]);
    } else if (event.key === "ArrowDown" && matches.length) {
      event.preventDefault();
      setActive((index) => (index + 1) % matches.length);
    } else if (event.key === "ArrowUp" && matches.length) {
      event.preventDefault();
      setActive((index) => (index - 1 + matches.length) % matches.length);
    } else if (event.key === "Backspace" && !query && chips.length) {
      setChips((current) => current.slice(0, -1));
    }
  }
  async function submit() {
    setBusy(true);
    setError("");
    try {
      for (const chip of chips) {
        await iam.addGroupMember({ groupId: group.id, userId: chip.id });
      }
      onClose(true);
    } catch (cause) {
      setError(message(cause, "Unable to add users."));
      setBusy(false);
    }
  }
  return (
    <Dialog open onOpenChange={(next) => !next && !busy && onClose(false)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add users to {group.name}</DialogTitle>
          <DialogDescription>
            Changes apply to signed-in members on their next request.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-2">
          <Label htmlFor="add-users">Users</Label>
          <div className="relative">
            <div
              className="flex min-h-24 flex-wrap content-start gap-1.5 rounded-md border bg-background p-2 text-sm focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/40"
              onClick={() => input.current?.focus()}
            >
              {chips.map((chip) => (
                <span
                  key={chip.id}
                  className="flex h-7 items-center gap-1 rounded-md bg-muted px-2 font-medium"
                >
                  <span className="max-w-48 truncate">{chip.label}</span>
                  <button
                    type="button"
                    aria-label={`Remove ${chip.label}`}
                    className="rounded-sm text-muted-foreground transition-colors hover:text-foreground"
                    disabled={busy}
                    onClick={() => setChips((current) => current.filter((c) => c.id !== chip.id))}
                  >
                    <XIcon className="size-3.5" />
                  </button>
                </span>
              ))}
              <input
                ref={input}
                id="add-users"
                role="combobox"
                aria-expanded={matches.length > 0}
                aria-controls="add-users-matches"
                aria-autocomplete="list"
                autoComplete="off"
                autoFocus
                disabled={busy}
                placeholder={chips.length ? "" : "Type a name or email"}
                value={query}
                className="h-7 min-w-32 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={onKeyDown}
              />
            </div>
            {matches.length > 0 && (
              <ul
                id="add-users-matches"
                role="listbox"
                aria-label="Matching users"
                style={{ left: caret.left, top: caret.top }}
                className="absolute z-10 mt-1 grid max-h-56 w-64 max-w-full gap-0.5 overflow-y-auto rounded-xl border bg-popover p-1.5 shadow-lg animate-in fade-in-0 zoom-in-95 duration-150"
              >
                {matches.map((match, index) => (
                  <li key={match.id} role="presentation">
                    <button
                      type="button"
                      role="option"
                      aria-selected={index === active}
                      className={`flex w-full items-center rounded-lg px-2.5 py-2 text-left text-sm transition-colors hover:bg-muted ${index === active ? "bg-muted" : ""}`}
                      onMouseEnter={() => setActive(index)}
                      onClick={() => add(match)}
                    >
                      {match.label}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={() => onClose(false)}>
            Cancel
          </Button>
          <Button disabled={busy || !chips.length} onClick={() => void submit()}>
            {busy
              ? "Adding…"
              : `Add ${chips.length || ""} ${chips.length === 1 ? "user" : "users"}`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
