// Adapted from Dispatch's capability avatar stack and BotSelectionDialog.
// Keep registry requests here; the ported avatar package has no API dependency.
import { useEffect, useRef, useState } from "react";
import { PlusIcon, XIcon, CheckIcon } from "lucide-react";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { AgentAvatar } from "./agent-avatar";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Dialog, DialogContent, DialogTitle, DialogDescription } from "./ui/dialog";

export function AgentTargetPicker({
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
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const [confirming, setConfirming] = useState(false);
  const [saveError, setSaveError] = useState("");
  async function confirm() {
    setConfirming(true);
    setSaveError("");
    try {
      if (draft.join(",") !== value.join(",")) await onConfirm(draft);
      setOpen(false);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : "Unable to save selected agents.");
    } finally {
      setConfirming(false);
    }
  }
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<Agent[]>([]);
  const [known, setKnown] = useState<Record<string, Agent>>({});
  const [next, setNext] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  // Resolve saved targets even if they are outside the current search/page.
  useEffect(() => {
    let active = true;
    const missing = value.filter((id) => !known[id]);
    if (missing.length)
      void Promise.all(
        missing.map(async (id) => {
          try {
            return (await agents.getAgent({ id })).agent;
          } catch {
            return undefined;
          } // Keep unavailable IDs removable rather than silently dropping grants.
        }),
      ).then((resolved) => {
        if (active && resolved.some(Boolean))
          setKnown((current) => ({
            ...current,
            ...Object.fromEntries(
              resolved.filter((agent): agent is Agent => !!agent).map((agent) => [agent.id, agent]),
            ),
          }));
      });
    return () => {
      active = false;
    };
  }, [value, known]);

  async function load(pageToken: string, current: number) {
    setLoading(true);
    setError("");
    try {
      const result = await agents.listAgents({ search, pageSize: 20, pageToken });
      if (generation.current !== current) return;
      setItems((previous) => (pageToken ? [...previous, ...result.agents] : result.agents));
      setKnown((previous) => ({
        ...previous,
        ...Object.fromEntries(result.agents.map((agent) => [agent.id, agent])),
      }));
      setNext(result.nextPageToken);
    } catch (error) {
      if (generation.current === current)
        setError(error instanceof Error ? error.message : "Unable to load agents.");
    } finally {
      if (generation.current === current) setLoading(false);
    }
  }
  useEffect(() => {
    const current = ++generation.current;
    if (!open) return;
    setItems([]);
    setNext("");
    setLoading(true);
    const timeout = setTimeout(() => void load("", current), 200);
    return () => {
      clearTimeout(timeout);
      ++generation.current;
    };
    // A fresh generation owns each open/search request; pagination explicitly calls load.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, search]);

  return (
    <div className="col-span-full" aria-label={`${label} targets`}>
      <div className="flex -space-x-2 items-center py-1">
        {value.map((id) => {
          const agent = known[id] ?? { id, name: "Unavailable agent" };
          return (
            <button
              key={id}
              type="button"
              disabled={disabled}
              title={agent.name}
              aria-label={`Remove ${agent.name} from ${label}`}
              onClick={() =>
                void onConfirm(value.filter((selected) => selected !== id)).catch(() => {})
              }
              className="group relative rounded-full border-2 border-background bg-background hover:z-10 focus-visible:z-10 focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-50"
            >
              <AgentAvatar agent={agent} className="size-8" />
              <span className="absolute inset-0 grid place-items-center rounded-full bg-destructive/80 text-white opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100">
                <XIcon className="size-4" />
              </span>
            </button>
          );
        })}
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="relative rounded-full border-2 border-background"
          disabled={disabled}
          aria-label={`Select agents for ${label}`}
          onClick={() => {
            setDraft(value);
            setSaveError("");
            setOpen(true);
          }}
        >
          <PlusIcon />
        </Button>
      </div>
      <Dialog
        open={open}
        onOpenChange={(next) => {
          if (!confirming) setOpen(next);
        }}
      >
        <DialogContent className="sm:max-w-xl" showCloseButton={!confirming}>
          <DialogTitle>{label}</DialogTitle>
          <DialogDescription>Select the agents this capability applies to.</DialogDescription>
          <Input
            autoFocus
            aria-label="Search agents"
            placeholder="Search agents by name…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
          <div className="grid max-h-80 gap-1 overflow-y-auto sm:grid-cols-2">
            {items.map((agent) => (
              <button
                key={agent.id}
                type="button"
                disabled={confirming}
                aria-pressed={draft.includes(agent.id)}
                className="flex min-w-0 items-center gap-3 rounded-lg p-2 text-left hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
                onClick={() =>
                  setDraft(
                    draft.includes(agent.id)
                      ? draft.filter((id) => id !== agent.id)
                      : [...draft, agent.id],
                  )
                }
              >
                <AgentAvatar agent={agent} />
                <span className="min-w-0 flex-1 truncate">{agent.name}</span>
                {draft.includes(agent.id) && <CheckIcon className="size-4" />}
              </button>
            ))}
          </div>
          {loading && (
            <p role="status" className="text-sm text-muted-foreground">
              Loading agents…
            </p>
          )}
          {!loading && !error && !items.length && (
            <p className="text-sm text-muted-foreground">No agents found.</p>
          )}
          {error && (
            <p role="alert">
              {error}{" "}
              <Button
                type="button"
                variant="link"
                onClick={() => void load(next, generation.current)}
              >
                Retry
              </Button>
            </p>
          )}
          {saveError && (
            <p role="alert" className="text-sm text-destructive">
              {saveError}
            </p>
          )}
          <div className="flex justify-between gap-2">
            <span className="text-sm text-muted-foreground">{draft.length} selected</span>
            <div className="flex gap-2">
              {next && (
                <Button
                  type="button"
                  variant="outline"
                  disabled={loading}
                  onClick={() => void load(next, generation.current)}
                >
                  Load more
                </Button>
              )}
              <Button type="button" disabled={confirming} onClick={() => void confirm()}>
                Confirm
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
