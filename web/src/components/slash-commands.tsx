// Ported from trytilde/common-js (packages/ui/src/components/product/skills/slash-commands.tsx).
import { type Editor, Extension, type Range, ReactRenderer } from "@tiptap/react";
import Suggestion, { type SuggestionKeyDownProps, type SuggestionProps } from "@tiptap/suggestion";
import {
  Code,
  Heading1,
  Heading2,
  Heading3,
  List,
  ListOrdered,
  type LucideIcon,
  Minus,
  Quote,
  SquareCode,
  Type,
} from "lucide-react";
import { forwardRef, type ReactNode, useEffect, useImperativeHandle, useState } from "react";
import tippy, { type Instance as TippyInstance, type Props as TippyProps } from "tippy.js";

type SlashItem = {
  title: string;
  icon: LucideIcon;
  keywords: string[];
  apply: (args: { editor: Editor; range: Range }) => void;
};

const ITEMS: SlashItem[] = [
  {
    title: "Text",
    icon: Type,
    keywords: ["paragraph", "plain"],
    apply: ({ editor, range }) => editor.chain().focus().deleteRange(range).setParagraph().run(),
  },
  {
    title: "Heading 1",
    icon: Heading1,
    keywords: ["h1", "title"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).setNode("heading", { level: 1 }).run(),
  },
  {
    title: "Heading 2",
    icon: Heading2,
    keywords: ["h2", "subtitle"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).setNode("heading", { level: 2 }).run(),
  },
  {
    title: "Heading 3",
    icon: Heading3,
    keywords: ["h3"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).setNode("heading", { level: 3 }).run(),
  },
  {
    title: "Bullet list",
    icon: List,
    keywords: ["bullet", "list"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).toggleBulletList().run(),
  },
  {
    title: "Numbered list",
    icon: ListOrdered,
    keywords: ["ordered", "numbered"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).toggleOrderedList().run(),
  },
  {
    title: "Quote",
    icon: Quote,
    keywords: ["blockquote", "quote"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).toggleBlockquote().run(),
  },
  {
    title: "Code block",
    icon: SquareCode,
    keywords: ["code", "fence"],
    apply: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleCodeBlock().run(),
  },
  {
    title: "Inline code",
    icon: Code,
    keywords: ["inline", "code"],
    apply: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleCode().run(),
  },
  {
    title: "Divider",
    icon: Minus,
    keywords: ["divider", "rule", "hr"],
    apply: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).setHorizontalRule().run(),
  },
];

function filterItems(query: string): SlashItem[] {
  if (!query) return ITEMS;
  const lowered = query.toLowerCase();
  return ITEMS.filter(
    (item) =>
      item.title.toLowerCase().includes(lowered) ||
      item.keywords.some((keyword) => keyword.includes(lowered)),
  );
}

type SlashMenuRef = {
  onKeyDown: (props: SuggestionKeyDownProps) => boolean;
};

const SlashMenu = forwardRef<SlashMenuRef, SuggestionProps<SlashItem>>(function SlashMenu(
  { items, command },
  ref,
) {
  const [selected, setSelected] = useState(0);

  useEffect(() => {
    setSelected(0);
  }, [items.length]);

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (event.key === "ArrowDown") {
        setSelected((value) => (value + 1) % Math.max(items.length, 1));
        return true;
      }
      if (event.key === "ArrowUp") {
        setSelected((value) => (value - 1 + Math.max(items.length, 1)) % Math.max(items.length, 1));
        return true;
      }
      if (event.key === "Enter") {
        const item = items[selected];
        if (item) command(item);
        return true;
      }
      return false;
    },
  }));

  if (items.length === 0) {
    return (
      <MenuShell>
        <div className="px-3 py-2 text-xs text-muted-foreground">No matches</div>
      </MenuShell>
    );
  }

  return (
    <MenuShell>
      {items.map((item, index) => {
        const Icon = item.icon;
        const active = index === selected;
        return (
          <button
            key={item.title}
            type="button"
            className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors ${active ? "bg-accent text-accent-foreground" : "text-foreground"}`}
            onMouseEnter={() => setSelected(index)}
            onClick={() => command(item)}
          >
            <Icon className="h-3.5 w-3.5 shrink-0" />
            <span className="font-medium">{item.title}</span>
          </button>
        );
      })}
    </MenuShell>
  );
});

function MenuShell({ children }: { children: ReactNode }) {
  return (
    <div className="z-50 max-h-72 w-56 overflow-y-auto rounded-md border border-border bg-popover py-1 text-popover-foreground shadow-md">
      {children}
    </div>
  );
}

export const SlashCommands = Extension.create({
  name: "slashCommands",

  addOptions() {
    return {
      suggestion: {
        char: "/",
        startOfLine: false,
        command: ({ editor, range, props }: { editor: Editor; range: Range; props: SlashItem }) => {
          props.apply({ editor, range });
        },
      },
    };
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        ...this.options.suggestion,
        items: ({ query }: { query: string }) => filterItems(query),
        render: () => {
          let component: ReactRenderer<SlashMenuRef, SuggestionProps<SlashItem>>;
          let popup: TippyInstance[] | undefined;

          return {
            onStart: (props) => {
              component = new ReactRenderer(SlashMenu, {
                props,
                editor: props.editor,
              });
              if (!props.clientRect) return;
              popup = tippy("body", {
                getReferenceClientRect: props.clientRect as TippyProps["getReferenceClientRect"],
                appendTo: () => document.body,
                content: component.element,
                showOnCreate: true,
                interactive: true,
                trigger: "manual",
                placement: "bottom-start",
              });
            },
            onUpdate(props) {
              component.updateProps(props);
              if (!props.clientRect) return;
              popup?.[0]?.setProps({
                getReferenceClientRect: props.clientRect as TippyProps["getReferenceClientRect"],
              });
            },
            onKeyDown(props) {
              if (props.event.key === "Escape") {
                popup?.[0]?.hide();
                return true;
              }
              return component.ref?.onKeyDown(props) ?? false;
            },
            onExit() {
              popup?.[0]?.destroy();
              component.destroy();
            },
          };
        },
      }),
    ];
  },
});
