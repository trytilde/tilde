import { useEffect, useMemo, useRef, useState } from "react";
import { ZapIcon } from "lucide-react";
import { cn } from "cn";

/** A path a `{{ key }}` template may name, with an example value. */
export type TemplateVariable = { key: string; description: string; example: string };

type Part =
  | { id: string; type: "text"; value: string }
  | { id: string; type: "variable"; key: string };

const TOKEN = /{{\s*([A-Za-z0-9_.-]+)\s*}}/g;
let nextPart = 0;
const partId = () => `part-${nextPart++}`;

function parse(value: string): Part[] {
  const parts: Part[] = [];
  let cursor = 0;
  for (const match of value.matchAll(TOKEN)) {
    const start = match.index ?? 0;
    if (start > cursor)
      parts.push({ id: partId(), type: "text", value: value.slice(cursor, start) });
    parts.push({ id: partId(), type: "variable", key: match[1] ?? "" });
    cursor = start + match[0].length;
  }
  if (cursor < value.length) parts.push({ id: partId(), type: "text", value: value.slice(cursor) });
  return parts;
}
const serialize = (parts: Part[], draft = "") =>
  parts.map((part) => (part.type === "variable" ? `{{ ${part.key} }}` : part.value)).join("") +
  draft;

const unknown = (key: string): TemplateVariable => ({ key, description: "", example: "" });

/** A variable as one token: its key and an example of what it renders. */
export function VariableBadge({
  variable,
  onRemove,
}: {
  variable: TemplateVariable;
  onRemove?: () => void;
}) {
  const content = (
    <>
      <span className="flex h-full items-center px-1.5 text-sky-700 dark:text-sky-300">
        <ZapIcon className="size-3" />
      </span>
      <span className="h-full w-px bg-sky-200 dark:bg-sky-800" />
      <span className="min-w-0 truncate px-1.5 font-mono text-[11px] text-sky-950 dark:text-sky-100">
        {variable.key}
      </span>
      {variable.example && (
        <>
          <span className="h-full w-px bg-sky-200 dark:bg-sky-800" />
          <span className="max-w-[10rem] truncate px-1.5 text-[11px] text-sky-700 dark:text-sky-300">
            {variable.example}
          </span>
        </>
      )}
    </>
  );
  const className =
    "inline-flex h-6 max-w-full shrink-0 items-center overflow-hidden rounded-md border border-sky-200 bg-sky-50 text-left align-middle text-xs shadow-xs dark:border-sky-800 dark:bg-sky-950";
  return onRemove ? (
    <button
      type="button"
      className={cn(className, "cursor-pointer")}
      onClick={onRemove}
      aria-label={`Remove ${variable.key}`}
      title={variable.description || undefined}
    >
      {content}
    </button>
  ) : (
    <span className={className} title={variable.description || undefined}>
      {content}
    </span>
  );
}

/**
 * A text field whose `{{ key }}` variables render as badges. Typing `{{` searches the variables;
 * picking one, or typing a complete `{{ key }}`, inserts it as a token. Backspace removes a whole
 * token, or takes the text before it back into editing. The value is the serialized template.
 */
export function TemplateInput({
  id,
  value,
  onChange,
  onFocus,
  variables,
  multiline,
  placeholder,
  invalid,
}: {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  onFocus?: () => void;
  variables: TemplateVariable[];
  multiline?: boolean;
  placeholder?: string;
  invalid?: boolean;
}) {
  const [parts, setParts] = useState(() => parse(value));
  const [draft, setDraft] = useState("");
  const field = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  // A value changed from outside (an inserted variable, a new default) is parsed again.
  useEffect(() => {
    if (value !== serialize(parts, draft)) {
      setParts(parse(value));
      setDraft("");
    }
    // Only outside changes matter here; local edits keep the value and state in step.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);
  const byKey = useMemo(() => new Map(variables.map((v) => [v.key, v])), [variables]);
  const query = useMemo(() => {
    const open = draft.lastIndexOf("{{");
    return open === -1 || draft.lastIndexOf("}}") > open ? null : draft.slice(open + 2).trim();
  }, [draft]);
  const suggestions =
    query === null
      ? []
      : variables.filter((v) => v.key.toLowerCase().includes(query.toLowerCase()));

  function commit(nextParts: Part[], nextDraft: string) {
    setParts(nextParts);
    setDraft(nextDraft);
    onChange(serialize(nextParts, nextDraft));
  }
  function type(text: string) {
    const completed = /{{\s*([A-Za-z0-9_.-]+)\s*}}/.exec(text);
    if (!completed) return commit(parts, text);
    const before = text.slice(0, completed.index);
    const next: Part[] = [...parts];
    if (before) next.push({ id: partId(), type: "text", value: before });
    next.push({ id: partId(), type: "variable", key: completed[1] ?? "" });
    commit(next, text.slice(completed.index + completed[0].length));
  }
  function pick(variable: TemplateVariable) {
    const open = draft.lastIndexOf("{{");
    const before = open >= 0 ? draft.slice(0, open) : draft;
    const next: Part[] = [...parts];
    if (before) next.push({ id: partId(), type: "text", value: before });
    next.push({ id: partId(), type: "variable", key: variable.key });
    commit(next, "");
    field.current?.focus();
  }
  const Field = multiline ? "textarea" : "input";
  return (
    <div className="relative">
      <div
        className={cn(
          "flex w-full cursor-text flex-wrap items-center gap-1 rounded-lg border border-input bg-transparent px-2.5 py-1 text-sm transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50",
          multiline ? "min-h-24 items-start" : "min-h-8",
          invalid && "border-destructive ring-3 ring-destructive/20",
        )}
        onClick={() => field.current?.focus()}
      >
        {parts.map((part) =>
          part.type === "variable" ? (
            <VariableBadge
              key={part.id}
              variable={byKey.get(part.key) ?? unknown(part.key)}
              onRemove={() =>
                commit(
                  parts.filter((p) => p.id !== part.id),
                  draft,
                )
              }
            />
          ) : (
            <span key={part.id} className="whitespace-pre-wrap">
              {part.value}
            </span>
          ),
        )}
        <Field
          ref={field}
          id={id}
          rows={multiline ? 2 : undefined}
          className={cn(
            "min-w-[8rem] flex-1 resize-none bg-transparent outline-none placeholder:text-muted-foreground",
            multiline && "field-sizing-content",
          )}
          value={draft}
          placeholder={parts.length === 0 && !draft ? placeholder : ""}
          onFocus={onFocus}
          onChange={(event) => type(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Backspace" || draft || !parts.length) return;
            event.preventDefault();
            const last = parts[parts.length - 1];
            const rest = parts.slice(0, -1);
            // A variable goes whole; text comes back into the field minus its last character.
            commit(rest, last.type === "text" ? last.value.slice(0, -1) : "");
          }}
        />
      </div>
      {suggestions.length > 0 && (
        <div className="absolute z-50 mt-1 max-h-56 w-full overflow-auto rounded-lg border bg-popover p-1 shadow-md">
          {suggestions.map((variable) => (
            <button
              type="button"
              key={variable.key}
              className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-muted"
              onMouseDown={(event) => {
                event.preventDefault();
                pick(variable);
              }}
            >
              <VariableBadge variable={variable} />
              <span className="truncate text-muted-foreground">{variable.description}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
