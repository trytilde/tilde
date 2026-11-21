// Ported from trytilde/common-js (packages/ui/src/components/product/skills/markdown-editor.tsx),
// restyled with the app's theme tokens, given a read-only mode for viewing, a selection bubble
// menu, task lists, tables, links, a raw Markdown mode and a save shortcut.
import Placeholder from "@tiptap/extension-placeholder";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import { TableKit } from "@tiptap/extension-table";
import { type Editor, EditorContent, useEditor, useEditorState } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";
import StarterKit from "@tiptap/starter-kit";
import {
  Bold,
  Code,
  FileCode2,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  Link2,
  List,
  ListChecks,
  ListOrdered,
  Minus,
  Quote,
  Redo,
  SquareCode,
  Strikethrough,
  Table2,
  Undo,
  Unlink,
} from "lucide-react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { Markdown } from "tiptap-markdown";
import { Button } from "./ui/button";
import { SlashCommands } from "./slash-commands";
import "./markdown-editor.css";

const markdownOf = (editor: Editor) =>
  (editor.storage as unknown as { markdown: { getMarkdown: () => string } }).markdown.getMarkdown();

export const isMac = () =>
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.userAgent);
/** Markdown compared without its whitespace, so only real rewrites count. */
// Compares Markdown ignoring formatting tiptap rewrites harmlessly: whitespace, table cell
// padding and separator rows, and `_` versus `*` emphasis.
const squash = (markdown: string) =>
  markdown
    .replace(/:?-{3,}:?/g, "---")
    .replace(/_/g, "*")
    .replace(/\s+/g, " ")
    .replace(/ ?\| ?/g, "|")
    .trim();
const REWRITE_WARNING =
  "Rich editing rewrites this Markdown: front matter, HTML, footnotes, reference links or list markers may change. Switch anyway?";
/** The save shortcut as the platform writes it. */
export const saveKey = () => (isMac() ? "⌘S" : "Ctrl+S");

/**
 * Rich Markdown editing through tiptap; `readOnly` renders the same document without chrome.
 * Markdown in and out: tables, task lists, links and code survive a round trip. Markdown that
 * would not (front matter, HTML, footnotes, `*` bullets) opens in the raw source view, and
 * switching it to the rich view asks first, so opening a document never rewrites it. `onSave`
 * runs on ⌘S / Ctrl+S in either mode.
 */
