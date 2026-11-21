import { useState } from "react";
import { useForm } from "react-hook-form";
import { CheckIcon, CopyIcon } from "lucide-react";
import { Harness, type Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { TildeChatKey } from "./tilde-chat-provider";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

/** The harnesses a local agent runs in: its name, and its name to `tilde local install`. */
const HARNESSES: Partial<Record<Harness, { label: string; cli: string }>> = {
  [Harness.CLAUDE_CODE]: { label: "Claude Code", cli: "claude-code" },
  [Harness.CODEX]: { label: "Codex", cli: "codex" },
  [Harness.CLAUDE_DESKTOP]: { label: "Claude Desktop", cli: "claude-desktop" },
  [Harness.CHATGPT_DESKTOP]: { label: "ChatGPT Desktop", cli: "chatgpt-desktop" },
};
export const harnessLabel = (harness: Harness | undefined) =>
  (harness !== undefined && HARNESSES[harness]?.label) || "Local agent";

type Form = { name: string; harness: string };

/** Registers a local agent: the harness it runs in and a name. Setup continues on its page. */
export function NewLocalAgentDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (agent: Agent) => void;
}) {
  const form = useForm<Form>({ defaultValues: { name: "", harness: String(Harness.CLAUDE_CODE) } });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const harness = Number(form.watch("harness")) as Harness;
  const items = Object.entries(HARNESSES).map(([value, { label }]) => ({ value, label }));
  function close() {
    onOpenChange(false);
    setError("");
    form.reset();
  }
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Connect a local agent</DialogTitle>
          <DialogDescription>
            An agent that runs on your machine, in a harness such as Claude Code. Tilde never runs
            it: the harness reports its sessions here and gets the tools and skills you give the
            agent.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setBusy(true);
            setError("");
            try {
              const { agent } = await agents.createAgent({
                name: values.name.trim() || harnessLabel(harness),
                harness,
              });
              close();
              if (agent) onCreated(agent);
            } catch (e) {
              setError(e instanceof Error ? e.message : "Unable to connect the agent.");
            } finally {
              setBusy(false);
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="local-agent-harness">Harness</Label>
            <Select
              value={form.watch("harness")}
              onValueChange={(value) => value && form.setValue("harness", value)}
              items={items}
            >
              <SelectTrigger id="local-agent-harness" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {items.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label htmlFor="local-agent-name">Name</Label>
            <Input
              id="local-agent-name"
              placeholder={harnessLabel(harness)}
              maxLength={200}
              {...form.register("name")}
            />
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Connect
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Command({ label, text }: { label: string; text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-start gap-2 rounded-lg border bg-muted/40 p-3">
      <code className="min-w-0 flex-1 font-mono text-xs break-all whitespace-pre-wrap">{text}</code>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Copy ${label}`}
        onClick={() => void navigator.clipboard.writeText(text).then(() => setCopied(true))}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
    </div>
  );
}

/**
 * How a local agent's harness is connected, and what its adapter supports: the rest of the
 * agent's tabs follow the same features.
 */
export function LocalAgentSetup({ agent }: { agent: Agent }) {
  const harness = agent.local?.harness;
  const features = agent.local?.features;
  const cli = (harness !== undefined && HARNESSES[harness]?.cli) || "";
  let gateway = window.location.origin;
  const supported = [
    features?.sessions && "Its sessions are reported to Tilde as threads.",
    features?.traces && "Each turn is a trace.",
    features?.tools && "The agent's tools are served to it over MCP.",
    features?.skills && "The agent's skills are installed into its skill folder.",
    features?.logs && "Its logs are collected.",
  ].filter(Boolean);
  return (
    <section className="grid max-w-3xl gap-6" aria-label="Local agent setup">
      <div className="grid gap-1">
        <h2 className="text-base font-semibold">Runs in {harnessLabel(harness)}</h2>
        <p className="text-sm text-muted-foreground">
          This agent runs on a person's machine, not in Tilde. {supported.join(" ")}
        </p>
      </div>
      <ol className="grid list-decimal gap-5 pl-5 text-sm">
        <li className="grid gap-2">
          <span className="flex items-center gap-2">
            Copy the agent's key. It is what the harness authenticates with.
            <TildeChatKey
              agentId={agent.id}
              title="Agent key"
              description="The key this agent's harness authenticates with. It is stored on your machine by tilde local install."
            />
          </span>
        </li>
        {harness === Harness.CHATGPT_DESKTOP ? (
          <li className="grid gap-2">
            <span>
              ChatGPT takes remote MCP servers only. In ChatGPT, open Settings → Apps → Create and
              add this server, with the agent's key as its bearer token.
            </span>
            <Command label="MCP server URL" text={`${gateway}/agents/${agent.id}/mcp`} />
          </li>
        ) : (
          <>
            <li className="grid gap-2">
              <span>Install the Tilde CLI on the machine {harnessLabel(harness)} runs on.</span>
              <Command label="install command" text="npm install -g @trytilde/cli" />
            </li>
            <li className="grid gap-2">
              <span>Connect the harness. It asks for the agent's key.</span>
              <Command
                label="connect command"
                text={`tilde local install ${cli} --url ${gateway} --agent-id ${agent.id}`}
              />
            </li>
          </>
        )}
      </ol>
    </section>
  );
}
