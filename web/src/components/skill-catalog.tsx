import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { ExternalLinkIcon, XIcon } from "lucide-react";
import type { CatalogGroup, CatalogSkill } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import {
  CatalogIcon,
  CatalogProviderRow,
  CatalogSearchField,
  CatalogSection,
  CatalogSkeleton,
  CategoryFilter,
  categoryLabel,
  compareCategories,
} from "./catalog";
import { usePageCrumb } from "./dashboard-breadcrumbs";
import { message } from "./skill-common";
import { Badge } from "./ui/badge";
import { ExpandableText } from "./expandable-text";
import { Button, buttonVariants } from "./ui/button";
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "./ui/sheet";
import { Skeleton } from "./ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";

const category = (group: CatalogGroup) => group.category || "other";

/**
 * The Tilde Catalog: groups built into Tilde, whose skills are listed up front, and managed
 * providers, trusted repositories whose skills are listed once enabled and synced. A provider's
 * panel previews its skills live before then. Enabling one creates its group (a skill source).
 * A group's panel opens at its own path, /skills/catalog/$groupId, over the catalog.
 */
/** Tilde's own groups lead the catalog; the rest follow the shared category order. */
function tildeFirst(left: string, right: string) {
  if ((left === "Tilde") !== (right === "Tilde")) return left === "Tilde" ? -1 : 1;
  return compareCategories(left, right);
}

export function SkillCatalogPage() {
  const navigate = useNavigate();
  const { groupId: openId } = useParams({ strict: false });
  usePageCrumb(openId ? `/skills/catalog/${openId}` : "/skills/catalog", "Tilde Catalog");
  const [groups, setGroups] = useState<CatalogGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const refresh = useCallback(async (signal?: AbortSignal) => {
    const response = await skills.listCatalog({}, { signal });
    if (signal?.aborted) return;
    setGroups(response.groups);
    setError("");
  }, []);
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load the catalog."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  const categories = useMemo(() => [...new Set(groups.map(category))].sort(tildeFirst), [groups]);
  const needle = query.trim().toLowerCase();
  const sections = new Map<string, CatalogGroup[]>();
  for (const group of groups) {
    if (selectedCategory !== null && category(group) !== selectedCategory) continue;
    // A group matches on its own text or on one of its skills'.
    const text = [group.name, group.description, ...group.skills.map((s) => s.name)].join(" ");
    if (!text.toLowerCase().includes(needle)) continue;
    sections.set(category(group), [...(sections.get(category(group)) ?? []), group]);
  }
  const open = groups.find((group) => group.id === openId);
  return (
    <section
      aria-label="Tilde Catalog"
      className="flex min-h-full min-w-0 flex-col text-foreground"
    >
      <div className="shrink-0 border-b bg-background">
        <div
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
          role="group"
          aria-label="Catalog filter bar"
        >
          <CategoryFilter
            categories={categories}
            selectedCategory={selectedCategory}
            onSelect={setSelectedCategory}
          />
          <CatalogSearchField label="Search catalog" value={query} onChange={setQuery} />
        </div>
      </div>

      <section aria-label="Skill groups" className="p-4 lg:p-6">
        {error ? (
          <div
            className="mx-2 mb-2 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
            role="alert"
          >
            {error}
          </div>
        ) : null}
        {loading ? (
          <CatalogSkeleton label="Loading catalog" />
        ) : (
          <div className="space-y-3">
            {[...sections]
              .sort(([left], [right]) => tildeFirst(left, right))
              .map(([name, items]) => (
                <CatalogSection key={name} title={categoryLabel(name)}>
                  {items.map((group) => (
                    <CatalogProviderRow
                      key={group.id}
                      entry={group}
                      badge={group.sourceId ? <Badge variant="secondary">Enabled</Badge> : null}
                      onOpen={() =>
                        void navigate({
                          to: "/skills/catalog/$groupId",
                          params: { groupId: group.id },
                        })
                      }
                    />
                  ))}
                </CatalogSection>
              ))}
          </div>
        )}
        {!loading && !error && sections.size === 0 ? (
          <div className="mt-2.5 rounded-xl border border-dashed border-foreground/15 p-9 text-center text-[12.5px] text-foreground/40">
            No skill groups match these filters.
          </div>
        ) : null}
      </section>

      {open ? (
        <CatalogGroupPanel
          key={open.id}
          group={open}
          refresh={refresh}
          onClose={() => void navigate({ to: "/skills/catalog" })}
        />
      ) : null}
    </section>
  );
}

