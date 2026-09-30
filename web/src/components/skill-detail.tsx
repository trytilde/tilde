import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckIcon, PencilIcon, SaveIcon, XIcon } from "lucide-react";
import {
  SkillSourceKind,
  type Skill,
  type SkillFile,
  type SkillVersion,
} from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import { usePageCrumb } from "./dashboard-breadcrumbs";
import {
  ENTRYPOINT,
  KindLabel,
  SourceKindIcon,
  frontMatter,
  message,
  validName,
  withFrontMatter,
} from "./skill-common";
import { OpenFile } from "./file-pane";
import { SkillExplorer, type Incoming } from "./skill-explorer";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Badge } from "./ui/badge";

/**
 * A file while editing. Stored binaries are sent back with `keep` (`data` unset), so saving
 * reuses the current copy without downloading and re-uploading it; `from` is its stored path
 * once renamed or moved.
 */
type DraftFile = {
  path: string;
  from?: string;
  content: string;
  binary: boolean;
  data?: Uint8Array;
  mediaType: string;
  sizeBytes: bigint;
  downloadUrl: string;
  executable: boolean;
};
const isBinary = (file: SkillFile) => !file.text;
const toDraft = (file: SkillFile): DraftFile => ({
  path: file.path,
  content: file.content,
  binary: isBinary(file),
  mediaType: file.mediaType,
  sizeBytes: file.sizeBytes,
  downloadUrl: file.downloadUrl,
  executable: file.executable,
});
const validPath = (path: string) =>
  /^[A-Za-z0-9._-]+(\/[A-Za-z0-9._-]+)*$/.test(path) &&
  !path.split("/").some((s) => s === "." || s === "..");
const sortByPath = (files: DraftFile[]) =>
  [...files].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));

/**
 * One skill's current files. Skills in editor sources are edited here by the source's editors,
 * and each save becomes the skill's next version; catalog, git and bundled skills are read-only
 * and change only through their origin (the API refuses edits to them too). Version history is
 * kept but not shown for now.
 */
export function SkillDetail({ id }: { id: string }) {
  // A fresh editor per skill, so a draft or pending autosave never carries over to the next one.
  return <SkillEditor key={id} id={id} />;
}