export function MarkdownEditor({
  value,
  onChange,
  onSave,
  readOnly = false,
  label,
  placeholder = "Write, or type / for blocks…",
  className = "",
  minHeight = "min-h-[300px]",
  contentClassName = "prose prose-sm dark:prose-invert max-w-none prose-code:before:content-none prose-code:after:content-none",
  autoFocus = false,
  bare = false,
}: {
  value: string;
  onChange?: (value: string) => void;
  onSave?: () => void;
  readOnly?: boolean;
  /** Accessible name of the editable area. */
  label?: string;
  placeholder?: string;
  className?: string;
  minHeight?: string;
  /** Typography of the document. */
  contentClassName?: string;
  autoFocus?: boolean;
  /** Without its own border, for editors that fill a pane. */
  bare?: boolean;
}) {
  // The markdown last handed to or from the editor. Tiptap normalizes markdown (list markers,
  // spacing), so comparing against its own output would reload, or report an edit, on open.
  const last = useRef(value);
  const save = useRef(onSave);
  save.current = onSave;
  const [source, setSource] = useState(false);
  // The Markdown being typed in the source view.
  const [raw, setRaw] = useState("");
  const [linking, setLinking] = useState(false);
  // Whether the rich view would rewrite the Markdown it was given.
  const [lossy, setLossy] = useState(false);
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        link: { openOnClick: false, autolink: true, defaultProtocol: "https" },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      TableKit.configure({ table: { resizable: false } }),
      Markdown.configure({ html: false, transformPastedText: true, transformCopiedText: true }),
      Placeholder.configure({ placeholder }),
      SlashCommands,
    ],
    content: value,
    editable: !readOnly,
    autofocus: autoFocus ? "end" : false,
    onUpdate: ({ editor }) => {
      const markdown = markdownOf(editor);
      // Only a real edit reports a change, never a reload or an editable switch.
      if (markdown === last.current) return;
      last.current = markdown;
      onChange?.(markdown);
    },
    editorProps: {
      attributes: {
        class: `markdown-editor-content ${contentClassName} focus:outline-none px-5 py-4 ${readOnly ? "" : minHeight}`,
        ...(label && { "aria-label": label }),
      },
      handleKeyDown: (_view, event) => {
        if (!(event.metaKey || event.ctrlKey)) return false;
        const key = event.key.toLowerCase();
        if (key === "s") save.current?.();
        else if (key === "k") setLinking(true);
        else return false;
        event.preventDefault();
        return true;
      },
    },
  });
  // On open, and whenever the value is replaced from outside, Markdown the rich view would rewrite
  // opens in the source view.
  const check = useCallback((editor: Editor) => {
    const rewrites = squash(markdownOf(editor)) !== squash(last.current);
    setLossy(rewrites);
    if (rewrites) {
      setRaw(last.current);
      setSource(true);
    }
  }, []);
  useEffect(() => {
    if (editor) check(editor);
  }, [editor, check]);
  useEffect(() => {
    if (!editor || value === last.current) return;
    last.current = value;
    editor.commands.setContent(value, { emitUpdate: false });
    check(editor);
  }, [value, editor, check]);
  useEffect(() => {
    editor?.setEditable(!readOnly, false);
  }, [readOnly, editor]);
  const chrome = !readOnly;
  return (
    <div
      className={`markdown-editor flex flex-col bg-background ${bare ? "" : "rounded-lg border"} ${className}`}
    >
      {chrome && (
        <MarkdownToolbar
          editor={editor}
          source={source}
          linking={linking}
          onLinking={setLinking}
          onSource={() => {
            if (!source) {
              setRaw(last.current);
              return setSource(true);
            }
            // Leaving the source view loads what was typed there, once agreed if it rewrites it.
            if (!editor) return;
            editor.commands.setContent(last.current, { emitUpdate: false });
            const rewrites = squash(markdownOf(editor)) !== squash(last.current);
            if (rewrites && !window.confirm(REWRITE_WARNING)) return;
            setLossy(false);
            setSource(false);
          }}
        />
      )}
      {chrome && source && lossy && (
        <p
          role="status"
          className="border-b bg-amber-500/10 px-4 py-1.5 text-[11px] text-muted-foreground"
        >
          Opened as Markdown: the rich view would rewrite parts of it (front matter, HTML, footnotes
          or list markers).
        </p>
      )}
      {source && chrome ? (
        <textarea
          aria-label={label ? `${label} (Markdown)` : "Markdown"}
          className={`w-full flex-1 resize-none bg-transparent px-5 py-4 font-mono text-xs leading-5 outline-none ${minHeight}`}
          spellCheck={false}
          value={raw}
          onChange={(event) => {
            setRaw(event.target.value);
            last.current = event.target.value;
            onChange?.(event.target.value);
          }}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
              event.preventDefault();
              save.current?.();
            }
          }}
        />
      ) : (
        <EditorContent editor={editor} className="flex-1" />
      )}
      {chrome && editor && !source && <SelectionMenu editor={editor} />}
      {chrome && (
        <div className="flex items-center gap-3 border-t bg-muted/40 px-3 py-1 font-mono text-[11px] text-muted-foreground">
          <span>
            <Kbd>/</Kbd> blocks · select text to format · <Kbd>{isMac() ? "⌘K" : "Ctrl+K"}</Kbd>{" "}
            link
            {onSave && (
              <>
                {" "}
                · <Kbd>{saveKey()}</Kbd> save
              </>
            )}
          </span>
          <WordCount editor={editor} source={source ? raw : undefined} />
        </div>
      )}
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border bg-background px-1 py-px font-mono text-[10px] text-muted-foreground">
      {children}
    </kbd>
  );
}

function WordCount({ editor, source }: { editor: Editor | null; source?: string }) {
  const text = useEditorState({ editor, selector: ({ editor }) => editor?.getText() ?? "" });
  const words = (source ?? text ?? "").split(/\s+/).filter(Boolean).length;
  return (
    <span className="ml-auto tabular-nums">
      {words} {words === 1 ? "word" : "words"}
    </span>
  );
}

