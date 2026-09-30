import { useEffect, useState } from "react";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { CatalogIcon } from "./tool-catalog";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export type Choice<T> = { id: string; name: string; detail: string; iconUrl?: string; value: T };

/**
 * A card grid, in the tool catalog's style, of things to choose one of. `load` runs on the server
 * side for every (debounced) search; the dialog never filters what it was given. Continue hands
 * the chosen card to `onChoose`.
 */
export function ChooseDialog<T>({
  open,
  title,
  description,
  searchLabel,
  listLabel,
  empty,
  busy,
  load,
  reloadKey = "",
  onClose,
  onChoose,
}: {
  open: boolean;
  title: string;
  description: string;
  searchLabel: string;
  listLabel: string;
  empty: (search: string) => string;
  busy: boolean;
  load: (search: string | undefined, signal: AbortSignal) => Promise<Choice<T>[]>;
  /** Changes when something `load` depends on changes. */
  reloadKey?: string;
  onClose: () => void;
  onChoose: (choice: Choice<T>) => Promise<void>;
}) {
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const [chosen, setChosen] = useState<Choice<T>>();
  const [matches, setMatches] = useState<Choice<T>[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!open) return;
    const abort = new AbortController();
    setError("");
    void load(search || undefined, abort.signal)
      .then((found) => {
        if (!abort.signal.aborted) setMatches(found);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      });
    return () => abort.abort();
    // `load` is recreated by every parent render; `reloadKey` names what it depends on.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, search, reloadKey]);
  function close() {
    setQuery("");
    setChosen(undefined);
    setMatches(null);
    onClose();
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) close();
      }}
    >
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <Input
          aria-label={searchLabel}
          placeholder={searchLabel}
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
        />
        <ul
          role="listbox"
          aria-label={listLabel}
          className="m-0 grid max-h-[55dvh] list-none grid-cols-2 gap-x-2 gap-y-0.5 overflow-y-auto p-0 max-sm:grid-cols-1"
        >
          {(matches ?? []).map((choice) => {
            const selected = chosen?.id === choice.id;
            return (
              <li
                key={choice.id}
                role="option"
                aria-selected={selected}
                tabIndex={0}
                onClick={() => setChosen(choice)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setChosen(choice);
                  }
                }}
                className={`flex min-w-0 cursor-pointer items-center gap-3 rounded-2xl px-3 py-[9.5px] outline-none hover:bg-foreground/5 focus-visible:bg-foreground/5 ${
                  selected ? "bg-primary/10 ring-1 ring-primary hover:bg-primary/10" : ""
                }`}
              >
                <CatalogIcon id={choice.id} name={choice.name} iconUrl={choice.iconUrl} />
                <div className="flex min-w-0 flex-1 flex-col gap-px">
                  <h3 className="m-0 truncate text-[13px] leading-[18px] font-medium text-foreground">
                    {choice.name}
                  </h3>
                  <p className="m-0 truncate text-[13px] leading-[18px] text-foreground/60">
                    {choice.detail}
                  </p>
                </div>
              </li>
            );
          })}
          {matches?.length === 0 && (
            <li className="col-span-2 px-3 py-6 text-center text-sm text-muted-foreground">
              {empty(search)}
            </li>
          )}
          {matches === null && !error && (
            <li className="col-span-2 px-3 py-6 text-center text-sm text-muted-foreground">
              Loading…
            </li>
          )}
          {error && (
            <li role="alert" className="col-span-2 px-3 py-6 text-center text-sm text-destructive">
              {error}
            </li>
          )}
        </ul>
        <DialogFooter>
          <Button type="button" variant="outline" disabled={busy} onClick={close}>
            Cancel
          </Button>
          <Button
            disabled={!chosen || busy}
            onClick={() => {
              if (chosen) void onChoose(chosen).then(close);
            }}
          >
            Continue
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
