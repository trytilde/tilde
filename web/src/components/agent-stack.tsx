import { useEffect, useState } from "react";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { agents } from "@/client";
import { AgentAvatar } from "./agent-avatar";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";

/** Avatars shown before the rest collapse into "+N more". */
const STACKED = 3;

/** Every agent, by ID, for naming the agents a table row lists. */
export function useAgentRegistry() {
  const [registry, setRegistry] = useState<Map<string, Agent>>(() => new Map());
  useEffect(() => {
    const abort = new AbortController();
    void (async () => {
      const found = new Map<string, Agent>();
      let pageToken = "";
      do {
        const page = await agents.listAgents(
          { pageSize: 100, pageToken },
          { signal: abort.signal },
        );
        for (const agent of page.agents) found.set(agent.id, agent);
        pageToken = page.nextPageToken;
      } while (pageToken);
      setRegistry(found);
    })().catch(() => {});
    return () => abort.abort();
  }, []);
  return registry;
}

/** Overlapping avatars of the first few agents, then how many more. */
export function AgentStack({
  ids,
  registry,
  onOpen,
}: {
  ids: string[];
  registry: Map<string, Agent>;
  onOpen: () => void;
}) {
  if (!ids.length) return <span className="text-muted-foreground">—</span>;
  const shown = ids.slice(0, STACKED).map((id) => registry.get(id) ?? { id, name: "Agent" });
  const names = ids.map((id) => registry.get(id)?.name ?? "Agent").join(", ");
  return (
    // Negative margins keep the stack from growing the row; rings, unlike borders, take no space.
    <button
      type="button"
      className="-my-1 flex cursor-pointer items-center gap-1.5 rounded-md p-0.5 hover:bg-muted"
      aria-label={`Agents: ${names}`}
      onClick={(event) => {
        event.stopPropagation();
        onOpen();
      }}
    >
      <span className="flex items-center -space-x-1">
        {shown.map((agent) => (
          <Tooltip key={agent.id}>
            <TooltipTrigger
              render={<span className="flex rounded-full bg-background ring-1 ring-background" />}
            >
              <AgentAvatar agent={agent} className="size-4" />
            </TooltipTrigger>
            <TooltipContent>{agent.name}</TooltipContent>
          </Tooltip>
        ))}
      </span>
      {ids.length > STACKED && (
        <span className="whitespace-nowrap text-xs text-muted-foreground">
          +{ids.length - STACKED} more
        </span>
      )}
    </button>
  );
}