function SkillEditor({ id }: { id: string }) {
  const [skill, setSkill] = useState<Skill>();
  const [versions, setVersions] = useState<SkillVersion[]>([]);
  const [contents, setContents] = useState<Record<string, SkillFile[]>>({});
  const [path, setPath] = useState(ENTRYPOINT);
  const [draft, setDraft] = useState<DraftFile[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  usePageCrumb(`/skills/${id}`, skill?.name);
  const load = useCallback(
    async (signal?: AbortSignal) => {
      const response = await skills.getSkill({ id }, { signal });
      if (signal?.aborted) return;
      setSkill(response.skill);
      setVersions(response.versions);
    },
    [id],
  );
  useEffect(() => {
    const abort = new AbortController();
    void load(abort.signal).catch((e) => {
      if (!abort.signal.aborted) setError(message(e, "Unable to load the skill."));
    });
    return () => abort.abort();
  }, [load]);
  const version = versions[0];
  // The latest version's files load on demand and stay cached; versions are immutable.
  useEffect(() => {
    const abort = new AbortController();
    if (version && !contents[version.id])
      void skills
        .getSkillVersion({ id: version.id }, { signal: abort.signal })
        .then((response) => {
          if (!abort.signal.aborted && response.version)
            setContents((current) => ({ ...current, [version.id]: response.version!.files }));
        })
        .catch((e) => {
          if (!abort.signal.aborted) setError(message(e, "Unable to load skill files."));
        });
    return () => abort.abort();
  }, [version, contents]);
  const editable = skill?.sourceKind === SkillSourceKind.EDITOR;
  const stored = version ? contents[version.id] : undefined;
  const files: DraftFile[] = useMemo(() => draft ?? (stored ?? []).map(toDraft), [draft, stored]);
  const file = files.find((f) => f.path === path) ?? files[0];
  // What was last loaded or saved: the draft is dirty only where it differs from this.
  const [baseline, setBaseline] = useState<DraftFile[]>([]);
  const dirty = useMemo(() => {
    if (!draft) return false;
    const saved = new Map(baseline.map((f) => [f.path, f]));
    return (
      draft.length !== baseline.length ||
      draft.some((f) => {
        const s = saved.get(f.path);
        return !s || f.data !== s.data || f.from !== s.from || f.content !== s.content;
      })
    );
  }, [draft, baseline]);
  // Editable skills open straight into editing on the files as loaded. Saving never replaces
  // the draft, so the editors stay mounted, editable and focused, and typing during a save is
  // kept.
  useEffect(() => {
    if (!editable || !stored || draft !== null) return;
    const fresh = stored.map(toDraft);
    setDraft(fresh);
    setBaseline(fresh);
  }, [editable, stored, draft]);
  const editing = draft !== null;
  function update(content: string) {
    if (!draft || !file) return;
    setDraft(draft.map((f) => (f.path === file.path ? { ...f, content } : f)));
  }
  const [folders, setFolders] = useState<string[]>([]);
  const fail = (text: string) => {
    setError(text);
    return false;
  };
  const blank = (at: string): DraftFile => ({
    path: at,
    content: "",
    binary: false,
    mediaType: "",
    sizeBytes: 0n,
    downloadUrl: "",
    executable: false,
  });
  function createFile(at: string) {
    if (!draft) return;
    if (!validPath(at)) return fail("File paths use letters, digits, '.', '_', '-' and '/'.");
    setError("");
    setDraft(sortByPath([...draft, blank(at)]));
    setPath(at);
  }
  // Moving a folder moves every file under it; stored binaries remember where they came from.
  // Every folder in the tree: those holding files and those created empty. Moving or deleting a
  // folder's last file keeps the folder, as VS Code does (until the page is reloaded, since
  // only files are stored).
  const allFolders = (list: DraftFile[]) => {
    const all = new Set(folders);
    for (const f of list)
      for (let at = f.path; at.includes("/");) all.add((at = at.slice(0, at.lastIndexOf("/"))));
    return [...all];
  };
  function rename(from: string, to: string, folder: boolean) {
    if (!draft) return;
    const moved = (at: string) =>
      folder
        ? at === from || at.startsWith(`${from}/`)
          ? to + at.slice(from.length)
          : at
        : at === from
          ? to
          : at;
    const next = draft.map((f) =>
      moved(f.path) === f.path ? f : { ...f, path: moved(f.path), from: f.from ?? f.path },
    );
    if (next.some((f, i) => next.findIndex((g) => g.path === f.path) !== i))
      return fail(`${to} already exists.`);
    if (!next.every((f) => validPath(f.path)))
      return fail("File paths use letters, digits, '.', '_', '-' and '/'.");
    setError("");
    setDraft(sortByPath(next));
    setFolders([...new Set(allFolders(draft).map(moved))]);
    setPath(moved(path));
  }
  function remove(at: string, folder: boolean) {
    if (!draft) return;
    const gone = (p: string) => (folder ? p === at || p.startsWith(`${at}/`) : p === at);
    setDraft(draft.filter((f) => !gone(f.path)));
    setFolders(allFolders(draft).filter((f) => !gone(f)));
    if (gone(path)) setPath(ENTRYPOINT);
  }
  // Uploads replace a file of the same path, as dropping onto VS Code's explorer does once
  // confirmed; text stays editable once saved.
  async function uploadFiles(folder: string, incoming: Incoming[]) {
    if (!draft) return;
    const added: DraftFile[] = [];
    for (const { path: relative, file: chosen } of incoming) {
      const at = folder ? `${folder}/${relative}` : relative;
      if (!validPath(at)) return fail(`${at} is not a valid file path.`);
      const data = new Uint8Array(await chosen.arrayBuffer());
      added.push({
        path: at,
        content: "",
        binary: true,
        data,
        mediaType: chosen.type || "application/octet-stream",
        sizeBytes: BigInt(data.length),
        downloadUrl: "",
        executable: false,
      });
    }
    setError("");
    const replaced = new Set(added.map((f) => f.path));
    setDraft(sortByPath([...draft.filter((f) => !replaced.has(f.path)), ...added]));
    if (added[0]) setPath(added[0].path);
  }
  function download(at: string) {
    const target = files.find((f) => f.path === at);
    if (!target) return;
    const href =
      target.binary && !target.data
        ? target.downloadUrl
        : URL.createObjectURL(new Blob([target.data?.slice() ?? target.content]));
    const link = document.createElement("a");
    link.href = href;
    link.download = at.split("/").pop()!;
    link.click();
    if (href.startsWith("blob:")) setTimeout(() => URL.revokeObjectURL(href), 1000);
  }
  /** Save these files as the skill's next version. */
  async function send(next: DraftFile[]) {
    setBusy(true);
    setError("");
    try {
      await skills.updateSkill({
        id,
        files: next.map((f) =>
          f.binary
            ? f.data
              ? { path: f.path, data: f.data, mediaType: f.mediaType }
              : { path: f.path, keep: true, ...(f.from && { keepPath: f.from }) }
            : { path: f.path, content: f.content, ...(f.executable && { executable: true }) },
        ),
      });
      // What was sent is now stored; edits made meanwhile stay dirty against it.
      setBaseline(next);
      setSaved(true);
      await load();
    } catch (e) {
      setError(message(e, "Unable to save the skill."));
    } finally {
      setBusy(false);
    }
  }
  const save = () => (draft ? send(draft) : Promise.resolve());
  // Documents save themselves a moment after the last change, like an editor with auto save.
  useEffect(() => {
    if (!dirty || busy || !draft) return;
    setSaved(false);
    const timer = setTimeout(() => void send(draft), 1000);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- send reads the draft it is given.
  }, [draft, dirty, busy]);
  const entry = files.find((f) => f.path === ENTRYPOINT);
  const heading = frontMatter(entry?.content ?? "");
  /**
   * Name and description live in SKILL.md's front matter and save with the rest of the draft; a
   * new name renames the skill.
   */
  function setField(key: "name" | "description", value: string) {
    setDraft(
      files.map((f) =>
        f.path === ENTRYPOINT ? { ...f, content: withFrontMatter(f.content, key, value) } : f,
      ),
    );
  }
  return (
    <section className="flex min-h-0 flex-1" aria-label={skill ? `Skill ${skill.name}` : "Skill"}>
      <aside className="flex w-64 shrink-0 flex-col gap-4 border-r pt-4 lg:pt-6">
        {skill && (
          <div className="space-y-1 px-4 lg:px-6">
            <div className="flex flex-wrap items-center gap-2">
              <SourceKindIcon kind={skill.sourceKind} />
              <KindLabel kind={skill.sourceKind} />
              {skill.sourceKind !== SkillSourceKind.EDITOR && (
                <Badge
                  variant="outline"
                  title="Catalog, Git and bundled skills follow their origin and are never edited here."
                >
                  Read-only
                </Badge>
              )}
            </div>
            <p className="m-0 text-sm text-muted-foreground">
              From <span className="font-medium text-foreground">{skill.sourceName}</span>
            </p>
          </div>
        )}
        {version && skill && (
          <SkillExplorer
            title={skill.name}
            files={files}
            folders={folders}
            selected={file?.path ?? ""}
            readOnly={!editing}
            locked={[ENTRYPOINT]}
            onOpen={setPath}
            onCreateFile={createFile}
            onCreateFolder={(at) => setFolders((current) => [...current, at])}
            onRename={rename}
            onDelete={remove}
            onUpload={(folder, incoming) => void uploadFiles(folder, incoming)}
            onDownload={download}
          />
        )}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 flex-wrap items-start justify-between gap-4 p-4 pb-3 lg:p-6 lg:pb-4">
          <div className="min-w-0 flex-1 space-y-1">
            {version && (
              <>
                <InlineField
                  label="name"
                  value={heading.name || skill?.name || ""}
                  editable={editable && !!entry && !busy}
                  validate={(v) =>
                    validName(v) || "Skill names use lowercase letters, digits, '-' or '_'."
                  }
                  onSave={(v) => setField("name", v)}
                  className="font-mono text-lg font-semibold"
                  as="h1"
                />
                <InlineField
                  label="description"
                  value={heading.description ?? version.description}
                  placeholder="No description"
                  editable={editable && !!entry && !busy}
                  multiline
                  onSave={(v) => setField("description", v)}
                  className="text-sm text-muted-foreground"
                />
              </>
            )}
          </div>
          {editable && (
            <Button className="ml-auto" onClick={() => void save()} disabled={busy || !dirty}>
              <SaveIcon />
              {busy ? "Saving…" : dirty ? "Save" : saved ? "Saved" : "Save"}
            </Button>
          )}
          {error && (
            <p role="alert" className="m-0 basis-full text-sm text-destructive">
              {error}
            </p>
          )}
        </div>
        {file && (
          <OpenFile
            file={file}
            editing={editing}
            onChange={update}
            hideFrontMatter={file.path === ENTRYPOINT}
          />
        )}
      </div>
    </section>
  );
}

/** Text that turns into an input behind a pencil, as on the agent Tools tab. */
function InlineField({
  label,
  value,
  placeholder,
  editable,
  multiline = false,
  validate,
  onSave,
  className,
  as: Tag = "p",
}: {
  label: string;
  value: string;
  placeholder?: string;
  editable: boolean;
  multiline?: boolean;
  validate?: (value: string) => true | string;
  onSave: (value: string) => Promise<unknown> | void;
  className?: string;
  as?: "h1" | "p";
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [invalid, setInvalid] = useState("");
  async function commit() {
    if (editing === null) return;
    const next = editing.trim();
    const checked = validate?.(next) ?? true;
    if (checked !== true) return setInvalid(checked);
    setEditing(null);
    setInvalid("");
    if (next !== value) await onSave(next);
  }
  if (editing !== null)
    return (
      <form
        className="flex items-start gap-1"
        onSubmit={(event) => {
          event.preventDefault();
          void commit();
        }}
      >
        <div className="min-w-0 flex-1">
          {multiline ? (
            <textarea
              aria-label={`Skill ${label}`}
              value={editing}
              autoFocus
              rows={3}
              onChange={(event) => setEditing(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") setEditing(null);
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void commit();
                }
              }}
              className="w-full resize-y rounded-md border border-input bg-transparent px-2 py-1 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
            />
          ) : (
            <Input
              aria-label={`Skill ${label}`}
              value={editing}
              autoFocus
              className="font-mono"
              onChange={(event) => setEditing(event.target.value)}
              onKeyDown={(event) => event.key === "Escape" && setEditing(null)}
            />
          )}
          {invalid && <p className="m-0 mt-1 text-xs text-destructive">{invalid}</p>}
        </div>
        <Button type="submit" size="icon-sm" variant="ghost" aria-label={`Save ${label}`}>
          <CheckIcon />
        </Button>
        <Button
          type="button"
          size="icon-sm"
          variant="ghost"
          aria-label={`Cancel ${label}`}
          onClick={() => {
            setEditing(null);
            setInvalid("");
          }}
        >
          <XIcon />
        </Button>
      </form>
    );
  return (
    <div className="group flex items-start gap-1">
      <Tag className={`m-0 min-w-0 ${className ?? ""}`}>{value || placeholder}</Tag>
      {editable && (
        <Button
          size="icon-xs"
          variant="ghost"
          aria-label={`Edit ${label}`}
          className="text-muted-foreground"
          onClick={() => setEditing(value)}
        >
          <PencilIcon />
        </Button>
      )}
    </div>
  );
}
