import { useEffect, useMemo, useRef, useState, type DragEvent, type KeyboardEvent } from "react";
import { cn } from "cn";
import { getIcon } from "material-file-icons";
import {
  ChevronRightIcon,
  ChevronsDownUpIcon,
  ClipboardCopyIcon,
  DownloadIcon,
  FilePlusIcon,
  FolderPlusIcon,
  PencilIcon,
  Trash2Icon,
  UploadIcon,
} from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from "./ui/context-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";

/** A file handed to the explorer: its path within the skill. */
type ExplorerFile = { path: string };
/** A dropped or chosen file and where under the target folder it goes (folders keep their shape). */
export type Incoming = { path: string; file: File };

type Node = { name: string; path: string; folder: boolean; children: Node[] };
type Editing =
  | { kind: "file" | "folder"; parent: string }
  | { kind: "rename"; path: string; folder: boolean };

const ROW = 22;
const INDENT = 8;
const DRAG_TYPE = "application/x-tilde-skill-path";
const dirname = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");
const basename = (path: string) => path.slice(path.lastIndexOf("/") + 1);
const join = (folder: string, name: string) => (folder ? `${folder}/${name}` : name);

/** Nest paths into folders (listed first) and files, each sorted like VS Code, case-insensitively. */
function build(files: readonly ExplorerFile[], folders: readonly string[]): Node[] {
  const root: Node = { name: "", path: "", folder: true, children: [] };
  const ensure = (path: string): Node => {
    let at = root;
    for (const part of path.split("/")) {
      const next = join(at.path, part);
      let child = at.children.find((c) => c.folder && c.path === next);
      if (!child) {
        child = { name: part, path: next, folder: true, children: [] };
        at.children.push(child);
      }
      at = child;
    }
    return at;
  };
  for (const folder of folders) if (folder) ensure(folder);
  for (const { path } of files) {
    const parent = dirname(path) ? ensure(dirname(path)) : root;
    parent.children.push({ name: basename(path), path, folder: false, children: [] });
  }
  const sort = (nodes: Node[]) => {
    nodes.sort(
      (a, b) =>
        Number(b.folder) - Number(a.folder) ||
        a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
    );
    for (const node of nodes) sort(node.children);
    return nodes;
  };
  return sort(root.children);
}

/** Files (and folders, recursively) from an OS drop, keeping their relative paths. */
async function dropped(event: DragEvent): Promise<Incoming[]> {
  const entries = [...event.dataTransfer.items]
    .map((item) => item.webkitGetAsEntry?.())
    .filter((entry): entry is FileSystemEntry => !!entry);
  if (!entries.length)
    return [...event.dataTransfer.files].map((file) => ({ path: file.name, file }));
  const out: Incoming[] = [];
  const walk = async (entry: FileSystemEntry, prefix: string): Promise<void> => {
    if (entry.isFile) {
      const file = await new Promise<File>((resolve, reject) =>
        (entry as FileSystemFileEntry).file(resolve, reject),
      );
      out.push({ path: join(prefix, entry.name), file });
      return;
    }
    const reader = (entry as FileSystemDirectoryEntry).createReader();
    // readEntries returns batches until an empty one.
    for (;;) {
      const batch = await new Promise<FileSystemEntry[]>((resolve, reject) =>
        reader.readEntries(resolve, reject),
      );
      if (!batch.length) break;
      for (const child of batch) await walk(child, join(prefix, entry.name));
    }
  };
  for (const entry of entries) await walk(entry, "");
  return out;
}

function FileIcon({ name }: { name: string }) {
  // Material file icons, by name then extension, as the VS Code icon theme of the same name.
  return (
    <span
      aria-hidden="true"
      className="flex size-4 shrink-0 items-center justify-center [&>svg]:size-4"
      dangerouslySetInnerHTML={{ __html: getIcon(name).svg }}
    />
  );
}

/**
 * A skill's files as VS Code's Explorer: compact rows with indent guides and file-type icons,
 * inline create and rename (F2), delete (Delete), keyboard navigation, a right-click menu, drag
 * to move, and drop from the desktop to upload (folders included). Folders exist only as paths
 * of their files, so an empty folder lives in `folders` until something is put in it. Nothing
 * here saves: every change is reported to the owner, which keeps the draft.
 */