/** One catalog group: what it holds, where it comes from, and enabling it. */
function CatalogGroupPanel({
  group,
  refresh,
  onClose,
}: {
  group: CatalogGroup;
  refresh: () => Promise<void>;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const provider = !!group.repositoryUrl;
  const repository = group.repositoryUrl.replace(/^https:\/\//, "");
  // The panel's skills come from the group itself: a provider not yet enabled is previewed
  // from its repository by the server. Enabling changes the answer, so it is fetched again.
  const [listed, setListed] = useState<{ skills?: CatalogSkill[]; error?: string }>({});
  useEffect(() => {
    const abort = new AbortController();
    setListed({});
    skills
      .getCatalogGroup({ id: group.id }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) setListed({ skills: response.group?.skills ?? [] });
      })
      .catch((e) => {
        if (!abort.signal.aborted) setListed({ error: message(e, "Unable to list the skills.") });
      });
    return () => abort.abort();
  }, [group.id, group.sourceId]);
  async function enable() {
    setBusy(true);
    setError("");
    try {
      await skills.enableCatalogGroup({ group: group.id });
      await refresh();
    } catch (e) {
      setError(message(e, "Unable to enable the group."));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Sheet
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <SheetContent
        side="right"
        showCloseButton={false}
        className="w-full gap-0 overflow-y-auto p-0 data-[side=right]:w-full data-[side=right]:sm:w-1/2 data-[side=right]:sm:min-w-[560px] data-[side=right]:sm:max-w-none"
      >
        <SheetClose
          aria-label="Close"
          className="absolute top-4 right-4 grid size-7 cursor-pointer place-items-center rounded-md bg-transparent text-foreground/40 hover:bg-foreground/5 hover:text-foreground focus-visible:bg-foreground/5 focus-visible:text-foreground focus-visible:outline-none"
        >
          <XIcon className="size-3.5" />
        </SheetClose>
        <div className="flex flex-col gap-6 p-6">
          <header className="grid gap-1.5 pr-10">
            <p className="m-0 font-mono text-[11px] text-muted-foreground uppercase">
              {categoryLabel(category(group))} ·{" "}
              {provider ? "Managed provider" : "Built into Tilde"}
            </p>
            <div className="flex items-center gap-2.5">
              <CatalogIcon id={group.id} name={group.name} iconUrl={group.iconUrl} small />
              <SheetTitle className="m-0 text-lg font-semibold">{group.name}</SheetTitle>
            </div>
            <SheetDescription className="m-0 text-[13px] leading-5 text-foreground/60">
              {group.description}
            </SheetDescription>
            {provider ? (
              <a
                href={group.repositoryUrl}
                target="_blank"
                rel="noreferrer"
                className="inline-flex w-fit items-center gap-1 text-xs font-medium text-foreground/60 hover:text-foreground hover:underline"
              >
                {repository}
                <ExternalLinkIcon className="size-3" />
              </a>
            ) : null}
          </header>

          {error ? (
            <p
              className="m-0 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
              role="alert"
            >
              {error}
            </p>
          ) : null}

          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="m-0 text-xs text-muted-foreground">
              {group.sourceId
                ? provider
                  ? "Enabled. Its skills sync from the repository; add it to agents from their Skills tab."
                  : "Enabled. It updates with Tilde; add it to agents from their Skills tab."
                : provider
                  ? `Preview from ${repository} @ ${group.branch}. Enabling copies these skills and keeps them in sync.`
                  : "Enabling adds these skills and keeps them up to date with Tilde."}
            </p>
            {group.sourceId ? (
              <Link to="/skills" className={buttonVariants({ variant: "outline" })}>
                Open skills
              </Link>
            ) : (
                <Button disabled={busy} onClick={() => void enable()}>
                  {busy ? "Enabling…" : "Enable"}
                </Button>
            )}
          </div>

          <CatalogSkills group={group} provider={provider} {...listed} />
        </div>
      </SheetContent>
    </Sheet>
  );
}

/** The group's skills: loading, failed, empty (a provider's first sync may still be running) or a table. */
function CatalogSkills({
  group,
  provider,
  skills,
  error,
}: {
  group: CatalogGroup;
  provider: boolean;
  skills?: CatalogSkill[];
  error?: string;
}) {
  if (error)
    return (
      <p
        className="m-0 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
        role="alert"
      >
        {error}
      </p>
    );
  if (!skills)
    return (
      <div aria-label="Loading skills" className="grid gap-2">
        <Skeleton className="h-8" />
        <Skeleton className="h-8" />
        <Skeleton className="h-8" />
      </div>
    );
  if (!skills.length)
    return (
      <p
        role="status"
        className="m-0 rounded-lg border px-3 py-4 text-xs leading-5 text-muted-foreground"
      >
        {!provider
          ? "This group has no skills."
          : group.sourceId
            ? "Syncing: the repository's skills appear here and on the group's page once the first sync finishes."
            : "The repository has no skills within this provider's scope."}
      </p>
    );
  return (
    <section aria-label="Skills" className="grid gap-2">
      <h3 className="m-0 text-[13px] font-medium">
        Skills <span className="text-muted-foreground tabular-nums">{skills.length}</span>
      </h3>
      <div className="overflow-hidden rounded-lg border">
        <Table className="table-fixed text-xs">
          <TableHeader>
            <TableRow>
              <TableHead className="w-[30%]">Name</TableHead>
              <TableHead>Description</TableHead>
              {provider ? <TableHead className="w-[28%]">Path</TableHead> : null}
            </TableRow>
          </TableHeader>
          <TableBody>
            {skills.map((skill) => (
              <TableRow key={skill.name}>
                <TableCell className="truncate font-mono font-medium" title={skill.name}>
                  {skill.name}
                </TableCell>
                <TableCell className="text-muted-foreground">
                  <ExpandableText text={skill.description || "—"} />
                </TableCell>
                {provider ? (
                  <TableCell
                    className="truncate font-mono text-muted-foreground"
                    title={skill.path || undefined}
                  >
                    {skill.path || "/"}
                  </TableCell>
                ) : null}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </section>
  );
}
