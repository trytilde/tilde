import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { NativeSelect } from "@trytilde/connection-ui";
import type { Wiki, WikiAccess } from "@trytilde/contracts/tilde/management/v1/wikis_pb.js";
import { wikis as client } from "@/client";
import type { Applied } from "./agent-memory";
import { InlineSaving, type InlineSavingState } from "./inline-saving";
import { LinkedResources } from "./linked-agents";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

/** What an agent being created gets: a wiki of its own, and links to shared wikis. */
export type WikiDraft = {
  own: { name: string } | null;
  links: { id: string; access: WikiAccess }[];
};
export const NO_WIKIS: WikiDraft = { own: null, links: [] };

/** Creates the drafted wiki and links once the agent exists; `name` is the agent's. */
export async function applyWikiDraft(
  agentId: string,
  name: string,
  draft: WikiDraft,
  applied: Applied,
) {
  if (draft.own && !applied.own) {
    await client.createWiki({ name: draft.own.name.trim() || name, description: "", agentId });
    applied.own = true;
  }
  if (!applied.links) {
    for (const link of draft.links)
      await client.setAgentWiki({ agentId, wikiId: link.id, access: link.access });
    applied.links = true;
  }
}

/**
 * The agent's wikis: one of its own, and shared wikis it reads and may edit. Without `agentId`
 * it drafts the choices for an agent being created (`draft`, `onDraft`) and reports through
 * `onTaken` while the drafted wiki's name is another wiki's; with one, each change is saved at
 * once.
 */
export function AgentWikis({
  agentId,
  agentName,
  draft,
  onDraft,
  onTaken,
  disabled,
}: {
  agentId?: string;
  agentName: string;
  draft?: WikiDraft;
  onDraft?: (draft: WikiDraft) => void;
  onTaken?: (taken: boolean) => void;
  disabled?: boolean;
}) {
  let editable = true;
  const [wikis, setWikis] = useState<Wiki[]>([]);
  // The saved state of an existing agent; undefined until loaded.
  const [saved, setSaved] = useState<{ own?: Wiki; links: WikiDraft["links"] }>();
  // An existing agent's own wiki, before it is created.
  const [later, setLater] = useState({ name: "" });
  const [feedback, setFeedback] = useState<{
    state: InlineSavingState;
    attempt: number;
    error?: string;
  }>({ state: "idle", attempt: 0 });
  const load = useCallback(
    async (signal?: AbortSignal) => {
      const [list, linked] = await Promise.all([
        client.listWikis({}, { signal }),
        agentId ? client.listAgentWikis({ agentId }, { signal }) : undefined,
      ]);
      setWikis(list.wikis);
      if (linked)
        setSaved({
          own: list.wikis.find((w) => w.agentId === agentId),
          links: linked.wikis
            .filter((l) => !l.own)
            .map((l) => ({ id: l.wikiId, access: l.access })),
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
  // The own wiki being made: an existing agent's, or the draft's.
  const making = agentId ? (own ? null : later) : draft?.own;
  const ownName = making ? making.name.trim() || agentName.trim() : "";
  const taken = !!making && !!ownName && wikis.some((w) => w.name === ownName);
  useEffect(() => onTaken?.(!agentId && taken), [agentId, taken, onTaken]);
  function setOwn(next: { name: string } | null) {
    if (agentId) setLater(next ?? later);
    else onDraft?.({ ...draft!, own: next });
  }
  return (
    <section className="col-span-full grid gap-3 border-t pt-5" aria-label="Wikis">
      <div>
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium">Wikis</h3>
          <InlineSaving
            state={feedback.state}
            label="Wikis"
            resetKey={feedback.attempt}
            error={feedback.error}
          />
        </div>
        <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
          The agent reads its wikis' reference pages and, where it may write, keeps them up to date.{" "}
          <Link to="/wikis" className="text-foreground underline">
            All wikis
          </Link>
        </p>
      </div>
      <div className="grid gap-2 sm:grid-cols-[10rem_minmax(0,1fr)] sm:items-center">
        <Label htmlFor="agent-own-wiki">Its own wiki</Label>
        {own ? (
          <span className="text-sm">
            <Link to="/wikis/$wikiId/pages" params={{ wikiId: own.id }} className="underline">
              {own.name}
            </Link>{" "}
            <span className="text-muted-foreground">· {own.pageCount} pages</span>
          </span>
        ) : (
          <span className="flex flex-wrap items-center gap-2">
            {!agentId && (
              <NativeSelect
                id="agent-own-wiki"
                className="w-auto"
                value={making ? "new" : ""}
                disabled={locked}
                onChange={(event) =>
                  setOwn(event.target.value ? { name: making?.name ?? "" } : null)
                }
              >
                <option value="">None</option>
                <option value="new">New wiki</option>
              </NativeSelect>
            )}
            {making && (
              <Input
                id={agentId ? "agent-own-wiki" : undefined}
                aria-label="Name of its own wiki"
                className="w-56"
                placeholder={agentName || "Wiki name"}
                value={making.name}
                disabled={locked}
                onChange={(event) => setOwn({ name: event.target.value })}
              />
            )}
            {agentId && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={locked || taken || !ownName}
                onClick={() =>
                  void save(() => client.createWiki({ name: ownName, description: "", agentId }))
                }
              >
                Create its own wiki
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
            A wiki named “{ownName}” already exists; choose another name.
          </p>
        )}
      </div>
      <LinkedResources
        noun="wiki"
        plural="Other wikis"
        resources={wikis.filter((w) => !w.agentId).map((w) => ({ id: w.id, name: w.name }))}
        links={links}
        locked={locked}
        open={(wiki) => (
          <Link to="/wikis/$wikiId/pages" params={{ wikiId: wiki.id }} className="underline">
            {wiki.name}
          </Link>
        )}
        empty={
          <>
            No shared wikis yet.{" "}
            <Link to="/wikis" className="text-foreground underline">
              Create one
            </Link>
            .
          </>
        }
        unlinkDescription="The agent no longer reads or edits it. Its pages stay."
        onSet={(id, access) =>
          agentId
            ? void save(() => client.setAgentWiki({ agentId, wikiId: id, access }))
            : onDraft?.({ ...draft!, links: [...links.filter((l) => l.id !== id), { id, access }] })
        }
        onUnlink={(id) =>
          agentId
            ? void save(() => client.removeAgentWiki({ agentId, wikiId: id }))
            : onDraft?.({ ...draft!, links: links.filter((l) => l.id !== id) })
        }
      />
    </section>
  );
}