export function SkillExplorer({
  title,
  files,
  folders,
  selected,
  readOnly,
  locked,
  onOpen,
  onCreateFile,
  onCreateFolder,
  onRename,
  onDelete,
  onUpload,
  onDownload,
}: {
  title: string;
  files: readonly ExplorerFile[];
  folders: readonly string[];
  selected: string;
  readOnly: boolean;
  /** Paths that cannot be renamed, moved or deleted (SKILL.md). */
  locked: readonly string[];
  onOpen: (path: string) => void;
  onCreateFile: (path: string) => void;
  onCreateFolder: (path: string) => void;
  /** A file or a folder (with everything in it) moves or is renamed. */
  onRename: (from: string, to: string, folder: boolean) => void;
  onDelete: (path: string, folder: boolean) => void;
  onUpload: (folder: string, incoming: Incoming[]) => void;
  onDownload?: (path: string) => void;
}) {
  const nodes = useMemo(() => build(files, folders), [files, folders]);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [focused, setFocused] = useState(selected);
  const [editing, setEditing] = useState<Editing>();
  const [target, setTarget] = useState<{ path: string; folder: boolean }>();
  const [dropFolder, setDropFolder] = useState<string>();
  const upload = useRef<HTMLInputElement>(null);
  const uploadInto = useRef("");
  const expandTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const exists = useMemo(() => {
    const all = new Set<string>();
    const walk = (list: Node[]) => {
      for (const node of list) {
        all.add(node.path);
        walk(node.children);
      }
    };
    walk(nodes);
    return all;
  }, [nodes]);
  useEffect(() => setFocused(selected), [selected]);

  // Rows in display order, for keyboard navigation.
  const visible = useMemo(() => {
    const rows: { node: Node; depth: number }[] = [];
    const walk = (list: Node[], depth: number) => {
      for (const node of list) {
        rows.push({ node, depth });
        if (node.folder && !collapsed.has(node.path)) walk(node.children, depth + 1);
      }
    };
    walk(nodes, 0);
    return rows;
  }, [nodes, collapsed]);

  const toggle = (path: string, open?: boolean) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (open ?? next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  const folderOf = (at?: { path: string; folder: boolean }) =>
    !at ? "" : at.folder ? at.path : dirname(at.path);
  const startCreate = (kind: "file" | "folder", at?: { path: string; folder: boolean }) => {
    const parent = folderOf(at);
    if (parent) toggle(parent, true);
    setEditing({ kind, parent });
  };
  const chooseUpload = (at?: { path: string; folder: boolean }) => {
    uploadInto.current = folderOf(at);
    upload.current?.click();
  };
  const collapseAll = () =>
    setCollapsed(new Set([...exists].filter((p) => !files.some((f) => f.path === p))));
  const movable = (path: string) => !readOnly && !locked.includes(path);
  const copy = (text: string) => void navigator.clipboard?.writeText(text);

  function onKeyDown(event: KeyboardEvent) {
    if (editing) return;
    const index = visible.findIndex((row) => row.node.path === focused);
    const row = visible[index];
    const move = (to: number) => {
      const next = visible[Math.max(0, Math.min(visible.length - 1, to))];
      if (next) setFocused(next.node.path);
    };
    switch (event.key) {
      case "ArrowDown":
        move(index + 1);
        break;
      case "ArrowUp":
        move(index - 1);
        break;
      case "ArrowRight":
        if (row?.node.folder) {
          if (collapsed.has(row.node.path)) toggle(row.node.path, true);
          else move(index + 1);
        }
        break;
      case "ArrowLeft":
        if (row?.node.folder && !collapsed.has(row.node.path)) toggle(row.node.path, false);
        else if (row && dirname(row.node.path)) setFocused(dirname(row.node.path));
        break;
      case "Enter":
      case " ":
        if (!row) return;
        if (row.node.folder) toggle(row.node.path);
        else onOpen(row.node.path);
        break;
      case "F2":
        if (row && movable(row.node.path))
          setEditing({ kind: "rename", path: row.node.path, folder: row.node.folder });
        break;
      case "Delete":
        if (row && movable(row.node.path)) onDelete(row.node.path, row.node.folder);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // Drag and drop: rows move within the skill; files and folders from the desktop upload. A
  // file row targets its folder, as in VS Code.
  const dropTarget = (node?: Node) => (!node ? "" : node.folder ? node.path : dirname(node.path));
  function onDragOver(event: DragEvent, node?: Node) {
    if (readOnly) return;
    event.preventDefault();
    event.stopPropagation();
    const folder = dropTarget(node);
    event.dataTransfer.dropEffect = event.dataTransfer.types.includes(DRAG_TYPE) ? "move" : "copy";
    if (folder !== dropFolder) {
      setDropFolder(folder);
      clearTimeout(expandTimer.current);
      // Hovering a closed folder opens it, so a drag can reach deeper folders.
      if (node?.folder && collapsed.has(node.path))
        expandTimer.current = setTimeout(() => toggle(node.path, true), 600);
    }
  }
  async function onDrop(event: DragEvent, node?: Node) {
    if (readOnly) return;
    event.preventDefault();
    event.stopPropagation();
    clearTimeout(expandTimer.current);
    const folder = dropTarget(node);
    setDropFolder(undefined);
    const moved = event.dataTransfer.getData(DRAG_TYPE);
    if (moved) {
      const isFolder = !files.some((f) => f.path === moved);
      const to = join(folder, basename(moved));
      // Not onto itself, nor a folder into its own subtree.
      if (to === moved || (isFolder && (folder + "/").startsWith(moved + "/"))) return;
      onRename(moved, to, isFolder);
      return;
    }
    const incoming = await dropped(event);
    if (incoming.length) onUpload(folder, incoming);
  }

  function row(node: Node, depth: number) {
    const open = node.folder && !collapsed.has(node.path);
    const renaming = editing?.kind === "rename" && editing.path === node.path;
    const active = node.path === selected && !node.folder;
    const isFocused = node.path === focused;
    return (
      <li key={node.path} role="none">
        {renaming ? (
          <NameInput
            depth={depth}
            folder={node.folder}
            initial={node.name}
            validate={(name) => {
              const to = join(dirname(node.path), name);
              return to !== node.path && exists.has(to)
                ? `A file or folder ${name} already exists at this location.`
                : undefined;
            }}
            onDone={(name) => {
              setEditing(undefined);
              const to = name && join(dirname(node.path), name);
              if (to && to !== node.path) onRename(node.path, to, node.folder);
            }}
          />
        ) : (
          <div
            role="treeitem"
            id={`explorer-${node.path}`}
            aria-level={depth + 1}
            aria-expanded={node.folder ? open : undefined}
            aria-selected={active}
            title={node.path}
            draggable={movable(node.path)}
            onDragStart={(event) => {
              event.dataTransfer.setData(DRAG_TYPE, node.path);
              event.dataTransfer.effectAllowed = "move";
            }}
            onDragOver={(event) => onDragOver(event, node)}
            onDrop={(event) => void onDrop(event, node)}
            onClick={() => {
              setFocused(node.path);
              if (node.folder) toggle(node.path);
              else onOpen(node.path);
            }}
            onContextMenu={() => {
              setFocused(node.path);
              setTarget({ path: node.path, folder: node.folder });
            }}
            style={{ height: ROW, paddingLeft: depth * INDENT * 2 + INDENT }}
            className={cn(
              "relative flex cursor-pointer items-center gap-1.5 pr-2 text-[13px] leading-[22px] outline-none hover:bg-muted/70",
              active && "bg-accent text-accent-foreground hover:bg-accent",
              isFocused && "ring-1 ring-ring ring-inset",
              node.folder && dropFolder === node.path && "bg-primary/10",
            )}
          >
            <Guides depth={depth} />
            {node.folder ? (
              <ChevronRightIcon
                aria-hidden="true"
                className={cn(
                  "size-4 shrink-0 text-muted-foreground transition-transform duration-100",
                  open && "rotate-90",
                )}
              />
            ) : (
              <FileIcon name={node.name} />
            )}
            <span className="min-w-0 truncate">{node.name}</span>
          </div>
        )}
        {node.folder && open && (
          <ul
            role="group"
            className={cn("m-0 list-none p-0", dropFolder === node.path && "bg-primary/10")}
          >
            {editing && editing.kind !== "rename" && editing.parent === node.path && (
              <li role="none">{create(depth + 1)}</li>
            )}
            {node.children.map((child) => row(child, depth + 1))}
          </ul>
        )}
      </li>
    );
  }

  function create(depth: number) {
    if (!editing || editing.kind === "rename") return null;
    const { kind, parent } = editing;
    return (
      <NameInput
        depth={depth}
        folder={kind === "folder"}
        initial=""
        validate={(name) =>
          exists.has(join(parent, name))
            ? `A file or folder ${name} already exists at this location.`
            : undefined
        }
        onDone={(name) => {
          setEditing(undefined);
          if (!name) return;
          const path = join(parent, name);
          if (kind === "folder") onCreateFolder(path);
          else onCreateFile(path);
        }}
      />
    );
  }

  const menuItems = () => {
    const at = target;
    const folderish = !at || at.folder;
    const canChange = at && movable(at.path);
    return (
      <>
        {!readOnly && folderish && (
          <>
            <ContextMenuItem onClick={() => startCreate("file", at)}>
              <FilePlusIcon />
              New File…
            </ContextMenuItem>
            <ContextMenuItem onClick={() => startCreate("folder", at)}>
              <FolderPlusIcon />
              New Folder…
            </ContextMenuItem>
            <ContextMenuItem onClick={() => chooseUpload(at)}>
              <UploadIcon />
              Upload Files…
            </ContextMenuItem>
            {at && <ContextMenuSeparator />}
          </>
        )}
        {at && !at.folder && onDownload && (
          <>
            <ContextMenuItem onClick={() => onDownload(at.path)}>
              <DownloadIcon />
              Download
            </ContextMenuItem>
            <ContextMenuSeparator />
          </>
        )}
        {at && (
          <>
            <ContextMenuItem onClick={() => copy(at.path)}>
              <ClipboardCopyIcon />
              Copy Relative Path
            </ContextMenuItem>
            <ContextMenuItem onClick={() => copy(basename(at.path))}>
              <ClipboardCopyIcon />
              Copy Name
            </ContextMenuItem>
          </>
        )}
        {canChange && (
          <>
            <ContextMenuSeparator />
            <ContextMenuItem
              onClick={() => setEditing({ kind: "rename", path: at.path, folder: at.folder })}
            >
              <PencilIcon />
              Rename…
              <ContextMenuShortcut>F2</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem variant="destructive" onClick={() => onDelete(at.path, at.folder)}>
              <Trash2Icon />
              Delete
              <ContextMenuShortcut>Del</ContextMenuShortcut>
            </ContextMenuItem>
          </>
        )}
        {!at && (
          <>
            {!readOnly && <ContextMenuSeparator />}
            <ContextMenuItem onClick={collapseAll}>
              <ChevronsDownUpIcon />
              Collapse Folders in Explorer
            </ContextMenuItem>
          </>
        )}
      </>
    );
  };

  return (
    <section aria-label="Explorer" className="group/explorer flex min-h-0 flex-1 flex-col">
      <div className="flex h-[22px] shrink-0 items-center justify-between pr-1 pl-2">
        <h3 className="m-0 truncate text-[11px] font-bold tracking-wide text-muted-foreground uppercase">
          {title}
        </h3>
        <span className="flex items-center opacity-0 transition-opacity group-focus-within/explorer:opacity-100 group-hover/explorer:opacity-100">
          {!readOnly && (
            <>
              <HeaderAction label="New File…" onClick={() => startCreate("file")}>
                <FilePlusIcon />
              </HeaderAction>
              <HeaderAction label="New Folder…" onClick={() => startCreate("folder")}>
                <FolderPlusIcon />
              </HeaderAction>
              <HeaderAction label="Upload Files…" onClick={() => chooseUpload()}>
                <UploadIcon />
              </HeaderAction>
            </>
          )}
          <HeaderAction label="Collapse Folders in Explorer" onClick={collapseAll}>
            <ChevronsDownUpIcon />
          </HeaderAction>
        </span>
      </div>
      <input
        ref={upload}
        type="file"
        multiple
        aria-label="Files to upload"
        className="hidden"
        onChange={(event) => {
          const chosen = [...(event.target.files ?? [])];
          event.target.value = "";
          if (chosen.length)
            onUpload(
              uploadInto.current,
              chosen.map((file) => ({ path: file.name, file })),
            );
        }}
      />
      <ContextMenu>
        <ContextMenuTrigger
          render={
            <div
              role="tree"
              aria-label="Files"
              aria-activedescendant={focused ? `explorer-${focused}` : undefined}
              tabIndex={0}
              onKeyDown={onKeyDown}
              onContextMenu={(event) => {
                // Rows set their own target first; the empty area below targets the root.
                if (event.target === event.currentTarget) setTarget(undefined);
              }}
              onDragOver={(event) => onDragOver(event)}
              onDragLeave={(event) => {
                if (!event.currentTarget.contains(event.relatedTarget as globalThis.Node))
                  setDropFolder(undefined);
              }}
              onDrop={(event) => void onDrop(event)}
              className={cn(
                "group/tree min-h-24 flex-1 overflow-y-auto pb-6 outline-none",
                dropFolder === "" && "bg-primary/10",
              )}
            />
          }
        >
          <ul role="none" className="m-0 list-none p-0">
            {editing && editing.kind !== "rename" && editing.parent === "" && (
              <li role="none">{create(0)}</li>
            )}
            {nodes.map((node) => row(node, 0))}
          </ul>
        </ContextMenuTrigger>
        <ContextMenuContent>{menuItems()}</ContextMenuContent>
      </ContextMenu>
    </section>
  );
}

function Guides({ depth }: { depth: number }) {
  // Indent guides show while the pointer is over the tree, as VS Code's default.
  return Array.from({ length: depth }, (_, level) => (
    <span
      key={level}
      aria-hidden="true"
      style={{ left: level * INDENT * 2 + INDENT + 7 }}
      className="absolute inset-y-0 w-px bg-border opacity-0 transition-opacity group-hover/tree:opacity-100"
    />
  ));
}

function HeaderAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-label={label}
            onClick={onClick}
            className="flex size-[22px] cursor-pointer items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground [&_svg]:size-4"
          />
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/**
 * VS Code's inline name box: Enter or leaving it commits, Escape cancels, and a rename selects
 * the name without its extension. A name with slashes makes the folders in between.
 */
function NameInput({
  depth,
  folder,
  initial,
  validate,
  onDone,
}: {
  depth: number;
  folder: boolean;
  initial: string;
  validate: (name: string) => string | undefined;
  onDone: (name: string) => void;
}) {
  const [value, setValue] = useState(initial);
  const input = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    const el = input.current;
    if (!el) return;
    el.focus();
    const dot = folder ? -1 : initial.lastIndexOf(".");
    el.setSelectionRange(0, dot > 0 ? dot : initial.length);
  }, [folder, initial]);
  const name = value.trim().replace(/^\/+|\/+$/g, "");
  const invalid =
    name && !/^[\w.-]+(\/[\w.-]+)*$/.test(name)
      ? "Names use letters, digits, '.', '_' and '-'."
      : name
        ? validate(name)
        : undefined;
  const finish = (commit: boolean) => {
    if (done.current) return;
    done.current = true;
    onDone(commit && !invalid ? name : "");
  };
  return (
    <div
      style={{ height: ROW, paddingLeft: depth * INDENT * 2 + INDENT }}
      className="relative flex items-center gap-1.5 pr-2"
    >
      <Guides depth={depth} />
      {folder ? (
        <ChevronRightIcon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
      ) : (
        <FileIcon name={name || "file"} />
      )}
      <div className="relative min-w-0 flex-1">
        <input
          ref={input}
          aria-label={folder ? "Folder name" : "File name"}
          aria-invalid={!!invalid}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === "Enter") finish(true);
            if (event.key === "Escape") finish(false);
          }}
          onBlur={() => finish(true)}
          className={cn(
            "h-[20px] w-full rounded-none border border-ring bg-background px-1 text-[13px] outline-none",
            invalid && "border-destructive",
          )}
        />
        {invalid && (
          <p
            role="alert"
            className="absolute top-full right-0 left-0 z-10 m-0 border border-destructive bg-background px-1.5 py-1 text-xs text-destructive"
          >
            {invalid}
          </p>
        )}
      </div>
    </div>
  );
}
