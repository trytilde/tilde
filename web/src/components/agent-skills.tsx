import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";
import {
  CableIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CodeIcon,
  LayersIcon,
  PlusIcon,
  SparklesIcon,
} from "lucide-react";
import type {
  Skill,
  SkillSource,
  SkillSourceKind,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import type { ConnectionSkills } from "@trytilde/contracts/tilde/management/v1/skills_pb.js";
import { Capability } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, skills } from "@/client";
import { servingContents } from "./agent-deployment";
import { CatalogIcon } from "./catalog";
import { KindLabel, SourceKindIcon, message } from "./skill-common";
import { RemoveButton } from "./remove-button";
import { Badge } from "./ui/badge";
import { ExpandableText } from "./expandable-text";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Switch } from "./ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { TableSkeletonRows } from "@/components/table-skeleton";

const COLUMNS = 4;
const count = (n: number) => `${n} skill${n === 1 ? "" : "s"}`;
const connectionCount = (c: ConnectionSkills) =>
  c.sources.reduce((sum, s) => sum + s.skillCount, 0);

/**
 * The skills one agent reaches, in one table grouped by skill group. A group is added with its
 * skills switched off; each skill switches on alone, or the group's switch turns on every skill
 * in it, including those a later sync adds. Skills linked to connections whose skills capability
 * the agent holds, and the skills bundled with its code, show too but are managed elsewhere.
 * Whoever holds edit_skills on the agent adds groups they can view; new skills start on the
 * Skills page.
 */
