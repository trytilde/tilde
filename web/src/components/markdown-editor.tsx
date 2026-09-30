// Ported from trytilde/common-js (packages/ui/src/components/product/skills/markdown-editor.tsx),
// restyled with the app's theme tokens and given a read-only mode for viewing.
import Placeholder from "@tiptap/extension-placeholder";
import { type Editor, EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import {
  Bold,
  Code,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  List,
  ListOrdered,
  Minus,
  Quote,
  Redo,
  SquareCode,
  Strikethrough,
  Undo,
} from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { Markdown } from "tiptap-markdown";
import { Button } from "./ui/button";
import { SlashCommands } from "./slash-commands";

const markdownOf = (editor: Editor) =>
  (editor.storage as unknown as { markdown: { getMarkdown: () => string } }).markdown.getMarkdown();

/** Rich markdown editing through tiptap; `readOnly` renders the same document without chrome. */
export function MarkdownEditor({
  value,
  onChange,
  readOnly = false,
  label,
  placeholder = "Write markdown…",
  className = "",
  minHeight = "min-h-[300px]",
}: {
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  /** Accessible name of the editable area. */
  label?: string;
  placeholder?: string;
  className?: string;
  minHeight?: string;
}) {
  // The markdown last handed to or from the editor. Tiptap normalizes markdown (list markers,
  // spacing), so comparing against its own output would reload, or report an edit, on open.
  const last = useRef(value);
  const editor = useEditor({
    extensions: [
      StarterKit,
      Markdown.configure({ html: false, transformPastedText: true, transformCopiedText: true }),
      Placeholder.configure({ placeholder }),
      SlashCommands,
    ],
    content: value,
    editable: !readOnly,
    onUpdate: ({ editor }) => {
      const markdown = markdownOf(editor);
      // Only a real edit reports a change, never a reload or an editable switch.
      if (markdown === last.current) return;
      last.current = markdown;
      onChange?.(markdown);
    },
    editorProps: {
      attributes: {
        class: `prose prose-sm dark:prose-invert max-w-none prose-code:before:content-none prose-code:after:content-none focus:outline-none px-5 py-5 ${readOnly ? "" : minHeight}`,
        ...(label && { "aria-label": label }),
      },
    },
  });
  useEffect(() => {
    if (!editor || value === last.current) return;
    last.current = value;
    editor.commands.setContent(value, { emitUpdate: false });
  }, [value, editor]);
  useEffect(() => {
    editor?.setEditable(!readOnly, false);
  }, [readOnly, editor]);
  return (
    <div className={`rounded-lg border bg-background ${className}`}>
      {!readOnly && <MarkdownToolbar editor={editor} />}
      <EditorContent editor={editor} />
      {!readOnly && (
        <div className="border-t bg-muted/40 px-3 py-1.5 font-mono text-[11px] text-muted-foreground">
          Type <kbd className="rounded border bg-background px-1 py-0.5 text-[10px]">/</kbd> to open
          the quick format menu.
        </div>
      )}
    </div>
  );
}

function MarkdownToolbar({ editor }: { editor: Editor | null }) {
  if (!editor) return <div className="flex items-center gap-1 border-b bg-muted/40 px-2 py-1.5" />;
  const chain = () => editor.chain().focus();
  return (
    <div className="flex flex-wrap items-center gap-0.5 border-b bg-muted/40 px-2 py-1.5">
      <ToolbarButton
        onClick={() => chain().toggleBold().run()}
        active={editor.isActive("bold")}
        disabled={!editor.can().chain().focus().toggleBold().run()}
        ariaLabel="Bold"
      >
        <Bold />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleItalic().run()}
        active={editor.isActive("italic")}
        disabled={!editor.can().chain().focus().toggleItalic().run()}
        ariaLabel="Italic"
      >
        <Italic />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleStrike().run()}
        active={editor.isActive("strike")}
        disabled={!editor.can().chain().focus().toggleStrike().run()}
        ariaLabel="Strikethrough"
      >
        <Strikethrough />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleCode().run()}
        active={editor.isActive("code")}
        disabled={!editor.can().chain().focus().toggleCode().run()}
        ariaLabel="Inline code"
      >
        <Code />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 1 }).run()}
        active={editor.isActive("heading", { level: 1 })}
        ariaLabel="Heading 1"
      >
        <Heading1 />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 2 }).run()}
        active={editor.isActive("heading", { level: 2 })}
        ariaLabel="Heading 2"
      >
        <Heading2 />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleHeading({ level: 3 }).run()}
        active={editor.isActive("heading", { level: 3 })}
        ariaLabel="Heading 3"
      >
        <Heading3 />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().toggleBulletList().run()}
        active={editor.isActive("bulletList")}
        ariaLabel="Bullet list"
      >
        <List />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleOrderedList().run()}
        active={editor.isActive("orderedList")}
        ariaLabel="Numbered list"
      >
        <ListOrdered />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleCodeBlock().run()}
        active={editor.isActive("codeBlock")}
        ariaLabel="Code block"
      >
        <SquareCode />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().toggleBlockquote().run()}
        active={editor.isActive("blockquote")}
        ariaLabel="Quote"
      >
        <Quote />
      </ToolbarButton>
      <ToolbarButton onClick={() => chain().setHorizontalRule().run()} ariaLabel="Horizontal rule">
        <Minus />
      </ToolbarButton>
      <ToolbarSeparator />
      <ToolbarButton
        onClick={() => chain().undo().run()}
        disabled={!editor.can().chain().focus().undo().run()}
        ariaLabel="Undo"
      >
        <Undo />
      </ToolbarButton>
      <ToolbarButton
        onClick={() => chain().redo().run()}
        disabled={!editor.can().chain().focus().redo().run()}
        ariaLabel="Redo"
      >
        <Redo />
      </ToolbarButton>
    </div>
  );
}

function ToolbarButton({
  onClick,
  active,
  disabled,
  ariaLabel,
  children,
}: {
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  ariaLabel: string;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      className={`rounded-sm [&_svg]:size-3.5 ${active ? "bg-background shadow-sm" : ""}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={ariaLabel}
      aria-pressed={active}
    >
      {children}
    </Button>
  );
}

function ToolbarSeparator() {
  return <div className="mx-1 h-5 w-px bg-border" />;
}
