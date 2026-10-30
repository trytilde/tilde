import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { NativeSelect } from "@trytilde/connection-ui";
import {
  MemoryBankScope,
  type MemoryAccess,
  type MemoryBank,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import { memory } from "@/client";
import { InlineSaving, type InlineSavingState } from "./inline-saving";
import { LinkedResources } from "./linked-agents";
import { SCOPES, scopeOf } from "./memory";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

/** What an agent being created gets: a bank of its own, and links to shared banks. */
export type MemoryDraft = {
  own: { name: string; scope: MemoryBankScope } | null;
  links: { id: string; access: MemoryAccess }[];
};
export const NO_MEMORY: MemoryDraft = { own: null, links: [] };
/** What of a draft exists already, so a retried save does neither twice. */
export type Applied = { own: boolean; links: boolean };

/** Creates the drafted bank and links once the agent exists; `name` is the agent's. */
export async function applyMemoryDraft(
  agentId: string,
  name: string,
  draft: MemoryDraft,
  applied: Applied,
) {
  if (draft.own && !applied.own) {
    await memory.createMemoryBank({
      name: draft.own.name.trim() || name,
      description: "",
      scope: draft.own.scope,
      agentId,
    });
    applied.own = true;
  }
  if (!applied.links) {
    for (const link of draft.links)
      await memory.setAgentMemoryBank({ agentId, bankId: link.id, access: link.access });
    applied.links = true;
  }
}

/** How a new own bank keeps memories, in the words the create form uses. */
const OWN_SCOPES: Record<Exclude<MemoryBankScope, MemoryBankScope.UNSPECIFIED>, string> = {
  [MemoryBankScope.SHARED]: "New bank, one pool",
  [MemoryBankScope.PER_PERSON]: "New bank, separate per person",
};

/**
 * The agent's memory: a bank of its own, and shared banks it recalls from and may write to.
 * Without `agentId` it drafts the choices for an agent being created (`draft`, `onDraft`) and
 * reports through `onTaken` while the drafted bank's name is another bank's; with one, each
 * change is saved at once.
 */
export function AgentMemory({
  agentId,
  agentName,
  draft,
  onDraft,
  onTaken,
  disabled,
}: {
  agentId?: string;
  agentName: string;
  draft?: MemoryDraft;
  onDraft?: (draft: MemoryDraft) => void;
  onTaken?: (taken: boolean) => void;
  disabled?: boolean;
}) {
  let editable = true;
  const [banks, setBanks] = useState<MemoryBank[]>([]);
  // The saved state of an existing agent; undefined until loaded.
  const [saved, setSaved] = useState<{ own?: MemoryBank; links: MemoryDraft["links"] }>();
  // An existing agent's own bank, before it is created.
  const [later, setLater] = useState({ name: "", scope: MemoryBankScope.SHARED });
  const [feedback, setFeedback] = useState<{
    state: InlineSavingState;
    attempt: number;
    error?: string;
  }>({ state: "idle", attempt: 0 });
  const load = useCallback(
    async (signal?: AbortSignal) => {
      const [list, linked] = await Promise.all([
        memory.listMemoryBanks({}, { signal }),
        agentId ? memory.listAgentMemoryBanks({ agentId }, { signal }) : undefined,
      ]);
      setBanks(list.banks);
      if (linked)
        setSaved({
          own: list.banks.find((b) => b.agentId === agentId),
          links: linked.banks
            .filter((l) => !l.own)
            .map((l) => ({ id: l.bankId, access: l.access })),
        });
    },
    [agentId],
  );
  useEffect(() => {
    const abort = new AbortController();
    load(abort.signal).catch(
      (e) =>
        !abort.signal.aborted &&
        setFeedback((current) => ({ ...current, state: "error", error: message(e) })),
    );
    return () => abort.abort();
  }, [load]);
  async function save(change: () => Promise<unknown>) {
    setFeedback((current) => ({ state: "saving", attempt: current.attempt + 1 }));
    try {
      await change();
      await load();
      setFeedback((current) => ({ ...current, state: "success" }));
    } catch (e) {
      setFeedback((current) => ({ ...current, state: "error", error: message(e) }));
    }
  }
  const links = agentId ? (saved?.links ?? []) : (draft?.links ?? []);
  const own = agentId ? saved?.own : undefined;
  const locked = disabled || !editable || (!!agentId && !saved) || feedback.state === "saving";
  // The own bank being made: an existing agent's, or the draft's.
  const making = agentId ? (own ? null : later) : draft?.own;
  const ownName = making ? making.name.trim() || agentName.trim() : "";
  const taken = !!making && !!ownName && banks.some((b) => b.name === ownName);
  useEffect(() => onTaken?.(!agentId && taken), [agentId, taken, onTaken]);
  function setOwn(next: { name: string; scope: MemoryBankScope } | null) {
    if (agentId) setLater(next ?? later);
    else onDraft?.({ ...draft!, own: next });
  }
  return (
    <section className="col-span-full grid gap-3 border-t pt-5" aria-label="Memory">
      <div>
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium">Memory</h3>
          <InlineSaving
            state={feedback.state}
            label="Memory"
            resetKey={feedback.attempt}
            error={feedback.error}
          />
        </div>
        <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
          The agent recalls from its memory banks and, where it may write, remembers what it learns.{" "}
          <Link to="/memory" className="text-foreground underline">
            All memory banks
          </Link>
        </p>
      </div>
      <div className="grid gap-2 sm:grid-cols-[10rem_minmax(0,1fr)] sm:items-center">
        <Label htmlFor="agent-own-bank">Its own bank</Label>
        {own ? (
          <span className="text-sm">
            <Link to="/memory/$bankId/memories" params={{ bankId: own.id }} className="underline">
              {own.name}
            </Link>{" "}
            <span className="text-muted-foreground">
              · {SCOPES[scopeOf(own.scope)].label} · {own.memoryCount} memories
            </span>
          </span>
        ) : (
          <span className="flex flex-wrap items-center gap-2">
            <NativeSelect
              id="agent-own-bank"
              className="w-auto"
              value={making ? String(making.scope) : ""}
              disabled={locked}
              onChange={(event) =>
                setOwn(
                  event.target.value
                    ? {
                        name: making?.name ?? "",
                        scope: Number(event.target.value) as MemoryBankScope,
                      }
                    : null,
                )
              }
            >
              {!agentId && <option value="">None</option>}
              {Object.entries(OWN_SCOPES).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </NativeSelect>
            {making && (
              <Input
                aria-label="Name of its own bank"
                className="w-56"
                placeholder={agentName || "Bank name"}
                value={making.name}
                disabled={locked}
                onChange={(event) => setOwn({ ...making, name: event.target.value })}
              />
            )}
            {agentId && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={locked || taken || !ownName}
                onClick={() =>
                  void save(() =>
                    memory.createMemoryBank({
                      name: ownName,
                      description: "",
                      scope: later.scope,
                      agentId,
                    }),
                  )
                }
              >
                Create its own bank
              </Button>
            )}
          </span>
        )}
        {!own && (
          <p className="text-xs text-muted-foreground sm:col-start-2">
            Created for this agent and deleted with it.
          </p>
        )}
        {taken && (
          <p role="alert" className="text-xs text-destructive sm:col-start-2">
            A memory bank named “{ownName}” already exists; choose another name.
          </p>
        )}
      </div>
      <LinkedResources
        noun="bank"
        plural="Other banks"
        resources={banks
          .filter((b) => !b.agentId)
          .map((b) => ({
            id: b.id,
            name: b.name,
            detail: SCOPES[scopeOf(b.scope)].label.toLowerCase(),
          }))}
        links={links}
        locked={locked}
        open={(bank) => (
          <Link to="/memory/$bankId/memories" params={{ bankId: bank.id }} className="underline">
            {bank.name}
          </Link>
        )}
        empty={
          <>
            No shared banks yet.{" "}
            <Link to="/memory" className="text-foreground underline">
              Create one
            </Link>
            .
          </>
        }
        unlinkDescription="The agent no longer recalls from or writes to it. Its memories stay."
        onSet={(id, access) =>
          agentId
            ? void save(() => memory.setAgentMemoryBank({ agentId, bankId: id, access }))
            : onDraft?.({ ...draft!, links: [...links.filter((l) => l.id !== id), { id, access }] })
        }
        onUnlink={(id) =>
          agentId
            ? void save(() => memory.removeAgentMemoryBank({ agentId, bankId: id }))
            : onDraft?.({ ...draft!, links: links.filter((l) => l.id !== id) })
        }
      />
    </section>
  );
}