export function AgentSkills({ agentId }: { agentId: string }) {
  const [sources, setSources] = useState<SkillSource[]>([]);
  const [disabled, setDisabled] = useState<Set<string>>(() => new Set());
  const [excluded, setExcluded] = useState<Set<string>>(() => new Set());
  const [single, setSingle] = useState<Skill[]>([]);
  const [linked, setLinked] = useState<ConnectionSkills[]>([]);
  const [groupSkills, setGroupSkills] = useState<Skill[]>([]);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [allSources, setAllSources] = useState<SkillSource[]>([]);
  const [icons, setIcons] = useState<Map<string, string>>(() => new Map());
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [choosing, setChoosing] = useState(false);
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [assigned, visible] = await Promise.all([
        skills.listAgentSkills({ agentId }, { signal }),
        skills.listSkillSources({}, { signal }),
      ]);
      if (signal?.aborted) return;
      setSources(assigned.sources);
      setDisabled(new Set(assigned.disabledSourceIds));
      setExcluded(new Set(assigned.disabledSkillIds));
      setSingle(assigned.skills);
      setLinked(assigned.connections);
      setGroupSkills(assigned.groupSkills);
      setAllSources(visible.sources);
    },
    [agentId],
  );
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load skills."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  // Catalog groups show their provider's logo, as on the Skills page.
  useEffect(() => {
    const abort = new AbortController();
    skills
      .listCatalog({}, { signal: abort.signal })
      .then(({ groups }) => setIcons(new Map(groups.map((group) => [group.id, group.iconUrl]))))
      .catch(() => {});
    return () => abort.abort();
  }, []);
  async function act(work: () => Promise<unknown>, fallback: string) {
    setBusy(true);
    setError("");
    try {
      await work();
      await refresh();
    } catch (e) {
      setError(message(e, fallback));
    } finally {
      setBusy(false);
    }
  }
  function toggle(id: string) {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  const addGroup = (sourceId: string) =>
    act(() => skills.assignSkillSource({ agentId, sourceId }), "Unable to add the group.");
  const iconOf = (catalogGroup = "") => icons.get(catalogGroup) || undefined;
  const catalogGroups = new Map(
    [...allSources, ...sources].map((source) => [source.id, source.catalogGroup]),
  );
  const groups = assignedGroups(sources, single, linked, groupSkills).map((group) => ({
    ...group,
    enabled: group.routes.some((route) => route.kind === "group") && !disabled.has(group.id),
    iconUrl: iconOf(catalogGroups.get(group.id)),
  }));
  const bundled = useBundledGroups(agentId);
  const all = [...bundled.groups, ...groups];
  // Groups the agent does not have yet.
  const options = useMemo((): Option[] => {
    const added = new Set(sources.map((s) => s.id));
    return allSources
      .filter((s) => !added.has(s.id))
      .map((s) => ({
        id: s.id,
        name: s.name,
        detail: count(s.skillCount),
        iconUrl: icons.get(s.catalogGroup) || undefined,
      }));
  }, [sources, allSources, icons]);
  return (
    <section className="space-y-5" aria-label="Skills">
      {(error || bundled.error) && (
        <p role="alert" className="text-sm text-destructive">
          {error || bundled.error}
        </p>
      )}
      <div className="flex justify-end">
        <DropdownMenu>
          <DropdownMenuTrigger
            disabled={busy || loading}
            render={<Button className="cursor-pointer gap-2" />}
          >
            <PlusIcon />
            Add skills
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="min-w-56">
            <DropdownMenuItem onClick={() => setChoosing(true)}>
              <LayersIcon />
              Choose existing
            </DropdownMenuItem>
            <DropdownMenuItem render={<Link to="/skills" />}>
              <SparklesIcon />
              Add a new skill
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="overflow-hidden rounded-xl border">
        <Table aria-label="Assigned skills">
          <TableHeader className="bg-background">
            <TableRow>
              <TableHead className="h-11 w-16 px-5">Use</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Description</TableHead>
              <TableHead className="w-12" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {all.map((group) => {
              const expanded = !collapsed.has(group.id);
              const isBundled = group.kind === "bundled";
              const added = group.routes.some((route) => route.kind === "group");
              const whole = !!group.enabled;
              const viaConnection = group.routes.some((route) => route.kind === "connection");
              // An enabled group gives every skill but those switched off; a disabled one none.
              const on = (row: Row) =>
                isBundled ||
                viaConnection ||
                row.routes.length > 0 ||
                (whole && !excluded.has(row.id));
              const removable =
                added || group.rows.some((row) => row.routes.some((r) => r.kind === "skill"));
              return (
                <Fragment key={group.id}>
                  <TableRow
                    aria-label={`Group ${group.name}`}
                    className="bg-muted/40 hover:bg-muted/40"
                  >
                    <TableCell className="px-5">
                      {/* Bundled skills are set in code and always used: nothing to switch or remove. */}
                      {isBundled ? (
                        <CodeIcon aria-hidden="true" className="size-5 text-muted-foreground" />
                      ) : (
                        added && (
                          <Switch
                            aria-label={`Use every skill in ${group.name}`}
                            title="Every skill in the group, including those a later sync adds."
                            checked={whole}
                            disabled={busy}
                            onCheckedChange={(checked) =>
                              void act(
                                () =>
                                  skills.setSkillSourceEnabled({
                                    agentId,
                                    sourceId: group.id,
                                    enabled: checked,
                                  }),
                                "Unable to switch the group.",
                              )
                            }
                          />
                        )
                      )}
                    </TableCell>
                    <TableCell colSpan={2} className="py-2.5">
                      <div className="flex min-w-0 items-center gap-2">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`${expanded ? "Collapse" : "Expand"} ${group.name}`}
                          aria-expanded={expanded}
                          onClick={() => toggle(group.id)}
                        >
                          {expanded ? <ChevronDownIcon /> : <ChevronRightIcon />}
                        </Button>
                        {group.kind === "bundled" ? null : group.iconUrl ? (
                          <img
                            src={group.iconUrl}
                            alt=""
                            aria-hidden="true"
                            className="size-5 shrink-0 rounded-sm object-contain"
                          />
                        ) : (
                          <SourceKindIcon kind={group.kind} />
                        )}
                        <span className="truncate text-sm font-semibold">{group.name}</span>
                        <Badge variant="outline">
                          {isBundled
                            ? count(group.rows.length)
                            : `${group.rows.filter(on).length} of ${count(group.rows.length)}`}
                        </Badge>
                        {group.kind === "bundled" ? (
                          <>
                            <Badge
                              variant="secondary"
                              title="Ships with the agent's code in this deployment and is not shared with other agents. It takes precedence over an assigned skill with the same name."
                            >
                              Bundled
                            </Badge>
                            {group.detail && (
                              <span className="text-xs text-muted-foreground">{group.detail}</span>
                            )}
                          </>
                        ) : (
                          <KindLabel kind={group.kind} />
                        )}
                        <Via routes={group.routes} />
                      </div>
                    </TableCell>
                    <TableCell className="pr-3 text-right">
                      {!isBundled && removable && (
                        <RemoveButton
                          label={`Remove ${group.name}`}
                          title={`Remove ${group.name} from this agent?`}
                          description="The agent stops seeing every skill from this group on its next invocation. The group itself is kept."
                          disabled={busy}
                          onConfirm={() =>
                            act(
                              () => skills.unassignSkillSource({ agentId, sourceId: group.id }),
                              "Unable to remove the group.",
                            )
                          }
                        />
                      )}
                    </TableCell>
                  </TableRow>
                  {expanded &&
                    group.rows.map((row) => {
                      const single = row.routes.some((route) => route.kind === "skill");
                      // In an added group each skill switches while the group is on; a single skill
                      // outside one switches off; code and connections hold the rest.
                      const locked = viaConnection || (added ? !whole : !single);
                      return (
                        <TableRow key={row.id}>
                          <TableCell className="px-5">
                            {!isBundled && (
                              <Switch
                                aria-label={`Use ${row.name}`}
                                title={
                                  added && !whole
                                    ? "Switch the group on to use its skills."
                                    : undefined
                                }
                                checked={on(row)}
                                disabled={locked || busy}
                                onCheckedChange={(checked) =>
                                  void act(
                                    () =>
                                      added
                                        ? skills.setSkillEnabled({
                                            agentId,
                                            skillId: row.id,
                                            enabled: checked,
                                          })
                                        : skills.unassignSkill({ agentId, skillId: row.id }),
                                    "Unable to switch the skill.",
                                  )
                                }
                              />
                            )}
                          </TableCell>
                          <TableCell className="pl-12">
                            <span className="flex items-center gap-2">
                              {/* Bundled skills open read-only; the title says where the code declared them. */}
                              <Link
                                to="/skills/$skillId"
                                params={{ skillId: row.id }}
                                className="font-mono font-medium hover:underline"
                                title={row.origin ? `Declared at ${row.origin}` : undefined}
                              >
                                {row.name}
                              </Link>
                              {row.version > 0 && (
                                <Badge
                                  variant="secondary"
                                  className="bg-muted font-normal text-muted-foreground"
                                >
                                  v{row.version}
                                </Badge>
                              )}
                            </span>
                          </TableCell>
                          <TableCell>
                            <ExpandableText
                              text={row.description || "—"}
                              className="max-w-md text-muted-foreground"
                            />
                          </TableCell>
                          <TableCell />
                        </TableRow>
                      );
                    })}
                  {expanded && !group.rows.length && (
                    <TableRow>
                      <TableCell
                        colSpan={COLUMNS}
                        className="h-12 pl-28 text-sm text-muted-foreground"
                      >
                        No skills in this group yet.
                      </TableCell>
                    </TableRow>
                  )}
                </Fragment>
              );
            })}
            {!all.length &&
              (loading || bundled.loading ? (
                <TableSkeletonRows columns={COLUMNS} label="Loading skills" />
              ) : (
                <TableRow>
                  <TableCell colSpan={COLUMNS} className="h-24 text-center text-muted-foreground">
                    No skills assigned to this agent.
                  </TableCell>
                </TableRow>
              ))}
          </TableBody>
        </Table>
      </div>
      {linked.length > 0 && (
        <section aria-label="Connections" className="space-y-2">
          <h3 className="m-0 text-sm font-semibold">Through connections</h3>
          <ul className="m-0 flex flex-wrap gap-2 p-0">
            {linked.map((connection) => (
              <li
                key={connection.connectionId}
                className="flex items-center gap-2 rounded-lg border py-1 pr-1 pl-3 text-sm"
              >
                <CableIcon aria-hidden="true" className="size-4 text-muted-foreground" />
                <span className="font-medium">{connection.connectionName}</span>
                <span className="text-xs text-muted-foreground">
                  {count(connectionCount(connection))}
                  {connection.status !== "ready" && ` · ${connection.status || "not ready"}`}
                </span>
                <RemoveButton
                  size="icon-xs"
                  label={`Remove ${connection.connectionName}`}
                  title={`Remove ${connection.connectionName}'s skills from this agent?`}
                  description="The agent stops seeing the skills linked to this connection. The connection and its other capabilities are kept."
                  disabled={busy}
                  onConfirm={() =>
                    act(
                      () =>
                        connections.unassignCapability({
                          connectionId: connection.connectionId,
                          assignment: { capability: Capability.SKILLS, agentId },
                        }),
                      "Unable to remove the connection's skills.",
                    )
                  }
                />
              </li>
            ))}
          </ul>
        </section>
      )}
      <ChooseGroupDialog
        open={choosing}
        options={options}
        busy={busy}
        onClose={() => setChoosing(false)}
        onChoose={(id) => addGroup(id).then(() => setChoosing(false))}
      />
    </section>
  );
}

