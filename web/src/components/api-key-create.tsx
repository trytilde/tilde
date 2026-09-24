import { useState } from "react";
import { ArrowLeftIcon, CopyIcon } from "lucide-react";
import { apiKeys } from "@/client";
import { AgentTargetPicker } from "@/components/agent-target-picker";
import { useCaller } from "@/hooks/use-caller";
import { roleId, type AgentRole } from "@/lib/access";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

// The form's "nothing on this row" choice; never sent.
const NONE = "none";
type Scope = "all" | "selected";
type RowState = { choice: string; scope: Scope; ids: string[] };
const initial: RowState = { choice: NONE, scope: "selected", ids: [] };
const roleHelp: Record<string, string> = {
  reader: "Find agents and read everything about them.",
  editor: "Change agent settings, connections and identity access, and share them.",
  deployer: "Register, promote and retire deployments and issue deployment tokens.",
};

/**
 * Full-page key creation laid out like agent capabilities: a toggle per row and its
 * explanation above it. Each row yields roles, narrowed to all agents or selected ones.
 */
export function ApiKeyCreate({ onDone }: { onDone: () => void }) {
  const caller = useCaller();
  const [name, setName] = useState("");
  const [agents, setAgents] = useState<RowState>(initial);
  const [deploy, setDeploy] = useState<RowState>(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [secret, setSecret] = useState("");
  const [copied, setCopied] = useState(false);
  function rolesFor(state: RowState, role: AgentRole) {
    if (state.choice === NONE) return [];
    return (state.scope === "all" ? [""] : state.ids).map((id) => roleId(id, role));
  }
  async function create() {
    if (!name.trim()) {
      setError("Enter a name.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const response = await apiKeys.createApiKey({
        name: name.trim(),
        roleIds: [
          ...rolesFor(agents, agents.choice === "editor" ? "editor" : "reader"),
          ...rolesFor(deploy, "deployer"),
        ],
      });
      setSecret(response.secret);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to create API key.");
    } finally {
      setBusy(false);
    }
  }
  async function copy() {
    try {
      await navigator.clipboard.writeText(secret);
      setCopied(true);
    } catch {
      setError("Unable to copy. Select the key and copy it manually.");
    }
  }
  function row(
    label: string,
    help: string,
    choices: { value: string; label: string; help: string }[],
    state: RowState,
    setState: (next: RowState) => void,
  ) {
    const off = state.choice === NONE;
    return (
      <>
        <div className="flex min-w-0 flex-col gap-2">
          <div className="min-w-0">
            <h3 className="text-sm font-medium">{label}</h3>
            <p className="text-sm text-muted-foreground">
              {off ? help : choices.find((c) => c.value === state.choice)?.help}
            </p>
          </div>
          <Tabs
            className="mt-auto"
            value={state.choice}
            onValueChange={(value) => setState({ ...state, choice: String(value) })}
          >
            <TabsList aria-label={label}>
              {choices.map((choice) => (
                <TabsTrigger key={choice.value} value={choice.value} disabled={busy}>
                  {choice.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        </div>
        {/* The right column always answers "which agents" for the row beside it; inert until one is chosen. */}
        <div
          aria-hidden
          className={`hidden h-9 items-center self-end text-muted-foreground md:flex ${off ? "opacity-50" : ""}`}
        >
          {/* Lucide's arrow stroke and caps, drawn at a longer length than the glyph allows. */}
          <svg
            width="56"
            height="16"
            viewBox="0 0 56 16"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M2 8h49" />
            <path d="m45 2 6 6-6 6" />
          </svg>
        </div>
        <div
          className="flex min-w-0 flex-col gap-2 aria-disabled:pointer-events-none aria-disabled:opacity-50"
          aria-disabled={off || undefined}
        >
          <div className="min-w-0">
            <h3 className="text-sm font-medium">Which agents</h3>
            <p className="text-sm text-muted-foreground">
              {off
                ? "Choose an option first."
                : state.scope === "all"
                  ? "Every agent, including ones created later."
                  : "Only the agents chosen here."}
            </p>
          </div>
          <div className="mt-auto flex h-9 items-center gap-3 [&_.py-1]:py-0">
            <Tabs
              value={state.scope}
              onValueChange={(value) => setState({ ...state, scope: value as Scope })}
            >
              <TabsList aria-label={`${label} scope`}>
                {/* Reaching every agent is an administrator's to give. */}
                <TabsTrigger value="all" disabled={busy || off || !caller.admin}>
                  All
                </TabsTrigger>
                <TabsTrigger value="selected" disabled={busy || off}>
                  Selected
                </TabsTrigger>
              </TabsList>
            </Tabs>
            {!off && state.scope === "selected" && (
              <AgentTargetPicker
                label={label}
                value={state.ids}
                onConfirm={async (ids) => setState({ ...state, ids })}
                disabled={busy}
              />
            )}
          </div>
        </div>
      </>
    );
  }
  if (secret)
    return (
      <section className="flex flex-1 flex-col px-4 py-6 lg:px-8" aria-label="Save your API key">
        <div className="mx-auto grid w-full max-w-6xl gap-6">
          <header className="grid gap-1">
            <h1 className="text-3xl font-semibold">Save your API key</h1>
            <p className="text-sm text-muted-foreground">
              Copy this key now. It will not be shown again.
            </p>
          </header>
          <div className="grid max-w-2xl gap-3">
            <Label htmlFor="api-key-secret">API key</Label>
            <Input
              id="api-key-secret"
              readOnly
              value={secret}
              className="font-mono text-xs"
              onFocus={(event) => event.target.select()}
            />
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => void copy()}>
                <CopyIcon />
                {copied ? "Copied" : "Copy key"}
              </Button>
              <Button onClick={onDone}>Done</Button>
            </div>
          </div>
        </div>
      </section>
    );
  return (
    <section className="flex flex-1 flex-col px-4 py-6 lg:px-8" aria-label="Create API key">
      <form
        className="mx-auto grid w-full max-w-6xl gap-8"
        onSubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <header className="grid gap-3">
          <Button
            type="button"
            variant="ghost"
            className="mb-2 w-fit -ml-2"
            disabled={busy}
            onClick={onDone}
          >
            <ArrowLeftIcon /> Back to API keys
          </Button>
          <h1 className="text-2xl font-semibold">Create API key</h1>
          <p className="text-sm text-muted-foreground">
            A key holds only the roles given here, plus anything shared with it later from an
            agent&apos;s Share button.
          </p>
          <div className="grid max-w-md gap-2">
            <Label htmlFor="api-key-name">Name</Label>
            <Input
              id="api-key-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              required
              maxLength={100}
              disabled={busy}
              autoFocus
              placeholder="Where this key is used"
            />
          </div>
        </header>
        <fieldset className="grid gap-x-6 gap-y-6 md:grid-cols-[minmax(0,1fr)_max-content_minmax(0,1fr)]">
          <legend className="sr-only">Access</legend>
          {row(
            "Agents",
            "The key cannot see any agents.",
            [
              { value: NONE, label: "None", help: "" },
              { value: "reader", label: "Reader", help: roleHelp.reader },
              { value: "editor", label: "Editor", help: roleHelp.editor },
            ],
            agents,
            setAgents,
          )}
          {row(
            "Manage agent deployments",
            "The key cannot deploy agents.",
            [
              { value: NONE, label: "No", help: "" },
              { value: "deployer", label: "Yes", help: roleHelp.deployer },
            ],
            deploy,
            setDeploy,
          )}
        </fieldset>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onDone} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? "Creating…" : "Create key"}
          </Button>
        </div>
      </form>
    </section>
  );
}
