import type { ConnectionUsage } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { Badge } from "./ui/badge";

const KINDS: Record<string, string> = {
  agent: "Agent",
  personal_agent: "Agent, personal use",
  tool_source: "Agent tool source",
  sandbox_tool_source: "Sandbox tool source",
  routine: "Routine",
  sandbox_blueprint: "Sandbox blueprint",
  sandbox: "Sandbox",
  sandbox_egress: "Sandbox egress",
  memory: "Memory",
  memory_synthesis: "Memory synthesis",
};

/** What uses a connection: it cannot be disconnected until each is removed. */
export function ConnectionUsages({ usages }: { usages: ConnectionUsage[] }) {
  return (
    <ul aria-label="In use by" className="m-0 flex list-none flex-col gap-1.5 p-0 text-sm">
      {usages.map((usage, index) => (
        <li
          key={`${usage.kind}-${usage.id}-${usage.detail}-${index}`}
          className="flex min-w-0 items-baseline gap-2"
        >
          <Badge variant="secondary" className="shrink-0">
            {KINDS[usage.kind] ?? usage.kind}
          </Badge>
          <span className="truncate font-medium">{usage.name}</span>
          {usage.detail && <span className="truncate text-muted-foreground">{usage.detail}</span>}
        </li>
      ))}
    </ul>
  );
}

/** The confirmation's explanation of a disconnect: what it does, or what blocks it. */
export function DisconnectDescription({ usages }: { usages: ConnectionUsage[] }) {
  if (usages.length === 0)
    return <>Its credentials are removed and its tools disappear from every agent using it.</>;
  return <>It is still in use. Remove it from each of these first, then disconnect it.</>;
}