type Option = { id: string; name: string; detail: string; iconUrl?: string };

/** Pick a group the agent does not have yet; it is added with its skills switched off. */
function ChooseGroupDialog({
  open,
  options,
  busy,
  onClose,
  onChoose,
}: {
  open: boolean;
  options: Option[];
  busy: boolean;
  onClose: () => void;
  onChoose: (id: string) => Promise<void>;
}) {
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<string>();
  useEffect(() => {
    setQuery("");
    setChosen(undefined);
  }, [open]);
  const words = query.trim().toLowerCase();
  const matches = options.filter((option) =>
    `${option.name} ${option.detail}`.toLowerCase().includes(words),
  );
  return (
    <Dialog open={open} onOpenChange={(next) => !next && !busy && onClose()}>
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Choose existing skills</DialogTitle>
          <DialogDescription>
            Groups of skills you can view that this agent does not use yet. Its skills are added
            switched off.
          </DialogDescription>
        </DialogHeader>
        <Input
          aria-label="Search groups"
          placeholder="Search groups"
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
        />
        <ul
          role="listbox"
          aria-label="Available groups"
          className="m-0 grid max-h-[55dvh] list-none grid-cols-2 gap-x-2 gap-y-0.5 overflow-y-auto p-0 max-sm:grid-cols-1"
        >
          {matches.map((option) => {
            const selected = chosen === option.id;
            return (
              <li
                key={option.id}
                role="option"
                aria-selected={selected}
                tabIndex={0}
                onClick={() => setChosen(option.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setChosen(option.id);
                  }
                }}
                className={`flex min-w-0 cursor-pointer items-center gap-3 rounded-2xl px-3 py-[9.5px] outline-none hover:bg-foreground/5 focus-visible:bg-foreground/5 ${
                  selected ? "bg-primary/10 ring-1 ring-primary hover:bg-primary/10" : ""
                }`}
              >
                <CatalogIcon id={option.id} name={option.name} iconUrl={option.iconUrl} />
                <div className="flex min-w-0 flex-1 flex-col gap-px">
                  <h3 className="m-0 truncate text-[13px] leading-[18px] font-medium text-foreground">
                    {option.name}
                  </h3>
                  <p className="m-0 truncate text-[13px] leading-[18px] text-foreground/60">
                    {option.detail}
                  </p>
                </div>
              </li>
            );
          })}
          {!matches.length && (
            <li className="col-span-2 px-3 py-6 text-center text-sm text-muted-foreground">
              {options.length
                ? "No groups match your search."
                : "No other groups you can view. Add new skills from the Skills page."}
            </li>
          )}
        </ul>
        <DialogFooter>
          <Button type="button" variant="outline" disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button disabled={!chosen || busy} onClick={() => chosen && void onChoose(chosen)}>
            Continue
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

type Route = { kind: "group" } | { kind: "skill" } | { kind: "connection"; name: string };
type Row = {
  id: string;
  name: string;
  description: string;
  version: number;
  origin?: string;
  /** How the agent is given this skill on its own; empty when only its group is. */
  routes: Route[];
};
type Group = {
  id: string;
  name: string;
  kind: SkillSourceKind | "bundled";
  /** How the agent is given the whole group. */
  routes: Route[];
  rows: Row[];
  detail?: string;
  iconUrl?: string;
  /** False when the agent's whole-group assignment is switched off. */
  enabled?: boolean;
};

/**
 * One group per skill group the agent reaches: groups given whole (directly or through a
 * connection) list every skill in them; single skills sit under their own group.
 */
function assignedGroups(
  sources: SkillSource[],
  single: Skill[],
  linked: ConnectionSkills[],
  groupSkills: Skill[],
): Group[] {
  const groups = new Map<string, Group>();
  const group = (id: string, name: string, kind: SkillSourceKind) => {
    let found = groups.get(id);
    if (!found) {
      found = { id, name, kind, routes: [], rows: [] };
      groups.set(id, found);
    }
    return found;
  };
  const row = (skill: Skill) => {
    const into = group(skill.sourceId, skill.sourceName, skill.sourceKind);
    let found = into.rows.find((r) => r.id === skill.id);
    if (!found) {
      found = {
        id: skill.id,
        name: skill.name,
        description: skill.latest?.description ?? "",
        version: skill.latest?.number ?? 0,
        routes: [],
      };
      into.rows.push(found);
    }
    return found;
  };
  for (const source of sources)
    group(source.id, source.name, source.kind).routes.push({ kind: "group" });
  for (const connection of linked)
    for (const source of connection.sources)
      group(source.id, source.name, source.kind).routes.push({
        kind: "connection",
        name: connection.connectionName,
      });
  for (const skill of groupSkills) row(skill);
  for (const skill of single) row(skill).routes.push({ kind: "skill" });
  return [...groups.values()]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((g) => ({ ...g, rows: g.rows.sort((a, b) => a.name.localeCompare(b.name)) }));
}

/** Connections a skill or group comes through, which are managed from the connection. */
function Via({ routes }: { routes: Route[] }) {
  return routes.map((route, index) =>
    route.kind === "connection" ? (
      <Badge key={index} variant="outline" className="font-normal">
        <CableIcon />
        via {route.name}
      </Badge>
    ) : null,
  );
}

/**
 * The skills bundled with the agent's code, one group per deployment that can receive
 * invocations (`servingContents`).
 */
function useBundledGroups(agentId: string) {
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    const signal = abort.signal;
    setLoading(true);
    void (async () => {
      const serving = await servingContents(agentId, signal);
      if (signal.aborted) return;
      setGroups(
        serving.map(({ deployment, name, detail, contents }) => ({
          id: `bundled-${deployment.id}`,
          name,
          kind: "bundled",
          routes: [],
          detail,
          rows: contents.skills.map((skill) => ({
            id: skill.skillId,
            name: skill.name,
            description: skill.description,
            version: skill.number,
            origin: skill.origin,
            routes: [],
          })),
        })),
      );
    })()
      .catch((e) => {
        if (!signal.aborted) setError(message(e, "Unable to load bundled skills."));
      })
      .finally(() => {
        if (!signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [agentId]);
  return { groups, loading, error };
}
