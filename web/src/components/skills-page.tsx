import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  GitBranchIcon,
  LayoutGridIcon,
  PlusIcon,
  SquarePenIcon,
  TriangleAlertIcon,
} from "lucide-react";
import {
  SkillSourceKind,
  type Skill,
  type SkillSource,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import { KindLabel, Pill, SourceKindIcon, message } from "./skill-common";
import { EditorSkillDialog, GitSourceDialog } from "./skill-dialogs";
import { SkillGroupActions } from "./skill-group-actions";
import { SkillAgentsDialog, type SkillAgent } from "./skill-agents-dialog";
import { ExpandableText } from "./expandable-text";
import { RemoveButton } from "./remove-button";
import { AgentStack, useAgentRegistry } from "./agent-stack";
import { Button } from "./ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";
import { FilterSearchInput } from "@/features/observability/filter-search-input";
import {
  parseSkillQuery,
  serializeSkillQuery,
  skillSuggestions,
  type SkillFilter,
} from "./skill-filter";
import { TableSkeletonRows } from "@/components/table-skeleton";

const COLUMNS = 4;
type Group = {
  id: string;
  name: string;
  kind: SkillSourceKind;
  iconUrl?: string;
  /** Agents given the whole group. */
  agentIds: string[];
  skills: Skill[];
  source?: SkillSource;
};
const KINDS = {
  all: "All",
  catalog: "Catalog",
  git: "Git",
  editor: "Editor",
} as const;
const KIND_OF: Record<string, SkillSourceKind | undefined> = {
  catalog: SkillSourceKind.CATALOG,
  git: SkillSourceKind.GIT,
  editor: SkillSourceKind.EDITOR,
};

/**
 * Every skill, grouped by where it comes from. In the UI a skill source is a group: one enabled
 * from Tilde's catalog, a GitHub repository, or a collection authored here.
 */
export function SkillsPage() {
  const navigate = useNavigate();
  const [list, setList] = useState<Skill[]>([]);
  const [sources, setSources] = useState<SkillSource[]>([]);
  const [filter, setFilter] = useState<SkillFilter>({ groups: [], words: [] });
  const [kind, setKind] = useState<keyof typeof KINDS>("all");
  const [viewing, setViewing] = useState<{ title: string; agents: SkillAgent[] }>();
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<"git" | "editor" | null>(null);
  const [editorGroup, setEditorGroup] = useState<string>();
  const editorGroups = useMemo(
    () => sources.filter((source) => source.kind === SkillSourceKind.EDITOR),
    [sources],
  );
  // The editor groups skills can be added to.
  let editable = editorGroups;
  let empty =
    "No skills yet. Enable a group from the Tilde Catalog, add a Git repository or create one in the editor.";
  // Catalog groups show their provider's logo, as on the catalog page.
  const [icons, setIcons] = useState<Map<string, string>>(() => new Map());
  // Names and avatars for the Agents column.
  const registry = useAgentRegistry();
  useEffect(() => {
    const abort = new AbortController();
    skills
      .listCatalog({}, { signal: abort.signal })
      .then(({ groups }) => setIcons(new Map(groups.map((group) => [group.id, group.iconUrl]))))
      .catch(() => {});
    return () => abort.abort();
  }, []);
  const refresh = useCallback(async (signal?: AbortSignal) => {
    const [listed, found] = await Promise.all([
      skills.listSkills({}, { signal }),
      skills.listSkillSources({}, { signal }),
    ]);
    if (signal?.aborted) return;
    setList(listed.skills);
    setSources(found.sources);
  }, []);
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load skills."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  // Every visible group shows, empty ones too; words keep only groups with a matching skill.
  const groups = useMemo(() => {
    const wanted = filter.groups.map((group) => group.toLowerCase());
    const byId = new Map<string, Group>(
      sources.map((s) => [
        s.id,
        {
          id: s.id,
          name: s.name,
          kind: s.kind,
          iconUrl: icons.get(s.catalogGroup),
          agentIds: s.agentIds,
          skills: [],
          source: s,
        },
      ]),
    );
    for (const skill of list) {
      const text = `${skill.name} ${skill.latest?.description ?? ""}`.toLowerCase();
      if (!filter.words.every((word) => text.includes(word))) continue;
      let group = byId.get(skill.sourceId);
      if (!group) {
        group = {
          id: skill.sourceId,
          name: skill.sourceName,
          kind: skill.sourceKind,
          agentIds: [],
          skills: [],
        };
        byId.set(group.id, group);
      }
      group.skills.push(skill);
    }
    return [...byId.values()]
      .filter((group) => kind === "all" || group.kind === KIND_OF[kind])
      .filter((group) => !wanted.length || wanted.includes(group.name.toLowerCase()))
      .filter((group) => !filter.words.length || group.skills.length)
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((group) => ({
        ...group,
        skills: group.skills.sort((a, b) => a.name.localeCompare(b.name)),
      }));
  }, [sources, list, filter, kind, icons]);
  const filtered = kind !== "all" || filter.groups.length > 0 || filter.words.length > 0;
  const groupNames = useMemo(() => sources.map((source) => source.name), [sources]);
  const suggest = useCallback(
    (query: string, caret: number) => skillSuggestions(query, caret, groupNames),
    [groupNames],
  );
  function toggle(id: string) {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  const failing = sources.filter((source) => source.syncError);
  return (
    <div className="flex flex-col">
      <div className="relative z-20 shrink-0 border-b bg-background">
        <div
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
          role="group"
          aria-label="Skill filter bar"
        >
          <Tabs
            value={kind}
            onValueChange={(value) => setKind(value as keyof typeof KINDS)}
            className="shrink-0 gap-0"
          >
            <TabsList aria-label="Group types" className="h-[26px] gap-0.5 rounded-md p-0.5">
              {Object.entries(KINDS).map(([id, label]) => (
                <TabsTrigger
                  key={id}
                  value={id}
                  className="h-[22px] rounded px-2 text-xs font-normal data-active:font-bold"
                >
                  {label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
          <span aria-hidden="true" className="mx-1 h-5 shrink-0 border-l" />
          <FilterSearchInput
            label="Search skills"
            canonical={serializeSkillQuery(filter)}
            disabled={loading}
            validate={parseSkillQuery}
            suggest={suggest}
            onApply={(query) => setFilter(parseSkillQuery(query))}
          />
        </div>
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap gap-3">
          <Pill
            icon={<LayoutGridIcon />}
            label="Tilde Catalog"
            onClick={() => void navigate({ to: "/skills/catalog" })}
          />
          <Pill icon={<GitBranchIcon />} label="Add from Git" onClick={() => setDialog("git")} />
          <Pill
            icon={<SquarePenIcon />}
            label="Create in editor"
            onClick={() => {
              setEditorGroup(undefined);
              setDialog("editor");
            }}
          />
        </div>
        {failing.length > 0 && (
          <p
            role="status"
            className="m-0 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-amber-700 dark:text-amber-400"
          >
            <TriangleAlertIcon aria-hidden="true" className="size-4" />
            Sync failed for
            {failing.map((source, index) => (
              <span key={source.id}>
                <span className="font-medium">{source.name}</span>
                {index < failing.length - 1 && ","}
              </span>
            ))}
          </p>
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-xl border">
          <Table aria-label="Skills">
            <TableHeader className="bg-background">
              <TableRow>
                <TableHead className="h-11 px-5">Name</TableHead>
                <TableHead>Description</TableHead>
                <TableHead className="px-5 text-center">Version</TableHead>
                <TableHead className="w-60 px-5">Agents</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {groups.length ? (
                groups.map((group) => {
                  const expanded = !collapsed.has(group.id);
                  const addable = editable.some((source) => source.id === group.id);
                  return (
                    <Fragment key={group.id}>
                      <TableRow
                        aria-label={`Group ${group.name}`}
                        className="bg-muted/40 hover:bg-muted/40"
                      >
                        <TableCell colSpan={COLUMNS - 1} className="px-3 py-2.5">
                          <div className="flex flex-wrap items-center justify-between gap-3">
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
                              {group.iconUrl ? (
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
                              <span className="rounded-md border bg-background px-1.5 text-xs tabular-nums text-muted-foreground">
                                {group.skills.length}
                              </span>
                              <KindLabel kind={group.kind} />
                            </div>
                            <div className="flex items-center gap-1">
                              {addable && (
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  aria-label={`New skill in ${group.name}`}
                                  onClick={() => {
                                    setEditorGroup(group.id);
                                    setDialog("editor");
                                  }}
                                >
                                  <PlusIcon />
                                  New skill
                                </Button>
                              )}
                            </div>
                          </div>
                        </TableCell>
                        <TableCell className="px-5">
                          <div className="flex items-center justify-between gap-2">
                            <AgentStack
                              ids={group.agentIds}
                              registry={registry}
                              onOpen={() =>
                                setViewing({
                                  title: group.name,
                                  agents: group.agentIds.map((id) => ({ id, via: "group" })),
                                })
                              }
                            />
                            {group.source && (
                              <SkillGroupActions group={group.source} onChanged={refresh} />
                            )}
                          </div>
                        </TableCell>
                      </TableRow>
                      {expanded &&
                        group.skills.map((skill) => (
                          <TableRow
                            key={skill.id}
                            className="cursor-pointer"
                            onClick={() =>
                              void navigate({
                                to: "/skills/$skillId",
                                params: { skillId: skill.id },
                              })
                            }
                          >
                            <TableCell className="pl-14">
                              <Link
                                to="/skills/$skillId"
                                params={{ skillId: skill.id }}
                                className="font-mono font-medium hover:underline"
                                onClick={(event) => event.stopPropagation()}
                              >
                                {skill.name}
                              </Link>
                            </TableCell>
                            <TableCell>
                              <ExpandableText
                                text={skill.latest?.description || "—"}
                                className="max-w-md text-muted-foreground"
                              />
                            </TableCell>
                            <TableCell className="px-5 text-center">
                              {skill.latest ? `v${skill.latest.number}` : "—"}
                            </TableCell>
                            <TableCell className="px-5">
                              <div className="flex items-center justify-between gap-2">
                                {/* Agents given the whole group show on the group's row only. */}
                                <AgentStack
                                  ids={skill.agentIds.filter((id) => !group.agentIds.includes(id))}
                                  registry={registry}
                                  onOpen={() =>
                                    setViewing({
                                      title: skill.name,
                                      agents: skill.agentIds.map((id) => ({
                                        id,
                                        via: group.agentIds.includes(id) ? "group" : "skill",
                                      })),
                                    })
                                  }
                                />
                                {addable && (
                                  <span onClick={(event) => event.stopPropagation()}>
                                    <RemoveButton
                                      label={`Delete ${skill.name}`}
                                      title={`Delete ${skill.name}?`}
                                      description="The skill and its version history are deleted, and every agent given it stops seeing it on its next invocation."
                                      confirmLabel="Delete skill"
                                      onConfirm={() =>
                                        skills
                                          .deleteSkill({ id: skill.id })
                                          .then(() => refresh())
                                          .catch((e) =>
                                            setError(message(e, "Unable to delete the skill.")),
                                          )
                                      }
                                    />
                                  </span>
                                )}
                              </div>
                            </TableCell>
                          </TableRow>
                        ))}
                      {expanded && group.skills.length === 0 && (
                        <TableRow>
                          <TableCell
                            colSpan={COLUMNS}
                            className="h-12 pl-14 text-sm text-muted-foreground"
                          >
                            No skills in this group yet.
                          </TableCell>
                        </TableRow>
                      )}
                    </Fragment>
                  );
                })
              ) : loading ? (
                <TableSkeletonRows columns={COLUMNS} label="Loading skills" />
              ) : (
                <TableRow>
                  <TableCell colSpan={COLUMNS} className="h-24 text-center text-muted-foreground">
                    {error
                      ? "Unable to load skills."
                      : filtered
                        ? "No skills match your search."
                        : empty}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
        <SkillAgentsDialog
          open={!!viewing}
          onOpenChange={(open) => !open && setViewing(undefined)}
          title={viewing?.title ?? ""}
          agents={viewing?.agents ?? []}
          registry={registry}
        />
        <GitSourceDialog
          open={dialog === "git"}
          onOpenChange={(open) => setDialog(open ? "git" : null)}
        />
        <EditorSkillDialog
          open={dialog === "editor"}
          groups={editable}
          group={editorGroup}
          onOpenChange={(open) => setDialog(open ? "editor" : null)}
        />
      </div>
    </div>
  );
}
