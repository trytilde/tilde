import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { Link } from "@tanstack/react-router";
import { timestampDate } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import { NativeSelect } from "@trytilde/connection-ui";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import type { Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import {
  MemoryAccess,
  MemorySynthesisBatchStatus,
  type GetMemorySynthesisResponse,
  type MemoryBank,
  type MemoryEvidence,
  type MemorySynthesisBatch,
} from "@trytilde/contracts/tilde/management/v1/memory_pb.js";
import type { Wiki } from "@trytilde/contracts/tilde/management/v1/wikis_pb.js";
import { memory, wikis as wikiClient } from "@/client";
import { inferenceConnections } from "./memory";
import { RemoveButton } from "./remove-button";
import { message } from "./tool-connections";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";

type Values = { connectionId: string; model: string; instructions: string };

/**
 * A bank's synthesis: the chat model that turns its sources into memories, the sources (linked
 * agents' conversations and wikis' pages), and how the latest batches went.
 */
export function BankSynthesis({
  bank,
  registry,
}: {
  bank: MemoryBank;
  registry: Map<string, Agent>;
}) {
  let editable = true;
  const [state, setState] = useState<GetMemorySynthesisResponse>();
  const [batches, setBatches] = useState<MemorySynthesisBatch[]>([]);
  const [available, setAvailable] = useState<Connection[]>([]);
  const [models, setModels] = useState<string[]>([]);
  const [allWikis, setAllWikis] = useState<Wiki[]>([]);
  const [error, setError] = useState("");
  const [linking, setLinking] = useState<Wiki>();
  const form = useForm<Values>({
    defaultValues: { connectionId: "", model: "", instructions: "" },
  });
  const connectionId = form.watch("connectionId");
  const load = useCallback(
    async (signal?: AbortSignal) => {
      const [got, listed, found, wikiList] = await Promise.all([
        memory.getMemorySynthesis({ bankId: bank.id }, { signal }),
        memory.listMemorySynthesisBatches({ bankId: bank.id }, { signal }),
        inferenceConnections(signal),
        wikiClient.listWikis({}, { signal }),
      ]);
      setState(got);
      setBatches(listed.batches);
      setAvailable(found);
      setAllWikis(wikiList.wikis);
      const s = got.synthesis;
      form.reset({
        connectionId: s?.connectionId ?? found[0]?.id ?? "",
        model: s?.model ?? "",
        instructions: s?.instructions ?? "",
      });
    },
    // Linked agents can change on the same page, and their retaining with them.
    [bank.id, bank.links, form],
  );
  useEffect(() => {
    const abort = new AbortController();
    load(abort.signal).catch((e) => !abort.signal.aborted && setError(message(e)));
    return () => abort.abort();
  }, [load]);
  useEffect(() => {
    if (!connectionId) return setModels([]);
    const abort = new AbortController();
    memory
      .listChatModels({ connectionId }, { signal: abort.signal })
      .then(({ models }) => setModels(models))
      .catch(() => !abort.signal.aborted && setModels([]));
    return () => abort.abort();
  }, [connectionId]);
  const act = (write: () => Promise<unknown>) =>
    write()
      .then(() => load())
      .then(() => setError(""))
      .catch((e) => setError(message(e)));

  if (!state) return null;
  const writers = bank.links.filter((l) => l.access === MemoryAccess.READ_WRITE);
  const retaining = new Set(state.retainingAgentIds);
  const linked = new Set(state.wikis.map((w) => w.wikiId));
  const unlinked = allWikis.filter((w) => !linked.has(w.id));
  return (
    <section className="grid max-w-2xl gap-4 border-t pt-6" aria-label="Synthesis">
      <div className="space-y-1">
        <h3 className="text-sm font-medium">Synthesis</h3>
        <p className="text-xs text-muted-foreground">
          A chat model reads this bank&apos;s sources and keeps their lasting facts as memories,
          citing where each came from. It never changes facts people wrote.
        </p>
      </div>
      <p className="text-sm" role="status">
        {`${
          !state.synthesis
            ? "Off: nothing is synthesized."
            : state.failures
              ? `Failing (${state.failures} in a row): ${state.lastError}`
              : state.lastRunAt
                ? `Last ran ${formatDistanceToNow(timestampDate(state.lastRunAt), { addSuffix: true })}.`
                : "Waiting for its sources."
        } ${state.pendingConversations} conversations, ${state.pendingPages} pages waiting.`}
      </p>
      <form
        className="grid gap-4"
        aria-label="Synthesis settings"
        onSubmit={form.handleSubmit((values) =>
          act(() =>
            memory.setMemorySynthesis({
              bankId: bank.id,
              synthesis: {
                connectionId: values.connectionId,
                model: values.model.trim(),
                instructions: values.instructions.trim(),
              },
            }),
          ),
        )}
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="synthesis-connection">Inference connection</Label>
            <NativeSelect
              id="synthesis-connection"
              disabled={!editable}
              {...form.register("connectionId", { required: true })}
            >
              {!available.length && <option value="">No ready inference connection</option>}
              {available.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </NativeSelect>
          </div>
          <div className="space-y-2">
            <Label htmlFor="synthesis-model">Model</Label>
            <Input
              id="synthesis-model"
              list="synthesis-models"
              autoComplete="off"
              disabled={!editable}
              aria-invalid={!!form.formState.errors.model}
              {...form.register("model", { validate: (value) => !!value.trim() })}
            />
            <datalist id="synthesis-models">
              {models.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          </div>
        </div>
        <p className="-mt-2 text-xs text-muted-foreground">
          OpenAI, Azure OpenAI, Vercel AI Gateway, Baseten, Cerebras or Anthropic. Saving checks the
          model first. Each batch is one call to this model, billed to the connection, not yet shown
          in usage.
        </p>
        <div className="space-y-2">
          <Label htmlFor="synthesis-instructions">Instructions</Label>
          <Textarea
            id="synthesis-instructions"
            maxLength={2000}
            disabled={!editable}
            placeholder="What this bank should keep, for example: customers' preferences and the outcome of each support case."
            {...form.register("instructions")}
          />
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        {editable && (
          <div className="flex justify-end gap-2">
            {state.synthesis && (
              <>
                <Button
                  type="button"
                  variant="outline"
                  className="mr-auto"
                  onClick={() => void act(() => memory.clearMemorySynthesis({ bankId: bank.id }))}
                >
                  Turn off
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => void act(() => memory.runMemorySynthesis({ bankId: bank.id }))}
                >
                  Run now
                </Button>
              </>
            )}
            <Button
              type="submit"
              disabled={
                form.formState.isSubmitting ||
                !available.length ||
                (!!state.synthesis && !form.formState.isDirty)
              }
            >
              {form.formState.isSubmitting ? "Checking the model…" : "Save"}
            </Button>
          </div>
        )}
      </form>

      <div className="space-y-2" aria-label="Conversations it learns from">
        <h4 className="text-sm font-medium">Conversations</h4>
        <p className="text-xs text-muted-foreground">
          The conversations of agents linked here with read and write access, from when learning is
          turned on, once each conversation goes quiet.
        </p>
        {!writers.length && (
          <p className="text-xs text-muted-foreground">No agent may write to this bank.</p>
        )}
        <ul className="space-y-1">
          {writers.map((l) => (
            <li key={l.agentId} className="flex items-center gap-3 text-sm">
              <Switch
                aria-label={`Learn from ${registry.get(l.agentId)?.name ?? "this agent"}'s conversations`}
                checked={retaining.has(l.agentId)}
                disabled={!editable}
                onCheckedChange={(autoRetain) =>
                  void act(() =>
                    memory.setMemoryBankRetain({ bankId: bank.id, agentId: l.agentId, autoRetain }),
                  )
                }
              />
              <span>{registry.get(l.agentId)?.name ?? "An agent"}</span>
            </li>
          ))}
        </ul>
      </div>

      <div className="space-y-2" aria-label="Wikis it learns from">
        <h4 className="text-sm font-medium">Wikis</h4>
        <p className="text-xs text-muted-foreground">
          Every page now, then each change (one model call per page).
        </p>
        <ul className="space-y-1">
          {state.wikis.map((w) => (
            <li key={w.wikiId} className="flex items-center gap-2 text-sm">
              <span className="flex-1">{w.name}</span>
              {editable && (
                <RemoveButton
                  label={`Stop learning from ${w.name}`}
                  title={`Stop learning from ${w.name}?`}
                  description="Memories already synthesized from it stay."
                  confirmLabel="Stop"
                  onConfirm={() =>
                    act(() => memory.removeMemoryBankWiki({ bankId: bank.id, wikiId: w.wikiId }))
                  }
                />
              )}
            </li>
          ))}
        </ul>
        {editable && unlinked.length > 0 && (
          <NativeSelect
            aria-label="Learn from a wiki"
            value=""
            onChange={(event) => setLinking(unlinked.find((w) => w.id === event.target.value))}
          >
            <option value="">Learn from a wiki…</option>
            {unlinked.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </NativeSelect>
        )}
        <AlertDialog open={!!linking} onOpenChange={(open) => !open && setLinking(undefined)}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Learn from {linking?.name}?</AlertDialogTitle>
              <AlertDialogDescription>
                Its {linking?.pageCount} pages are each read{" "}
                {state.synthesis ? "now" : "once synthesis is on"}, one model call per page, then
                each page again when it changes.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <Button
                onClick={() => {
                  const wikiId = linking?.id ?? "";
                  setLinking(undefined);
                  void act(() => memory.setMemoryBankWiki({ bankId: bank.id, wikiId }));
                }}
              >
                Learn from it
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>

      {batches.length > 0 && (
        <div className="space-y-2" aria-label="Recent batches">
          <h4 className="text-sm font-medium">Recent batches</h4>
          <ol className="space-y-2">
            {batches.map((b) => (
              <li key={b.id} className="text-xs">
                <span className="flex flex-wrap items-center gap-2">
                  <Badge
                    variant={
                      b.status === MemorySynthesisBatchStatus.FAILED ? "destructive" : "secondary"
                    }
                  >
                    {STATUS[b.status]}
                  </Badge>
                  <SourceLink source={b} agentName={(id) => registry.get(id)?.name ?? "an agent"} />
                  {b.status === MemorySynthesisBatchStatus.APPLIED && (
                    <span className="text-muted-foreground">
                      {b.written} written, {b.superseded} replaced, {b.forgotten} forgotten,{" "}
                      {b.dropped} refused
                    </span>
                  )}
                  {b.at && (
                    <span className="text-muted-foreground">
                      {formatDistanceToNow(timestampDate(b.at), { addSuffix: true })}
                    </span>
                  )}
                </span>
                {b.memories.length > 0 && (
                  <span className="mt-0.5 flex flex-wrap gap-x-2">
                    {b.memories.map((m) =>
                      m.forgotten ? (
                        <span key={m.id} className="text-muted-foreground line-through">
                          {m.title}
                        </span>
                      ) : (
                        <Link
                          key={m.id}
                          to="/memory/$bankId/memories"
                          params={{ bankId: bank.id }}
                          search={{ memory: m.id }}
                          className="underline"
                        >
                          {m.title}
                        </Link>
                      ),
                    )}
                  </span>
                )}
                {b.notes && <span className="mt-0.5 block text-muted-foreground">{b.notes}</span>}
              </li>
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}

const STATUS: Record<MemorySynthesisBatchStatus, string> = {
  [MemorySynthesisBatchStatus.UNSPECIFIED]: "",
  [MemorySynthesisBatchStatus.APPLIED]: "Applied",
  [MemorySynthesisBatchStatus.FAILED]: "Failed",
  [MemorySynthesisBatchStatus.SKIPPED]: "Skipped",
};
/**
 * Where a memory or a batch came from, linked: a conversation with an agent, or a wiki page
 * revision.
 */
export function SourceLink({
  source: s,
  agentName,
}: {
  source: MemoryEvidence | MemorySynthesisBatch;
  agentName: (agentId: string) => string;
}) {
  if (s.wikiId && s.path) {
    const wiki = "wikiName" in s && s.wikiName ? `${s.wikiName}: ` : "";
    return (
      <Link
        to="/wikis/$wikiId/pages"
        params={{ wikiId: s.wikiId }}
        search={{ page: s.path }}
        className="underline"
      >
        {wiki}
        {s.path}, revision {s.revision}
      </Link>
    );
  }
  if (s.pageId) return <span>A wiki page, revision {s.revision}</span>;
  if (s.agentId && s.threadId)
    return (
      <Link
        to="/agent/$agentId/sessions"
        params={{ agentId: s.agentId }}
        search={{ session: s.threadId }}
        className="underline"
      >
        A conversation with {agentName(s.agentId)}
      </Link>
    );
  return <span>A conversation</span>;
}
