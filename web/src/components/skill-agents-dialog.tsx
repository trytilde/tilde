import { useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { AgentAvatar } from "./agent-avatar";
import { Badge } from "./ui/badge";
import { Input } from "./ui/input";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./ui/dialog";

/**
 * An agent with the skill, and whether it came with the whole group or on its own. Agents using
 * a connection carry no `via`.
 */
export type SkillAgent = { id: string; via?: "group" | "skill" };
const VIA = { all: "All", group: "Whole group", skill: "This skill" } as const;

/**
 * Every agent with a skill (or group, or connection), searchable by name and filterable by how
 * it got it. Each links to the agent's `tab`, where its use is configured.
 */
export function SkillAgentsDialog({
  open,
  onOpenChange,
  title,
  agents,
  registry,
  tab = "skills",
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  agents: SkillAgent[];
  registry: Map<string, Agent>;
  tab?: "skills" | "tools";
}) {
  const [query, setQuery] = useState("");
  const [via, setVia] = useState<keyof typeof VIA>("all");
  const mixed = agents.some((a) => a.via === "group") && agents.some((a) => a.via === "skill");
  const shown = useMemo(() => {
    const words = query.trim().toLowerCase();
    return agents
      .map((a) => ({ ...a, agent: registry.get(a.id) ?? { id: a.id, name: "Agent" } }))
      .filter((a) => via === "all" || a.via === via)
      .filter((a) => (a.agent.name ?? "").toLowerCase().includes(words))
      .sort((a, b) => (a.agent.name ?? "").localeCompare(b.agent.name ?? ""));
  }, [agents, registry, query, via]);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setQuery("");
          setVia("all");
        }
        onOpenChange(next);
      }}
    >
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Agents with {title}</DialogTitle>
          <DialogDescription>
            {agents.length} agent{agents.length === 1 ? "" : "s"} you can view.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            aria-label="Search agents"
            placeholder="Search agents"
            value={query}
            className="min-w-48 flex-1"
            onChange={(event) => setQuery(event.target.value)}
          />
          {mixed && (
            <Tabs value={via} onValueChange={(value) => setVia(value as keyof typeof VIA)}>
              <TabsList aria-label="How agents have it">
                {Object.entries(VIA).map(([id, label]) => (
                  <TabsTrigger key={id} value={id}>
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
          )}
        </div>
        <ul
          aria-label="Agents"
          className="m-0 grid max-h-[55dvh] list-none gap-2 overflow-y-auto p-0 sm:grid-cols-2"
        >
          {shown.map(({ agent, via: how }) => (
            <li key={agent.id}>
              <Link
                to={tab === "tools" ? "/agent/$agentId/tools" : "/agent/$agentId/skills"}
                params={{ agentId: agent.id }}
                className="flex items-center gap-3 rounded-lg border p-3 hover:bg-muted/50"
              >
                <AgentAvatar agent={agent} className="size-8" />
                <span className="min-w-0 flex-1 truncate text-sm font-medium">{agent.name}</span>
                {how && (
                  <Badge variant="outline" className="font-normal">
                    {VIA[how]}
                  </Badge>
                )}
              </Link>
            </li>
          ))}
          {!shown.length && (
            <li className="col-span-full py-6 text-center text-sm text-muted-foreground">
              No agents match.
            </li>
          )}
        </ul>
      </DialogContent>
    </Dialog>
  );
}
