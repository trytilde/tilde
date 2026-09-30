import type { FieldValues, Path, UseFormReturn } from "react-hook-form";
import { CodeIcon, GitBranchIcon, LayoutGridIcon, SquarePenIcon } from "lucide-react";
import { SkillSourceKind, type SkillSource } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { Capability, type Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { NativeSelect } from "@trytilde/connection-ui";
import { connections, skills } from "@/client";
import { date } from "@/features/tracing/format";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";

export const message = (error: unknown, fallback: string) =>
  error instanceof Error ? error.message : fallback;
export const when = (timestamp: { seconds: bigint } | undefined) =>
  timestamp ? date(new Date(Number(timestamp.seconds) * 1000).toISOString()) : "—";
export const ENTRYPOINT = "SKILL.md";
export const skillTemplate = (name: string) =>
  `---\nname: ${name}\ndescription: When to use this skill, in one sentence.\n---\n\n# ${name}\n`;
export const validName = (name: string) => /^[a-z0-9_-]{1,64}$/.test(name);

const kindLabels: Record<SkillSourceKind, string> = {
  [SkillSourceKind.UNSPECIFIED]: "",
  [SkillSourceKind.CATALOG]: "Catalog",
  [SkillSourceKind.GIT]: "Git",
  [SkillSourceKind.EDITOR]: "Editor",
  [SkillSourceKind.BUNDLED]: "Bundled",
};
export function SourceKindIcon({
  kind,
  className = "size-4",
}: {
  kind: SkillSourceKind;
  className?: string;
}) {
  const Icon =
    kind === SkillSourceKind.GIT
      ? GitBranchIcon
      : kind === SkillSourceKind.EDITOR
        ? SquarePenIcon
        : kind === SkillSourceKind.BUNDLED
          ? CodeIcon
          : LayoutGridIcon;
  return <Icon aria-hidden="true" className={`shrink-0 text-muted-foreground ${className}`} />;
}
export function KindLabel({ kind }: { kind: SkillSourceKind }) {
  return (
    <span className="rounded bg-muted px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
      {kindLabels[kind]}
    </span>
  );
}

export const pillClass =
  "h-[45px] cursor-pointer gap-2 rounded-full border-border bg-background px-5 text-sm font-semibold shadow-sm hover:bg-muted/50 [&_svg]:size-4";
export function Pill({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button variant="outline" onClick={onClick} className={pillClass}>
      {icon}
      {label}
    </Button>
  );
}

/** Every connection offering the skills capability. */
export async function listSkillConnections(signal?: AbortSignal) {
  const found: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections(
      { pageSize: 100, pageToken, capability: Capability.SKILLS },
      { signal },
    );
    found.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return found;
}

/**
 * Choosing an editor group: an existing one, or a new one named here. In the UI a skill's
 * source is its group.
 */
export const NEW_GROUP = "new";
export type GroupChoice = { groupId: string; groupName: string };
export function GroupFields<T extends FieldValues & GroupChoice>({
  form,
  groups,
}: {
  form: UseFormReturn<T>;
  groups: readonly SkillSource[];
}) {
  const groupId = "groupId" as Path<T>;
  const groupName = "groupName" as Path<T>;
  const error = form.formState.errors.groupName?.message;
  // Controlled, so a group created by a failed attempt shows once its option renders.
  const chosen = form.watch(groupId);
  return (
    <>
      <div className="grid gap-2">
        <Label htmlFor="skill-group">Group</Label>
        <NativeSelect id="skill-group" {...form.register(groupId)} value={chosen}>
          {groups.map((group) => (
            <option key={group.id} value={group.id}>
              {group.name}
            </option>
          ))}
          <option value={NEW_GROUP}>New group…</option>
        </NativeSelect>
      </div>
      {chosen === NEW_GROUP && (
        <div className="grid gap-2">
          <Label htmlFor="skill-group-name">Group name</Label>
          <Input
            id="skill-group-name"
            placeholder="Support team"
            aria-invalid={!!error}
            {...form.register(groupName, {
              validate: (value: string, all: T) =>
                all.groupId !== NEW_GROUP || !!value.trim() || "Name the new group.",
            })}
          />
          {typeof error === "string" && <p className="m-0 text-xs text-destructive">{error}</p>}
        </div>
      )}
    </>
  );
}
/**
 * The chosen group's id, creating the new group first. The form then points at the created
 * group, so a retry after a later failure does not create another.
 */
export async function chosenGroup<T extends FieldValues & GroupChoice>(
  form: UseFormReturn<T>,
  values: GroupChoice,
) {
  if (values.groupId !== NEW_GROUP) return { id: values.groupId };
  const { source } = await skills.createEditorSource({ name: values.groupName.trim() });
  if (!source) throw new Error("Unable to create the group.");
  form.setValue("groupId" as Path<T>, source.id as T[Path<T>]);
  return { id: source.id, created: source };
}

/**
 * `name:` and `description:` from a SKILL.md front matter, read like the server does: indented
 * lines continue a value (`>`/`|` block scalars, wrapped plain scalars) and quotes are trimmed.
 */
export function frontMatter(md: string) {
  const lines = md.split("\n");
  const found: { name?: string; description?: string } = {};
  if (lines[0]?.trim() !== "---") return found;
  for (let i = 1; i < lines.length && lines[i].trim() !== "---"; i++) {
    const match = /^(name|description):(.*)$/.exec(lines[i]);
    if (!match) continue;
    const continued: string[] = [];
    while (i + 1 < lines.length && /^(\s|$)/.test(lines[i + 1]) && lines[i + 1].trim() !== "---") {
      continued.push(lines[++i].trim());
    }
    const rest = match[2].trim();
    const value = (
      rest.startsWith("|")
        ? continued.join("\n")
        : rest.startsWith(">")
          ? continued.join(" ")
          : [rest, ...continued].join(" ")
    )
      .trim()
      .replace(/^["']|["']$/g, "");
    found[match[1] as "name" | "description"] = value;
  }
  return found;
}

/** SKILL.md with one front matter key set to a single-line value, adding the block if missing. */
export function withFrontMatter(md: string, key: "name" | "description", value: string) {
  const plain = value.replace(/\s+/g, " ").trim();
  // Quote what YAML would read as something other than plain text; the reader trims quotes.
  const line = `${key}: ${
    /^[\s"'>|&*!%@`{[\]#,?:-]|: | #/.test(plain) ? `"${plain.replace(/"/g, "'")}"` : plain
  }`;
  const lines = md.split("\n");
  if (lines[0]?.trim() !== "---") return `---\n${line}\n---\n${md}`;
  const end = lines.findIndex((l, i) => i > 0 && l.trim() === "---");
  if (end === -1) return `---\n${line}\n---\n${md}`;
  const start = lines.findIndex((l, i) => i > 0 && i < end && l.startsWith(`${key}:`));
  if (start === -1) return [...lines.slice(0, end), line, ...lines.slice(end)].join("\n");
  let stop = start + 1;
  while (stop < end && /^(\s|$)/.test(lines[stop])) stop++;
  return [...lines.slice(0, start), line, ...lines.slice(stop)].join("\n");
}

/** SKILL.md split into its front matter block (delimiters included) and the markdown body. */
export function splitFrontMatter(md: string) {
  const match = /^---\r?\n[\s\S]*?\r?\n---[ \t]*(\r?\n|$)/.exec(md);
  return match ? { head: match[0], body: md.slice(match[0].length) } : { head: "", body: md };
}
