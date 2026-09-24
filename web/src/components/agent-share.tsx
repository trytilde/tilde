import { useCallback, useEffect, useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon, GlobeIcon, LockIcon, UsersIcon, XIcon } from "lucide-react";
import {
  PrincipalType,
  ResourceKind,
  type Principal,
  type Role,
  type RoleAssignment,
} from "@trytilde/contracts/tilde/types/v1/authorization_pb.js";
import { iam } from "@/client";
import { useCaller } from "@/hooks/use-caller";
import { EVERYONE_GROUP, assignable, roleId, roleSlug, type AgentRole } from "@/lib/access";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type Candidate = { principal: Principal; label: string; detail: string };
const typeLabels: Record<number, string> = {
  [PrincipalType.GROUP]: "Group",
  [PrincipalType.USER]: "User",
  [PrincipalType.API_KEY]: "API key",
};
function key(principal?: Principal) {
  return `${principal?.type ?? 0}:${principal?.id ?? ""}`;
}
function isEveryone(assignment: RoleAssignment) {
  return (
    assignment.principal?.type === PrincipalType.GROUP && assignment.principal.id === EVERYONE_GROUP
  );
}
/** Square initial tile, coloured per principal so rows are easy to tell apart. */
function Initial({ label, type }: { label: string; type?: PrincipalType }) {
  const tones = [
    "bg-violet-600",
    "bg-orange-500",
    "bg-rose-600",
    "bg-emerald-600",
    "bg-sky-600",
    "bg-amber-600",
  ];
  const tone =
    type === PrincipalType.GROUP
      ? "bg-foreground"
      : tones[label.split("").reduce((sum, c) => sum + c.charCodeAt(0), 0) % tones.length];
  return (
    <span
      aria-hidden
      className={`grid size-8 shrink-0 place-items-center rounded-md text-sm font-semibold text-white ${tone}`}
    >
      {label.trim().charAt(0).toUpperCase() || "?"}
    </span>
  );
}
function held(assignment: RoleAssignment): AgentRole[] {
  return assignment.roles.map((role) => roleSlug(role.id)).filter((r): r is AgentRole => !!r);
}
/** One person or group with access; the card is the anchor for its role menu. */
function AssignmentRow({
  assignment,
  roles,
  agentId,
  reach,
  editor,
  busy,
  you,
  run,
}: {
  assignment: RoleAssignment;
  roles: Role[];
  agentId: string;
  reach: AgentRole[];
  editor: boolean;
  busy: boolean;
  you: boolean;
  run: (change: () => Promise<unknown>) => Promise<void>;
}) {
  const card = useRef<HTMLLIElement>(null);
  const label = assignment.label || assignment.principal?.id || "";
  const current = held(assignment);
  // Someone holding a role beyond the caller's reach cannot be changed by them.
  const reachable = editor && current.every((role) => reach.includes(role));
  const summary = current.map((role) => roleLabel(roles, agentId, role)).join(", ");
  const principal = assignment.principal;
  return (
    <li
      ref={card}
      className="flex items-center gap-2.5 rounded-lg border bg-card px-2.5 py-1.5 transition-colors hover:border-foreground/20"
    >
      <Initial label={label} type={principal?.type} />
      <span className="grid min-w-0 flex-1">
        <span className="truncate text-sm font-medium">
          {label}
          {you && <span className="text-muted-foreground"> (you)</span>}
        </span>
        <span className="truncate text-xs text-muted-foreground">
          {typeLabels[principal?.type ?? 0]}
        </span>
      </span>
      {reachable ? (
        <RoleMenu
          label={`Access for ${label}`}
          summary={summary}
          roles={roles}
          agentId={agentId}
          held={current}
          reach={reach}
          disabled={busy}
          anchor={card}
          onToggle={(role, on) =>
            void run(() =>
              on
                ? iam.assignRole({ roleId: roleId(agentId, role), principal })
                : iam.revokeRole({ roleId: roleId(agentId, role), principal }),
            )
          }
          onRemove={() =>
            void run(() =>
              Promise.all(
                current.map((role) => iam.revokeRole({ roleId: roleId(agentId, role), principal })),
              ),
            )
          }
        />
      ) : (
        <span className="px-3 text-sm font-medium text-muted-foreground">{summary}</span>
      )}
    </li>
  );
}
function roleLabel(roles: Role[], agentId: string, role: AgentRole) {
  return roles.find((r) => r.id === roleId(agentId, role))?.name ?? role;
}
/**
 * The per-row role menu: every role the caller can hand out, with what it allows, ticked when
 * held. Several may be held at once; "Remove access" takes them all away. It floats beneath the
 * whole card rather than the trigger, never flips above it, and the page scrolls to reach it.
 */
