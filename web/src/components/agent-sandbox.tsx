import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { NativeSelect, useForm } from "@trytilde/connection-ui";
import type { SandboxBlueprint } from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { sandboxes } from "@/client";
import { InlineSaving, type InlineSavingState } from "./inline-saving";
import { REUSE, reuseOf } from "./sandboxes";
import { message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/**
 * The agent's sandbox setting: which blueprint its runs get a sandbox of, if any. Changing or
 * removing it terminates the agent's existing sandboxes, so either is confirmed first.
 */
export function AgentSandbox({ agentId }: { agentId: string }) {
  const [blueprints, setBlueprints] = useState<SandboxBlueprint[]>([]);
  // The saved blueprint ID; "" for none.
  const [saved, setSaved] = useState<string>();
  const form = useForm<{ blueprintId: string }>({ defaultValues: { blueprintId: "" } });
  const [confirming, setConfirming] = useState<string>();
  const [feedback, setFeedback] = useState<{
    state: InlineSavingState;
    attempt: number;
    error?: string;
  }>({ state: "idle", attempt: 0 });
  useEffect(() => {
    const abort = new AbortController();
    void Promise.all([
      sandboxes.getAgentSandbox({ agentId }, { signal: abort.signal }),
      sandboxes.listSandboxBlueprints({}, { signal: abort.signal }),
    ])
      .then(([{ sandbox }, list]) => {
        setBlueprints(list.blueprints);
        setSaved(sandbox?.blueprintId ?? "");
        form.reset({ blueprintId: sandbox?.blueprintId ?? "" });
      })
      .catch((e) => {
        if (!abort.signal.aborted)
          setFeedback((current) => ({ ...current, state: "error", error: message(e) }));
      });
    return () => abort.abort();
  }, [agentId]);
  async function change(blueprintId: string) {
    setConfirming(undefined);
    form.setValue("blueprintId", blueprintId);
    setFeedback((current) => ({ state: "saving", attempt: current.attempt + 1 }));
    try {
      if (blueprintId) await sandboxes.setAgentSandbox({ agentId, blueprintId });
      else await sandboxes.removeAgentSandbox({ agentId });
      setSaved(blueprintId);
      setFeedback((current) => ({ ...current, state: "success" }));
    } catch (e) {
      form.setValue("blueprintId", saved ?? "");
      setFeedback((current) => ({ ...current, state: "error", error: message(e) }));
    }
  }
  const chosen = blueprints.find((b) => b.id === form.watch("blueprintId"));
  const confirmed = blueprints.find((b) => b.id === confirming);
  return (
    <section className="grid gap-3 border-t pt-6" aria-label="Sandbox">
      <div className="flex items-center gap-2">
        <Label htmlFor="agent-sandbox">Sandbox</Label>
        <InlineSaving
          state={feedback.state}
          label="Sandbox"
          resetKey={feedback.attempt}
          error={feedback.error}
        />
      </div>
      <p id="agent-sandbox-help" className="text-xs leading-relaxed text-muted-foreground">
        Runs of the agent get a VM launched from a blueprint, and the sandbox tools (exec, files,
        patches, search) as a tool source.{" "}
        {chosen ? (
          <>
            {REUSE[reuseOf(chosen.reuse)].label}: {REUSE[reuseOf(chosen.reuse)].description}{" "}
            <Link
              to="/sandboxes/$blueprintId/settings"
              params={{ blueprintId: chosen.id }}
              className="text-foreground underline"
            >
              Open {chosen.name}
            </Link>
          </>
        ) : !blueprints.length && saved !== undefined ? (
          <>
            No blueprints yet.{" "}
            <Link to="/sandboxes" className="text-foreground underline">
              Create one
            </Link>
            .
          </>
        ) : null}
      </p>
      <NativeSelect
        id="agent-sandbox"
        className="max-w-sm"
        aria-describedby="agent-sandbox-help"
        {...form.register("blueprintId")}
        value={form.watch("blueprintId")}
        disabled={saved === undefined || feedback.state === "saving"}
        onChange={(event) => {
          const next = event.target.value;
          // Without a sandbox there is nothing to terminate.
          if (saved) setConfirming(next);
          else void change(next);
        }}
      >
        <option value="">No sandbox</option>
        {blueprints.map((blueprint) => (
          <option key={blueprint.id} value={blueprint.id}>
            {blueprint.name}
          </option>
        ))}
      </NativeSelect>
      <AlertDialog
        open={confirming !== undefined}
        onOpenChange={(open) => !open && setConfirming(undefined)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmed ? `Use ${confirmed.name} instead?` : "Remove the agent's sandbox?"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              The agent's existing sandboxes are terminated, with everything on them.
              {confirmed
                ? " Its next runs launch new ones from this blueprint."
                : " It also loses the sandbox tools."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <Button variant="destructive" onClick={() => void change(confirming ?? "")}>
              {confirmed ? "Change sandbox" : "Remove sandbox"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
