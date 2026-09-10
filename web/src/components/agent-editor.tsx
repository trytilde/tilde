import { AgentConnections } from "./agent-connections";
import { randomUUID } from "@/lib/browser-crypto";
import { useRef, useState, type FormEvent } from "react";
import type { Agent } from "@/gen/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function AgentEditor({
  agent,
  onClose,
  onSaved,
}: {
  agent: Agent | null;
  onClose: () => void;
  onSaved: (created: boolean) => void;
}) {
  const capabilityOptions = [
    ["agents.read", "Read agents", true],
    ["agents.create", "Create agents", false],
    ["agents.update", "Update agents", true],
    ["agents.delete", "Delete agents", true],
    ["agents.invoke", "Invoke agents in this thread", true],
    ["agents.grant_capabilities", "Grant capabilities", true],
    ["thread.read", "Read current thread", false],
    ["work.read", "Read goals and tasks", false],
    ["work.write", "Manage goals and tasks", false],
    ["run.update", "Update own run", false],
    ["tools.invoke", "Use provider tools", true],
  ] as const;
  const [grants, setGrants] = useState<Record<string, { mode: string; ids: string[] }>>(() =>
    Object.fromEntries(
      Object.entries(agent?.capabilities?.grants ?? {}).map(([name, scope]) => [
        name,
        { mode: scope.mode, ids: scope.ids },
      ]),
    ),
  );
  const capabilities = { grants };
  const [name, setName] = useState(agent?.name ?? "");
  const [endpoint, setEndpoint] = useState(agent?.endpointUrl ?? "");
  const [signingKey, setSigningKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const id = useRef(agent?.id ?? randomUUID());
  async function save(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      if (agent)
        await agents.updateAgent({ id: agent.id, name, endpointUrl: endpoint, capabilities });
      else
        await agents.createAgent({
          capabilities,
          id: id.current,
          name,
          endpointUrl: endpoint || undefined,
          webhookSigningKey: signingKey,
        });
      setSigningKey("");
      onSaved(!agent);
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to save agent.");
    } finally {
      setSaving(false);
    }
  }
  const form = (
    <form onSubmit={save} className="grid gap-5">
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="grid gap-2">
        <Label htmlFor="agent-name">Name</Label>
        <Input
          id="agent-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          required
          maxLength={200}
          disabled={saving}
          autoFocus
          placeholder="Research assistant"
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor="agent-endpoint">
          Endpoint <span className="font-normal text-muted-foreground">(optional)</span>
        </Label>
        <Input
          id="agent-endpoint"
          type="url"
          value={endpoint}
          onChange={(event) => setEndpoint(event.target.value)}
          disabled={saving}
          placeholder="https://agent.example.com"
        />
      </div>
      {!agent && (
        <div className="grid gap-2">
          <Label htmlFor="agent-key">Webhook signing key</Label>
          <Input
            id="agent-key"
            type="password"
            value={signingKey}
            onChange={(event) => setSigningKey(event.target.value)}
            required
            minLength={32}
            maxLength={1024}
            pattern="[!-~]+"
            autoComplete="new-password"
            disabled={saving}
            aria-describedby="agent-key-help"
          />
          <p id="agent-key-help" className="text-xs text-muted-foreground">
            Supply your own securely generated key and configure the same key in your agent. It is
            never returned after saving.
          </p>
        </div>
      )}
      <fieldset className="grid gap-3">
        <legend className="mb-2 font-medium">Capabilities</legend>
        <p className="text-xs text-muted-foreground">
          All actions deny by default. Thread and work access stays within the invocation.
        </p>
        {capabilityOptions.map(([key, label, targeted]) => (
          <div key={key} className="grid gap-1">
            <label htmlFor={`cap-${key}`} className="text-sm">
              {label}
            </label>
            <select
              id={`cap-${key}`}
              className="rounded border p-2 text-sm"
              disabled={saving}
              value={grants[key]?.mode ?? "none"}
              onChange={(event) =>
                setGrants({ ...grants, [key]: { mode: event.target.value, ids: [] } })
              }
            >
              <option value="none">None</option>
              <option value="any">Any</option>
              {targeted && <option value="only">Only specified targets</option>}
            </select>
            {grants[key]?.mode === "only" && (
              <Input
                aria-label={`${label} targets`}
                placeholder={
                  key === "tools.invoke"
                    ? "Tool names, separated by commas"
                    : "Agent UUIDs, separated by commas"
                }
                value={grants[key].ids.join(",")}
                disabled={saving}
                onChange={(event) =>
                  setGrants({
                    ...grants,
                    [key]: {
                      mode: "only",
                      ids: event.target.value.split(",").map((value) => value.trim()),
                    },
                  })
                }
              />
            )}
          </div>
        ))}
      </fieldset>
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" disabled={saving} onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={saving || !name.trim()}>
          {saving ? "Saving…" : agent ? "Save changes" : "Create agent"}
        </Button>
      </div>
    </form>
  );
  if (!agent) {
    return (
      <section
        className="flex flex-1 flex-col px-4 py-6 lg:px-6"
        aria-labelledby="create-agent-title"
      >
        <div className="mx-auto grid w-full max-w-2xl gap-6">
          <header className="grid gap-2">
            <h1 id="create-agent-title" className="text-2xl font-semibold">
              Create agent
            </h1>
            <p className="text-sm text-muted-foreground">
              Register an agent with its endpoint and shared signing key.
            </p>
          </header>
          {form}
        </div>
      </section>
    );
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !saving) onClose();
      }}
    >
      <DialogContent className="sm:max-w-lg max-h-[90vh] overflow-y-auto" showCloseButton={!saving}>
        <DialogHeader>
          <DialogTitle>Edit agent</DialogTitle>
          <DialogDescription>Update this agent’s name and endpoint.</DialogDescription>
        </DialogHeader>
        {form}
        <AgentConnections agentId={agent.id} />
      </DialogContent>
    </Dialog>
  );
}
