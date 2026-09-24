import { Autocomplete } from "@base-ui/react/autocomplete";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useForm } from "@trytilde/connection-ui";
import { SearchIcon, XIcon } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { tokens, type SearchSuggestion } from "./query";

/** A keyboard-accessible query editor over the existing URL/server filter contract. */
export function FilterSearchInput({
  canonical,
  disabled,
  onApply,
  validate,
  suggest,
  label,
}: {
  canonical: string;
  disabled: boolean;
  onApply: (query: string) => void;
  validate: (query: string) => unknown;
  suggest: (query: string, caret: number) => SearchSuggestion[];
  label: string;
}) {
  const form = useForm<{ query: string }>({ defaultValues: { query: "" } });
  const query = form.watch("query");
  const [chips, setChips] = useState(() => tokens(canonical).items.map((token) => token.text));
  const [hovering, setHovering] = useState(false);
  const [focused, setFocused] = useState(false);
  const [open, setOpen] = useState(false);
  const [caret, setCaret] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const errorId = useId();
  const items = useMemo(
    () => suggest(query, Math.min(caret, query.length)),
    [query, caret, suggest],
  );
  const error = form.formState.errors.query?.message;
  const { ref, ...registration } = form.register("query");
  useEffect(() => {
    form.reset({ query: "" });
    setChips(tokens(canonical).items.map((token) => token.text));
    setCaret(0);
  }, [canonical]);
  const submit = form.handleSubmit(({ query }) => {
    try {
      onApply([...chips, query].join(" "));
      setOpen(false);
    } catch (error) {
      form.setError("query", {
        message: error instanceof Error ? error.message : "Invalid search",
      });
      setOpen(true);
    }
  });
  return (
    <form
      onSubmit={submit}
      className="relative h-[26px] min-w-40 flex-1"
      role="search"
      aria-label={label}
      onMouseEnter={() => setHovering(true)}
      onMouseLeave={() => setHovering(false)}
      onFocusCapture={() => setFocused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
    >
      <button type="submit" tabIndex={-1} className="sr-only" disabled={disabled}>
        Apply search
      </button>
      <Autocomplete.Root
        items={items}
        filter={null}
        value={query}
        open={open && !disabled}
        openOnInputClick
        onOpenChange={(value) => setOpen(value)}
        onValueChange={(value, details) => {
          // Completed terms become chips; unfinished/invalid syntax stays editable.
          const scanned = tokens(value);
          const complete = scanned.items.filter((token) => token.end < value.length);
          let consumed = 0;
          const added: string[] = [];
          for (const token of complete) {
            try {
              validate(token.text);
            } catch {
              break;
            }
            added.push(token.text);
            consumed = token.end + 1;
          }
          if (added.length) setChips((old) => [...old, ...added]);
          const draft = value.slice(consumed);
          form.setValue("query", draft);
          form.clearErrors("query");
          setCaret(
            details.reason === "item-press"
              ? draft.length
              : Math.max(0, (input.current?.selectionStart ?? value.length) - consumed),
          );
          if (details.reason === "item-press" && value.endsWith(":"))
            queueMicrotask(() => setOpen(true));
        }}
        itemToStringValue={(item) => item.value}
      >
        <Autocomplete.InputGroup
          data-slot="trace-search-chips"
          data-expanded={!disabled && (hovering || focused || open)}
          className={`absolute top-0 left-0 z-40 flex min-h-[26px] w-full items-center gap-1 rounded-md border border-input bg-background py-0.5 pr-7 pl-6 ${!disabled && (hovering || focused || open) ? "max-h-64 flex-wrap overflow-y-auto shadow-md" : "max-h-[26px] flex-nowrap overflow-hidden"}`}
          onClick={(event) => {
            if (event.target === event.currentTarget) input.current?.focus();
          }}
        >
          <SearchIcon
            aria-hidden
            className="pointer-events-none absolute top-1.5 left-1.5 z-10 size-3 text-muted-foreground"
          />
          {chips.map((chip, index) => (
            <span
              key={`${index}:${chip}`}
              className="inline-flex max-w-full shrink-0 items-center rounded border bg-muted/60 text-[11px]"
              data-slot="search-chip"
            >
              <button
                type="button"
                disabled={disabled}
                aria-label={`Edit ${chip}`}
                className="min-w-0 px-1 leading-[18px] whitespace-normal break-all text-left font-normal"
                onClick={() => {
                  setChips((old) => old.filter((_, i) => i !== index));
                  const draft = [query, chip].filter(Boolean).join(" ");
                  form.setValue("query", draft);
                  setCaret(draft.length);
                  form.clearErrors("query");
                  input.current?.focus();
                }}
              >
                {!chip.startsWith('"') && chip.includes(":") ? (
                  <>
                    <strong className="font-bold">{chip.slice(0, chip.indexOf(":") + 1)}</strong>
                    {chip.slice(chip.indexOf(":") + 1)}
                  </>
                ) : (
                  chip
                )}
              </button>
              <button
                type="button"
                disabled={disabled}
                aria-label={`Remove ${chip}`}
                className="flex size-4 shrink-0 items-center justify-center rounded-sm hover:bg-muted"
                onClick={() => {
                  const remaining = chips.filter((_, i) => i !== index);
                  setChips(remaining);
                  try {
                    onApply(remaining.join(" "));
                  } catch {
                    /* Keep unfinished chips editable. */
                  }
                }}
              >
                <XIcon className="size-2.5" />
              </button>
            </span>
          ))}
          <Autocomplete.Input
            {...registration}
            ref={(node) => {
              ref(node);
              input.current = node;
            }}
            render={<Input />}
            disabled={disabled}
            aria-label={label}
            aria-invalid={!!error}
            aria-describedby={error ? errorId : undefined}
            placeholder={chips.length ? "Add filter…" : "Search or field:value…"}
            maxLength={8192}
            className="h-5 min-w-20 flex-1 rounded-none border-0 bg-transparent px-0 py-0 text-[11px] shadow-none ring-0! focus-visible:ring-0! md:text-[11px]"
            onSelect={(event) => setCaret(event.currentTarget.selectionStart ?? query.length)}
          />
          {(query || chips.length > 0) && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute top-0 right-0.5 size-6"
              aria-label="Clear search"
              disabled={disabled}
              onClick={() => {
                form.reset({ query: "" });
                setChips([]);
                setCaret(0);
                setOpen(false);
                onApply("");
                input.current?.focus();
              }}
            >
              <XIcon className="size-3" />
            </Button>
          )}
        </Autocomplete.InputGroup>
        <Autocomplete.Portal>
          <Autocomplete.Positioner sideOffset={6} align="start" className="z-50">
            <Autocomplete.Popup className="trace-controls w-[var(--anchor-width)] min-w-80 max-w-[calc(100vw-2rem)] overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-md">
              {error && (
                <p
                  role="alert"
                  id={errorId}
                  className="border-b px-3 py-2 text-xs text-destructive"
                >
                  {error}
                </p>
              )}
              <div className="flex justify-between border-b px-3 py-2 text-[10px] text-muted-foreground">
                <span>Search suggestions</span>
                <code>field:value</code>
              </div>
              <Autocomplete.List className="max-h-64 overflow-y-auto p-1">
                {(item) => (
                  <Autocomplete.Item
                    key={item.value}
                    value={item}
                    className="flex cursor-default items-center justify-between gap-3 rounded px-2 py-1 text-[11px] outline-none data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                  >
                    <code
                      className={`truncate ${item.label.endsWith(":") ? "font-bold" : "font-normal"}`}
                    >
                      {item.label}
                    </code>
                    <span className="shrink-0 text-[10px] text-muted-foreground">
                      {item.description}
                    </span>
                  </Autocomplete.Item>
                )}
              </Autocomplete.List>
              {!items.length && (
                <p className="px-3 py-2 text-xs text-muted-foreground">
                  {query.includes(":")
                    ? "Enter a value, then press Enter."
                    : "Enter text, then press Enter."}
                </p>
              )}
              <p className="border-t bg-muted/20 px-3 py-2 text-[10px] text-muted-foreground">
                ↑↓ suggestions · Enter applies · Quote values with spaces
              </p>
            </Autocomplete.Popup>
          </Autocomplete.Positioner>
        </Autocomplete.Portal>
      </Autocomplete.Root>
    </form>
  );
}
