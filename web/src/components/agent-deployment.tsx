import { useEffect, useState } from "react";
import { useForm } from "@trytilde/connection-ui";
import { deployments } from "@/client";
import {
  DeploymentMode,
  SidecarFailureMode,
  type SidecarNode,
} from "@/gen/tilde/types/v1/deployment_pb.js";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

type Fields = {
  mode: DeploymentMode;
  endpointUrl: string;
  failureMode: SidecarFailureMode;
};
export function AgentDeployment({ agentId, paused }: { agentId: string; paused: boolean }) {
  const form = useForm<Fields>({
    defaultValues: {
      mode: DeploymentMode.GATEWAY,
      endpointUrl: "",
      failureMode: SidecarFailureMode.REASSIGN,
    },
  });
  const [loadedMode, setLoadedMode] = useState<DeploymentMode>();
  const [nodes, setNodes] = useState<SidecarNode[]>([]);
  const [token, setToken] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [issuing, setIssuing] = useState(false);
  const mode = form.watch("mode");
  useEffect(() => {
    const abort = new AbortController();
    setToken("");
    setNotice("");
    setError("");
    setNodes([]);
    setLoadedMode(undefined);
    void deployments
      .getDeployment({ agentId }, { signal: abort.signal })
      .then(({ deployment, nodes }) => {
        if (!deployment || abort.signal.aborted) return;
        form.reset({
          mode: deployment.mode,
          endpointUrl: deployment.endpointUrl ?? "",
          failureMode: deployment.failureMode,
        });
        setLoadedMode(deployment.mode);
        setNodes(nodes);
      })
      .catch((error) => {
        if (!abort.signal.aborted)
          setError(error instanceof Error ? error.message : "Unable to load deployment.");
      });
    return () => abort.abort();
  }, [agentId, form.reset]);
  const save = form.handleSubmit(async (values) => {
    setError("");
    setNotice("");
    try {
      const { deployment } = await deployments.setDeployment({
        agentId,
        ...values,
        endpointUrl: values.mode === DeploymentMode.GATEWAY ? values.endpointUrl : undefined,
      });
      if (!deployment) throw new Error("Deployment was not returned.");
      setLoadedMode(deployment.mode);
      setNotice("Deployment saved.");
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to save deployment.");
    }
  });
  async function issueToken() {
    setIssuing(true);
    setError("");
    try {
      const response = await deployments.issueDeploymentToken({ agentId });
      setToken(response.token);
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to issue deployment token.");
    } finally {
      setIssuing(false);
    }
  }
  return (
    <section className="max-w-2xl space-y-6 py-6">
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      {loadedMode === undefined ? (
        <p role="status">Loading deployment…</p>
      ) : (
        <form onSubmit={save} className="space-y-5">
          <div className="space-y-2">
            <Label>Deployment mode</Label>
            <Tabs
              value={mode}
              onValueChange={(value) =>
                form.setValue("mode", Number(value) as DeploymentMode, { shouldDirty: true })
              }
            >
              <TabsList aria-label="Deployment mode">
                <TabsTrigger
                  value={DeploymentMode.GATEWAY}
                  disabled={!paused && loadedMode !== DeploymentMode.GATEWAY}
                >
                  Gateway
                </TabsTrigger>
                <TabsTrigger
                  value={DeploymentMode.SIDECAR}
                  disabled={!paused && loadedMode !== DeploymentMode.SIDECAR}
                >
                  Sidecar
                </TabsTrigger>
              </TabsList>
            </Tabs>
            {!paused && (
              <p className="text-sm text-muted-foreground">
                Pause the agent to change its deployment mode.
              </p>
            )}
          </div>
          {mode === DeploymentMode.GATEWAY ? (
            <div className="space-y-2">
              <Label htmlFor="deployment-endpoint">Agent endpoint URL</Label>
              <Input
                id="deployment-endpoint"
                type="url"
                {...form.register("endpointUrl", { required: true })}
              />
            </div>
          ) : (
            <>
              <div className="space-y-2">
                <Label>When an owner fails</Label>
                <Tabs
                  value={form.watch("failureMode")}
                  onValueChange={(value) =>
                    form.setValue("failureMode", Number(value) as SidecarFailureMode, {
                      shouldDirty: true,
                    })
                  }
                >
                  <TabsList aria-label="Failure mode">
                    <TabsTrigger value={SidecarFailureMode.REASSIGN}>
                      Assign to new node
                    </TabsTrigger>
                    <TabsTrigger value={SidecarFailureMode.STOP}>Stop</TabsTrigger>
                  </TabsList>
                </Tabs>
              </div>
            </>
          )}
          <Button type="submit" disabled={form.formState.isSubmitting}>
            {form.formState.isSubmitting ? "Saving…" : "Save deployment"}
          </Button>
        </form>
      )}
      {loadedMode === DeploymentMode.SIDECAR && (
        <>
          <div className="space-y-3">
            <h3 className="font-medium">Register sidecars</h3>
            <p className="text-sm text-muted-foreground">
              Use the same deployment token for every replica of this agent. Issuing another token
              replaces the token used for registration.
            </p>
            <Button variant="outline" onClick={() => void issueToken()} disabled={issuing}>
              {issuing ? "Issuing…" : "Issue deployment token"}
            </Button>
            {token && (
              <>
                <Label htmlFor="deployment-token">Copy this token now</Label>
                <Input
                  id="deployment-token"
                  readOnly
                  value={token}
                  autoComplete="off"
                  spellCheck={false}
                  onFocus={(event) => event.currentTarget.select()}
                />
                <pre className="overflow-auto rounded-md bg-muted p-3 text-xs">{`ENGINE_SIDECAR_GATEWAY_URL=https://gateway.example\nENGINE_SIDECAR_AGENT_TOKENS=<deployment-token>\nENGINE_SIDECAR_AGENT_ENDPOINTS=${agentId}=http://127.0.0.1:3000\ntilde-sidecar`}</pre>
              </>
            )}
          </div>
          <div className="space-y-3">
            <h3 className="font-medium">Registered nodes</h3>
            <Button
              variant="outline"
              onClick={() => {
                void deployments
                  .getDeployment({ agentId })
                  .then((r) => setNodes(r.nodes))
                  .catch((e) =>
                    setError(e instanceof Error ? e.message : "Unable to refresh nodes."),
                  );
              }}
            >
              Refresh nodes
            </Button>
            {nodes.length === 0 ? (
              <p className="text-sm text-muted-foreground">No sidecars registered yet.</p>
            ) : (
              <ul className="space-y-2">
                {nodes.map((node) => (
                  <li key={node.instanceId} className="rounded-md border p-3">
                    <p className="font-mono text-xs">{node.instanceId}</p>
                    <p className="text-sm">{node.ready ? "Ready" : "Unavailable"}</p>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </section>
  );
}
