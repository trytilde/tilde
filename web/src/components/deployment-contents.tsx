import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import type {
  DeclaredTool,
  DeploymentSkill,
  GetDeploymentContentsResponse,
} from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { PromptFormat } from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { deployments } from "@/client";
import { Badge } from "./ui/badge";
import { TildeLoader } from "@/components/loading-screen";

const formatLabels: Record<PromptFormat, string> = {
  [PromptFormat.UNSPECIFIED]: "",
  [PromptFormat.PLAIN]: "Plain",
  [PromptFormat.MUSTACHE]: "Mustache",
  [PromptFormat.BRACES]: "Braces",
  [PromptFormat.DYNAMIC]: "Dynamic",
};
export function PromptFormatBadge({ format }: { format: PromptFormat }) {
  if (!formatLabels[format]) return null;
  return <Badge variant="outline">{formatLabels[format]}</Badge>;
}
/** The prompt and skill versions and bundled tools one deployment shipped, fetched on mount. */
export function useDeploymentContents(agentId: string, deploymentId: string | undefined) {
  const [contents, setContents] = useState<GetDeploymentContentsResponse>();
  const [error, setError] = useState("");
  useEffect(() => {
    setContents(undefined);
    setError("");
    if (!deploymentId) return;
    const abort = new AbortController();
    void deployments
      .getDeploymentContents({ agentId, deploymentId }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) setContents(response);
      })
      .catch((e) => {
        if (!abort.signal.aborted)
          setError(e instanceof Error ? e.message : "Unable to load deployment contents.");
      });
    return () => abort.abort();
  }, [agentId, deploymentId]);
  return { contents, error };
}

function DeploymentSkillList({ skills }: { skills: DeploymentSkill[] }) {
  return (
    <ul aria-label="Deployment skills" className="m-0 divide-y rounded-lg border p-0">
      {skills.map((skill) => (
        <li key={skill.versionId} className="space-y-0.5 px-3 py-2">
          <span className="flex items-center gap-2">
            <span className="font-mono text-sm font-medium">{skill.name}</span>
            <Badge variant="secondary">v{skill.number}</Badge>
          </span>
          {skill.description && (
            <p className="m-0 text-xs text-muted-foreground">{skill.description}</p>
          )}
          {skill.origin && (
            <p className="m-0 font-mono text-[11px] text-muted-foreground">{skill.origin}</p>
          )}
        </li>
      ))}
      {!skills.length && (
        <li className="px-3 py-3 text-xs text-muted-foreground">No skills in this deployment.</li>
      )}
    </ul>
  );
}

function DeploymentToolList({ tools }: { tools: DeclaredTool[] }) {
  return (
    <ul aria-label="Deployment tools" className="m-0 divide-y rounded-lg border p-0">
      {tools.map((tool) => (
        <li key={tool.name} className="space-y-0.5 px-3 py-2">
          <span className="font-mono text-sm font-medium">{tool.name}</span>
          <p className="m-0 text-xs text-muted-foreground">{tool.summary || tool.description}</p>
          {tool.origin && (
            <p className="m-0 font-mono text-[11px] text-muted-foreground">{tool.origin}</p>
          )}
        </li>
      ))}
      {!tools.length && (
        <li className="px-3 py-3 text-xs text-muted-foreground">No tools in this deployment.</li>
      )}
    </ul>
  );
}

/** What a deployment shipped: the prompt and skill versions and bundled tools `tilde deploy` registered. */
export function DeploymentContents({
  agentId,
  deploymentId,
}: {
  agentId: string;
  deploymentId: string;
}) {
  const { contents, error } = useDeploymentContents(agentId, deploymentId);
  if (error)
    return (
      <p role="alert" className="text-sm text-destructive">
        {error}
      </p>
    );
  if (!contents) return <TildeLoader />;
  return (
    <div className="space-y-5">
      <section aria-label="Prompts" className="space-y-2">
        <h3 className="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          Prompts
        </h3>
        <ul aria-label="Deployment prompts" className="m-0 divide-y rounded-lg border p-0">
          {contents.prompts.map((prompt) => (
            <li key={prompt.version?.id ?? prompt.promptId} className="space-y-0.5 px-3 py-2">
              <span className="flex items-center gap-2">
                <Link
                  to="/agent/$agentId/prompts"
                  params={{ agentId }}
                  search={{ prompt: prompt.promptId }}
                  className="font-mono text-sm font-medium hover:underline"
                >
                  {prompt.name}
                </Link>
                {prompt.version && <Badge variant="secondary">v{prompt.version.number}</Badge>}
                {prompt.version && <PromptFormatBadge format={prompt.version.format} />}
              </span>
              {prompt.version?.origin && (
                <p className="m-0 font-mono text-[11px] text-muted-foreground">
                  {prompt.version.origin}
                </p>
              )}
            </li>
          ))}
          {!contents.prompts.length && (
            <li className="px-3 py-3 text-xs text-muted-foreground">
              No prompts in this deployment.
            </li>
          )}
        </ul>
      </section>
      <section aria-label="Skills" className="space-y-2">
        <h3 className="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          Skills
        </h3>
        <DeploymentSkillList skills={contents.skills} />
      </section>
      <section aria-label="Tools" className="space-y-2">
        <h3 className="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          Tools
        </h3>
        <DeploymentToolList tools={contents.tools} />
      </section>
    </div>
  );
}