function RoleMenu({
  label,
  summary,
  roles,
  agentId,
  held,
  reach,
  disabled,
  anchor,
  onToggle,
  onRemove,
}: {
  label: string;
  summary: string;
  roles: Role[];
  agentId: string;
  held: AgentRole[];
  reach: AgentRole[];
  disabled: boolean;
  /** The card the menu floats beneath, at the card's full width. */
  anchor: React.RefObject<HTMLElement | null>;
  onToggle: (role: AgentRole, on: boolean) => void;
  onRemove: () => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            aria-label={label}
            disabled={disabled}
            className={ghostTrigger}
          >
            <span className="max-w-40 truncate">{summary || "No access"}</span>
            <ChevronDownIcon className="size-4" />
          </Button>
        }
      />
      <DropdownMenuContent
        anchor={anchor}
        align="start"
        sideOffset={4}
        collisionAvoidance={{ side: "none" }}
        className="w-(--anchor-width) rounded-xl p-1.5 shadow-xl"
      >
        {reach.map((role) => {
          const on = held.includes(role);
          const detail = roles.find((r) => r.id === roleId(agentId, role));
          return (
            <DropdownMenuItem
              key={role}
              role="menuitemcheckbox"
              aria-checked={on}
              closeOnClick={false}
              className="items-start gap-3 rounded-lg px-3 py-2.5"
              onClick={() => onToggle(role, !on)}
            >
              <span className="grid min-w-0 flex-1 gap-0.5">
                <span className="text-sm font-medium">{detail?.name ?? role}</span>
                <span className="text-xs leading-snug text-muted-foreground">
                  {detail?.description}
                </span>
              </span>
              <CheckIcon
                aria-hidden
                className={`mt-0.5 size-4 shrink-0 ${on ? "text-foreground" : "invisible"}`}
              />
            </DropdownMenuItem>
          );
        })}
        <DropdownMenuSeparator />
        <DropdownMenuItem
          className="rounded-lg px-3 py-2.5 text-destructive data-highlighted:text-destructive"
          onClick={onRemove}
        >
          Remove access
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
/** Plain-text dropdown trigger with a chevron, as in the reference; focus draws a ring. */
const ghostTrigger =
  "h-8 gap-1.5 rounded-md border border-transparent bg-transparent px-3 text-sm font-medium shadow-none transition-colors hover:border-border hover:bg-muted/60 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40 data-[popup-open]:border-ring data-[popup-open]:ring-2 data-[popup-open]:ring-ring/40 [&_svg]:text-muted-foreground";

/**
 * Who can reach one agent. Anyone with edit invites users and groups and assigns the roles
 * they hold themselves (reader, editor, deployer, admin), and chooses whether every signed-in
 * user may view the agent. Viewers only see the list.
 */
export function AgentShare({
  agentId,
  variant = "outline",
  size = "icon-sm",
}: {
  agentId: string;
  variant?: "outline" | "ghost";
  size?: "icon" | "icon-sm";
}) {
  const caller = useCaller();
  const resource = { kind: ResourceKind.AGENT, id: agentId };
  const [open, setOpen] = useState(false);
  const [assignments, setAssignments] = useState<RoleAssignment[]>([]);
  const [roles, setRoles] = useState<Role[]>([]);
  const [mine, setMine] = useState<string[]>([]);
  // Roles the caller may hand out: share on the agent plus every action the role gives.
  const reach = assignable(mine);
  const editor = reach.length > 0;
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [chosen, setChosen] = useState<Candidate>();
  const load = useCallback(async () => {
    const [access, listed, available] = await Promise.all([
      iam.getAccess({ resource }),
      iam.listRoleAssignments({ resource }),
      iam.listRoles({ resource }),
    ]);
    setMine(access.actions);
    setAssignments(listed.assignments);
    setRoles(available.roles);
  }, [agentId]);
  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError("");
    load()
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : "Unable to load sharing."),
      )
      .finally(() => setLoading(false));
  }, [open, load]);
  // Search users and groups as the editor types; principals already listed are left out.
  useEffect(() => {
    if (!editor || chosen || !query.trim()) {
      setCandidates([]);
      return;
    }
    const abort = new AbortController();
    const timer = setTimeout(() => {
      const search = query.trim();
      void Promise.all([
        iam.listUsers({ search, pageSize: 6 }, { signal: abort.signal }),
        iam.listGroups({ search, pageSize: 6 }, { signal: abort.signal }),
      ])
        .then(([users, groups]) => {
          const listed = new Set(assignments.map((a) => key(a.principal)));
          const found: Candidate[] = [
            ...groups.groups
              .filter((group) => !group.id.startsWith("tilde_system:"))
              .map((group) => ({
                principal: { type: PrincipalType.GROUP, id: group.id } as Principal,
                label: group.name,
                detail: `Group · ${group.memberCount} members`,
              })),
            ...users.users.map((user) => ({
              principal: { type: PrincipalType.USER, id: user.id } as Principal,
              label: user.displayName ?? user.email ?? user.subject,
              detail: user.email ?? user.subject,
            })),
          ];
          if (!abort.signal.aborted)
            setCandidates(found.filter((c) => !listed.has(key(c.principal))));
        })
        .catch(() => undefined);
    }, 150);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query, editor, chosen, assignments]);
  async function run(change: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await change();
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to change sharing.");
    } finally {
      setBusy(false);
    }
  }
  const everyone = assignments.some(isEveryone);
  const people = assignments.filter((a) => !isEveryone(a));
  const reader = roleId(agentId, "reader");
  const visibilityCard = useRef<HTMLDivElement>(null);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button type="button" variant={variant} size={size} aria-label="Share agent">
            <UsersIcon />
          </Button>
        }
      />
      <PopoverContent
        align="end"
        sideOffset={8}
        className="w-[26rem] max-w-[calc(100vw-2rem)] gap-4 rounded-xl bg-background p-4 shadow-xl ring-1 ring-foreground/10 duration-200"
      >
        <h2 className="text-base font-semibold tracking-tight">Share agent</h2>
        {editor && (
          <form
            className="grid gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              if (!chosen) return;
              const principal = chosen.principal;
              // Invitees start as readers; the row's role menu adds more.
              void run(async () => {
                await iam.assignRole({ roleId: reader, principal });
                setChosen(undefined);
                setQuery("");
              });
            }}
          >
            <Label htmlFor="share-search" className="sr-only">
              Invite people or groups
            </Label>
            <div className="relative flex items-center gap-3">
              <div className="relative min-w-0 flex-1">
                {chosen ? (
                  <div className="flex h-9 items-center gap-2 rounded-md border bg-background px-2.5 text-sm ring-2 ring-ring/40">
                    <span className="flex min-w-0 items-center gap-1.5 rounded-md bg-muted px-2 py-1 font-medium">
                      <span className="truncate">{chosen.label}</span>
                      <button
                        type="button"
                        aria-label={`Remove ${chosen.label}`}
                        className="rounded-sm text-muted-foreground transition-colors hover:text-foreground"
                        onClick={() => setChosen(undefined)}
                      >
                        <XIcon className="size-3.5" />
                      </button>
                    </span>
                  </div>
                ) : (
                  <Input
                    id="share-search"
                    autoComplete="off"
                    placeholder="Invite people or groups"
                    value={query}
                    disabled={busy}
                    className="h-9 rounded-md px-2.5"
                    onChange={(event) => setQuery(event.target.value)}
                  />
                )}
                {!chosen && candidates.length > 0 && (
                  <ul
                    role="listbox"
                    aria-label="Matches"
                    className="absolute inset-x-0 top-full z-10 mt-1.5 grid max-h-64 gap-0.5 overflow-y-auto rounded-xl border bg-popover p-1.5 shadow-lg animate-in fade-in-0 zoom-in-95 duration-150"
                  >
                    {candidates.map((candidate) => (
                      <li key={key(candidate.principal)} role="presentation">
                        <button
                          type="button"
                          role="option"
                          aria-selected={false}
                          className="flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left text-sm transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
                          onClick={() => {
                            setChosen(candidate);
                            setCandidates([]);
                          }}
                        >
                          <Initial label={candidate.label} type={candidate.principal.type} />
                          <span className="grid min-w-0">
                            <span className="truncate font-medium">{candidate.label}</span>
                            <span className="truncate text-xs text-muted-foreground">
                              {candidate.detail}
                            </span>
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <Button
                type="submit"
                className="h-9 px-4 transition-opacity disabled:opacity-40"
                disabled={busy || !chosen}
              >
                Invite
              </Button>
            </div>
          </form>
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <ul aria-label="People with access" aria-busy={loading} className="grid gap-2">
          {people.map((assignment) => (
            <AssignmentRow
              key={key(assignment.principal)}
              assignment={assignment}
              roles={roles}
              agentId={agentId}
              reach={reach}
              editor={editor}
              busy={busy}
              you={
                assignment.principal?.type === PrincipalType.USER &&
                assignment.principal.id === caller.userId
              }
              run={run}
            />
          ))}
          {!people.length && (
            <li className="rounded-lg border border-dashed px-3 py-4 text-center text-sm text-muted-foreground">
              {loading ? "Loading…" : "Only administrators have access."}
            </li>
          )}
        </ul>
        <div className="grid gap-2.5">
          <p className="text-xs font-medium text-muted-foreground">General agent visibility</p>
          <div
            ref={visibilityCard}
            className="flex items-center gap-2.5 rounded-lg border bg-card px-2.5 py-1.5"
          >
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
              {everyone ? <GlobeIcon className="size-4" /> : <LockIcon className="size-4" />}
            </span>
            <div className="grid min-w-0 flex-1">
              {editor ? (
                <Select
                  value={everyone ? "anyone" : "restricted"}
                  disabled={busy}
                  onValueChange={(value) =>
                    void run(() => {
                      const principal = { type: PrincipalType.GROUP, id: EVERYONE_GROUP };
                      return value === "anyone"
                        ? iam.assignRole({ roleId: reader, principal })
                        : iam.revokeRole({ roleId: reader, principal });
                    })
                  }
                >
                  <SelectTrigger
                    aria-label="General agent visibility"
                    className={`${ghostTrigger} -ml-3 h-7 w-fit`}
                  >
                    <SelectValue>{everyone ? "Anyone can view" : "Restricted"}</SelectValue>
                  </SelectTrigger>
                  <SelectContent
                    anchor={visibilityCard}
                    align="start"
                    sideOffset={4}
                    alignItemWithTrigger={false}
                    collisionAvoidance={{ side: "none" }}
                    className="rounded-xl p-1.5"
                  >
                    <SelectItem value="restricted" className="rounded-lg py-2">
                      Restricted
                    </SelectItem>
                    <SelectItem value="anyone" className="rounded-lg py-2">
                      Anyone can view
                    </SelectItem>
                  </SelectContent>
                </Select>
              ) : (
                <span className="text-base font-medium">
                  {everyone ? "Anyone can view" : "Restricted"}
                </span>
              )}
              <span className="text-xs text-muted-foreground">
                {everyone ? "Every signed-in user can view this agent" : "Only people shared with"}
              </span>
            </div>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
