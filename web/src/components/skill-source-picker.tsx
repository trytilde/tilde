import { useEffect, useState } from "react";
import { CheckIcon, PlusIcon, XIcon } from "lucide-react";
import type { SkillSource } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { skills } from "@/client";
import { SourceKindIcon, message } from "./skill-common";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./ui/dialog";

/** Selected skill sources for a capability that targets sources rather than agents. */
export function SkillSourcePicker({
  label,
  value,
  onConfirm,
  disabled,
}: {
  label: string;
  value: string[];
  onConfirm: (ids: string[]) => Promise<void>;
  disabled: boolean;
}) {
  const [sources, setSources] = useState<SkillSource[]>([]);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    void skills
      .listSkillSources({}, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) setSources(response.sources);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load skill sources."));
      });
    return () => abort.abort();
  }, []);
  const name = (id: string) => sources.find((s) => s.id === id)?.name ?? "Unavailable source";
  async function confirm() {
    setConfirming(true);
    setError("");
    try {
      if (draft.join(",") !== value.join(",")) await onConfirm(draft);
      setOpen(false);
    } catch (e) {
      setError(message(e, "Unable to save selected sources."));
    } finally {
      setConfirming(false);
    }
  }
  return (
    <div className="col-span-full" aria-label={`${label} targets`}>
      <div className="flex flex-wrap items-center gap-1.5 py-1">
        {value.map((id) => (
          <span
            key={id}
            className="inline-flex items-center gap-1 rounded-md border bg-muted/50 py-0.5 pr-1 pl-2 text-xs"
          >
            {name(id)}
            <button
              type="button"
              disabled={disabled}
              aria-label={`Remove ${name(id)} from ${label}`}
              className="rounded-sm text-muted-foreground hover:text-foreground disabled:opacity-50"
              onClick={() => void onConfirm(value.filter((v) => v !== id)).catch(() => {})}
            >
              <XIcon className="size-3.5" />
            </button>
          </span>
        ))}
        <Button
          type="button"
          variant="outline"
          size="icon-sm"
          className="rounded-full"
          disabled={disabled}
          aria-label={`Select skill sources for ${label}`}
          onClick={() => {
            setDraft(value);
            setError("");
            setOpen(true);
          }}
        >
          <PlusIcon />
        </Button>
      </div>
      <Dialog open={open} onOpenChange={(next) => !confirming && setOpen(next)}>
        <DialogContent className="sm:max-w-md" showCloseButton={!confirming}>
          <DialogTitle>{label}</DialogTitle>
          <DialogDescription>
            Select the skill sources this capability applies to.
          </DialogDescription>
          <div className="grid max-h-80 gap-1 overflow-y-auto">
            {sources.map((source) => (
              <button
                key={source.id}
                type="button"
                disabled={confirming}
                aria-pressed={draft.includes(source.id)}
                className="flex min-w-0 items-center gap-3 rounded-lg p-2 text-left text-sm hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
                onClick={() =>
                  setDraft(
                    draft.includes(source.id)
                      ? draft.filter((id) => id !== source.id)
                      : [...draft, source.id],
                  )
                }
              >
                <SourceKindIcon kind={source.kind} />
                <span className="min-w-0 flex-1 truncate">{source.name}</span>
                {draft.includes(source.id) && <CheckIcon className="size-4" />}
              </button>
            ))}
            {!sources.length && (
              <p className="text-sm text-muted-foreground">No skill sources you can view.</p>
            )}
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm text-muted-foreground">{draft.length} selected</span>
            <Button type="button" disabled={confirming} onClick={() => void confirm()}>
              Confirm
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