/** Which marks and blocks the selection is in, so buttons show their state on every change. */
function useActive(editor: Editor | null) {
  return useEditorState({
    editor,
    selector: ({ editor }) =>
      editor
        ? {
            bold: editor.isActive("bold"),
            italic: editor.isActive("italic"),
            strike: editor.isActive("strike"),
            code: editor.isActive("code"),
            link: editor.isActive("link"),
            h1: editor.isActive("heading", { level: 1 }),
            h2: editor.isActive("heading", { level: 2 }),
            h3: editor.isActive("heading", { level: 3 }),
            bulletList: editor.isActive("bulletList"),
            orderedList: editor.isActive("orderedList"),
            taskList: editor.isActive("taskList"),
            codeBlock: editor.isActive("codeBlock"),
            blockquote: editor.isActive("blockquote"),
            table: editor.isActive("table"),
            canUndo: editor.can().undo(),
            canRedo: editor.can().redo(),
          }
        : undefined,
  });
}

/** Sets, changes or removes the link on the selection; an empty URL removes it. */
function applyLink(editor: Editor, href: string) {
  const chain = editor.chain().focus().extendMarkRange("link");
  if (!href.trim()) chain.unsetLink().run();
  else chain.setLink({ href: href.trim() }).run();
}

function MarkdownToolbar({
  editor,
  source,
  onSource,
  linking,
  onLinking: setLinking,
}: {
  editor: Editor | null;
  source: boolean;
  onSource: () => void;
  linking: boolean;
  onLinking: (linking: boolean) => void;
}) {
  const active = useActive(editor);
  if (!editor || !active)
    return <div className="h-9 border-b bg-muted/40" aria-label="Formatting" role="toolbar" />;
  const chain = () => editor.chain().focus();
  const off = source;
  return (
    <div
      role="toolbar"
      aria-label="Formatting"
      className="sticky top-0 z-10 flex flex-wrap items-center gap-0.5 border-b bg-muted/40 px-2 py-1 backdrop-blur"
    >
      <ToolbarButton
        onClick={() => chain().undo().run()}
        disabled={off || !active.canUndo}
        label="Undo"
      >
        <Undo />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().redo().run()}
        disabled={off || !active.canRedo}
        label="Redo"
      >
        <Redo />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 1 }).run()}
        active={active.h1}
        disabled={off}
        label="Heading 1"
      >
        <Heading1 />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 2 }).run()}
        active={active.h2}
        disabled={off}
        label="Heading 2"
      >
        <Heading2 />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 3 }).run()}
        active={active.h3}
        disabled={off}
        label="Heading 3"
      >
        <Heading3 />
      </ToolbarButton>
      <ToolbarSeparator />
      <MarkButtons editor={editor} active={active} disabled={off} />
      <ToolbarButton
        onClick={() => setLinking(true)}
        active={active.link}
        disabled={off}
        label="Link"
      >
        <Link2 />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().toggleBulletList().run()}
        active={active.bulletList}
        disabled={off}
        label="Bullet list"
      >
        <List />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleOrderedList().run()}
        active={active.orderedList}
        disabled={off}
        label="Numbered list"
      >
        <ListOrdered />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleTaskList().run()}
        active={active.taskList}
        disabled={off}
        label="Task list"
      >
        <ListChecks />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().toggleBlockquote().run()}
        active={active.blockquote}
        disabled={off}
        label="Quote"
      >
        <Quote />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleCodeBlock().run()}
        active={active.codeBlock}
        disabled={off}
        label="Code block"
      >
        <SquareCode />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
        active={active.table}
        disabled={off}
        label="Table"
      >
        <Table2 />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().setHorizontalRule().run()}
        disabled={off}
        label="Divider"
      >
        <Minus />
      </ToolbarButton>
      {active.table && !off && (
        <>
          <ToolbarSeparator />
          <TextButton onClick={() => chain().addRowAfter().run()}>+ Row</TextButton>
          <TextButton onClick={() => chain().addColumnAfter().run()}>+ Column</TextButton>
          <TextButton onClick={() => chain().deleteRow().run()}>− Row</TextButton>
          <TextButton onClick={() => chain().deleteColumn().run()}>− Column</TextButton>
          <TextButton onClick={() => chain().deleteTable().run()}>Delete table</TextButton>
        </>
      )}
      <div className="ml-auto" />
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={`h-7 gap-1.5 rounded-sm px-2 font-mono text-[11px] [&_svg]:size-3.5 ${source ? "bg-background shadow-sm" : ""}`}
        aria-pressed={source}
        title="Edit the raw Markdown"
        onClick={onSource}
      >
        <FileCode2 />
        Markdown
      </Button>
      {linking && (
        <LinkField editor={editor} className="basis-full pt-1" onDone={() => setLinking(false)} />
      )}
    </div>
  );
}

