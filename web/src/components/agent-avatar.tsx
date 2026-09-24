import { AgentAvatar as GeneratedAvatar } from "@trytilde/agent-avatar";
import { useState } from "react";

/** The same identity renders consistently in the registry, header and target picker. */
export function AgentAvatar({
  agent,
  className = "size-9",
  animated = false,
}: {
  agent: { id: string; name?: string; avatarSeed?: string; avatarUrl?: string };
  className?: string;
  animated?: boolean;
}) {
  const [failedUrl, setFailedUrl] = useState<string>();
  return agent.avatarUrl && failedUrl !== agent.avatarUrl ? (
    <img
      src={agent.avatarUrl}
      alt=""
      className={`shrink-0 rounded-full object-cover ${className}`}
      onError={() => setFailedUrl(agent.avatarUrl)}
    />
  ) : (
    <GeneratedAvatar id={agent.avatarSeed || agent.id} className={className} paused={!animated} />
  );
}