type Active = NonNullable<ReturnType<typeof useActive>>;
function MarkButtons({
  editor,
  active,
  disabled,
}: {
  editor: Editor;
  active: Active;
  disabled?: boolean;
}) {
  const chain = () => editor.chain().focus();
  return (
    <>
      <ToolbarButton
        onClick={() => chain().toggleBold().run()}
        active={active.bold}
        disabled={disabled}
        label="Bold"
      >
        <Bold />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleItalic().run()}
        active={active.italic}
        disabled={disabled}
        label="Italic"
      >
        <Italic />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleStrike().run()}
        active={active.strike}
        disabled={disabled}
        label="Strikethrough"
      >
        <Strikethrough />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleCode().run()}
        active={active.code}
        disabled={disabled}
        label="Inline code"
      >
        <Code />
      </ToolbarButton>
    </>
  );
}

/** Formatting for selected text, floating above it. */
function SelectionMenu({ editor }: { editor: Editor }) {
  const active = useActive(editor);
  const [linking, setLinking] = useState(false);
  if (!active) return null;
  return (
    <BubbleMenu
      editor={editor}
      options={{ placement: "top", offset: 6 }}
      shouldShow={({ editor, from, to }) =>
        from !== to && editor.isEditable && !editor.isActive("codeBlock")
      }
      className="z-50 flex items-center gap-0.5 rounded-md border bg-popover p-0.5 text-popover-foreground shadow-md"
    >
      {linking ? (
        <LinkField editor={editor} onDone={() => setLinking(false)} />
      ) : (
        <>
          <MarkButtons editor={editor} active={active} />
          <ToolbarButton onClick={() => setLinking(true)} active={active.link} label="Link">
            <Link2 />
          </ToolbarButton>
          {active.link && (
            <ToolbarButton
              onClick={() => editor.chain().focus().extendMarkRange("link").unsetLink().run()}
              label="Remove link"
            >
              <Unlink />
            </ToolbarButton>
          )}
        </>
      )}
    </BubbleMenu>
  );
}

/** A URL or wiki path for the selection's link; Enter applies, Escape cancels. */
function LinkField({
  editor,
  onDone,
  className = "",
}: {
  editor: Editor;
  onDone: () => void;
  className?: string;
}) {
  const [href, setHref] = useState(
    () => (editor.getAttributes("link").href as string | undefined) ?? "",
  );
  return (
    <div className={`flex items-center gap-1 ${className}`}>
      <input
        autoFocus
        aria-label="Link URL"
        placeholder="https://… or a wiki path like runbooks/deploys"
        className="h-7 w-72 rounded-sm border bg-background px-2 font-mono text-[11px] outline-none focus:ring-2 focus:ring-ring/40"
        value={href}
        onChange={(event) => setHref(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyLink(editor, href);
            onDone();
          }
          if (event.key === "Escape") {
            event.preventDefault();
            editor.commands.focus();
            onDone();
          }
        }}
      />
      <TextButton
        onClick={() => {
          applyLink(editor, href);
          onDone();
        }}
      >
        {href.trim() ? "Apply" : "Remove"}
      </TextButton>
    </div>
  );
}

function ToolbarButton({
  onClick,
  active,
  disabled,
  label,
  children,
}: {
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  label: string;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      className={`size-7 rounded-sm [&_svg]:size-3.5 ${active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground"}`}
      onClick={onClick}
      onMouseDown={(event) => event.preventDefault()}
      disabled={disabled}
      aria-label={label}
      title={label}
      aria-pressed={active}
    >
      {children}
    </Button>
  );
}

function TextButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      className="h-7 rounded-sm px-2 font-mono text-[11px]"
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

function ToolbarSeparator() {
  return <div aria-hidden className="mx-1 h-4 w-px bg-border" />;
}
