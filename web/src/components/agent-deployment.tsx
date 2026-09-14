import { useCallback, useEffect, useState } from "react";
import { NativeSelect, useForm } from "@trytilde/connection-ui";
import { deployments } from "@/client";
import {
  DeploymentMode,
  DeploymentRouting,
  DeploymentSource,
  DeploymentStatus,
  DeploymentTarget,
  SidecarFailureMode,
  type AgentDeployment as RegisteredDeployment,
  type AgentInstance,
  type Deployment,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

type SettingsFields = {
  mode: DeploymentMode;
  failureMode: SidecarFailureMode;
  routing: DeploymentRouting;
};
type RegisterFields = {
  target: string;
  endpointUrl: string;
  targetReference: string;
  label: string;
  repository: string;
  commitSha: string;
};
type Loaded = {
  deployment: Deployment;
  deployments: RegisteredDeployment[];
  instances: AgentInstance[];
};
type IssuedToken = { deployment: RegisteredDeployment; token: string };

const targetLabels: Record<DeploymentTarget, string> = {
  [DeploymentTarget.UNSPECIFIED]: "Unknown",
  [DeploymentTarget.DIRECT]: "Direct",
  [DeploymentTarget.SIDECAR]: "Sidecar",
  [DeploymentTarget.AWS_LAMBDA]: "AWS Lambda",
};

function shortCommit(sha: string) {
  return sha.slice(0, 7);
}
function describe(deployment: RegisteredDeployment) {
  return deployment.label || shortCommit(deployment.commitSha) || deployment.id;
}
function formatDate(timestamp: { seconds: bigint } | undefined) {
  if (!timestamp) return "";
  return new Date(Number(timestamp.seconds) * 1000).toLocaleString();
}
function byNewest(a: RegisteredDeployment, b: RegisteredDeployment) {
  return Number((b.createdAt?.seconds ?? 0n) - (a.createdAt?.seconds ?? 0n));
}
function message(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}
function sidecarSnippet(agentId: string, token: string) {
  return `# Sidecar process\nENGINE_SIDECAR_GATEWAY_URL=https://gateway.example\nENGINE_SIDECAR_AGENT_TOKENS=${token}\ntilde-sidecar\n\n# Agent process beside it (SDK)\nconnectAgent({ gatewayUrl: "http://127.0.0.1:8081/agents/${agentId}", deploymentToken: process.env.TILDE_DEPLOYMENT_TOKEN!, run })`;
}

export function AgentDeployment({ agentId, paused }: { agentId: string; paused: boolean }) {
  const settings = useForm<SettingsFields>({
    defaultValues: {
      mode: DeploymentMode.GATEWAY,
      failureMode: SidecarFailureMode.REASSIGN,
      routing: DeploymentRouting.LATEST,
    },
  });
  const register = useForm<RegisterFields>({
    defaultValues: {
      target: String(DeploymentTarget.DIRECT),
      endpointUrl: "",
      targetReference: "",
      label: "",
      repository: "",
      commitSha: "",
    },
  });
  const [loaded, setLoaded] = useState<Loaded>();
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [issued, setIssued] = useState<IssuedToken>();
  const [busy, setBusy] = useState("");
  const mode = settings.watch("mode");
  const loadedMode = loaded?.deployment.mode;
  const registerTarget = Number(register.watch("target")) as DeploymentTarget;

  const load = useCallback(
    async (signal?: AbortSignal) => {
      const response = await deployments.getDeployment({ agentId }, { signal });
      if (signal?.aborted) return;
      if (!response.deployment) throw new Error("Deployment was not returned.");
      setLoaded({
        deployment: response.deployment,
        deployments: [...(response.deployments ?? [])].sort(byNewest),
        instances: response.instances ?? [],
      });
      return response.deployment;
    },
    [agentId],
  );

  useEffect(() => {
    const abort = new AbortController();
    setLoaded(undefined);
    setIssued(undefined);
    setNotice("");
    setError("");
    load(abort.signal)
      .then((deployment) => {
        if (!deployment || abort.signal.aborted) return;
        settings.reset({
          mode: deployment.mode,
          failureMode: deployment.failureMode || SidecarFailureMode.REASSIGN,
          routing: deployment.routing || DeploymentRouting.LATEST,
        });
        register.reset({
          target: String(
            deployment.mode === DeploymentMode.SIDECAR
              ? DeploymentTarget.SIDECAR
              : DeploymentTarget.DIRECT,
          ),
          endpointUrl: "",
          targetReference: "",
          label: "",
          repository: "",
          commitSha: "",
        });
      })
      .catch((error) => {
        if (!abort.signal.aborted) setError(message(error, "Unable to load deployment."));
      });
    return () => abort.abort();
  }, [load, settings.reset, register.reset]);

  async function refresh(successNotice?: string) {
    setError("");
    try {
      await load();
      if (successNotice) setNotice(successNotice);
    } catch (error) {
      setError(message(error, "Unable to refresh deployments."));
    }
  }

  const save = settings.handleSubmit(async (values) => {
    setError("");
    setNotice("");
    try {
      const { deployment } = await deployments.setDeployment({ agentId, ...values });
      if (!deployment) throw new Error("Deployment was not returned.");
      setLoaded((current) => (current ? { ...current, deployment } : current));
      register.setValue(
        "target",
        String(
          deployment.mode === DeploymentMode.SIDECAR
            ? DeploymentTarget.SIDECAR
            : DeploymentTarget.DIRECT,
        ),
      );
      setNotice("Deployment saved.");
    } catch (error) {
      setError(message(error, "Unable to save deployment."));
    }
  });

  const submitRegister = register.handleSubmit(async (values) => {
    setError("");
    setNotice("");
    setIssued(undefined);
    const target = Number(values.target) as DeploymentTarget;
    const optional = (value: string) => value.trim() || undefined;
    try {
      const response = await deployments.registerDeployment({
        agentId,
        source: DeploymentSource.MANUAL,
        target,
        endpointUrl: target === DeploymentTarget.DIRECT ? optional(values.endpointUrl) : undefined,
        targetReference:
          target === DeploymentTarget.AWS_LAMBDA ? optional(values.targetReference) : undefined,
        label: optional(values.label),
        repository: optional(values.repository),
        commitSha: optional(values.commitSha),
      });
      if (!response.deployment) throw new Error("Deployment was not returned.");
      if (response.created && response.token) {
        setIssued({ deployment: response.deployment, token: response.token });
      }
      register.reset({ ...values, endpointUrl: "", targetReference: "", label: "", commitSha: "" });
      await load();
      setNotice(response.created ? "Deployment registered." : "Deployment already registered.");
    } catch (error) {
      setError(message(error, "Unable to register deployment."));
    }
  });

  async function act(
    key: string,
    run: () => Promise<void>,
    successNotice: string,
    fallback: string,
  ) {
    setBusy(key);
    setError("");
    setNotice("");
    try {
      await run();
      await load();
      setNotice(successNotice);
    } catch (error) {
      setError(message(error, fallback));
    } finally {
      setBusy("");
    }
  }
  function promote(deployment: RegisteredDeployment) {
    return act(
      `promote:${deployment.id}`,
      async () => {
        await deployments.promoteDeployment({ agentId, deploymentId: deployment.id });
      },
      `${describe(deployment)} is now serving.`,
      "Unable to promote deployment.",
    );
  }
  function retire(deployment: RegisteredDeployment) {
    return act(
      `retire:${deployment.id}`,
      async () => {
        await deployments.retireDeployment({ agentId, deploymentId: deployment.id });
      },
      `${describe(deployment)} retired.`,
      "Unable to retire deployment.",
    );
  }
  function rotate(deployment: RegisteredDeployment) {
    return act(
      `rotate:${deployment.id}`,
      async () => {
        const response = await deployments.issueDeploymentToken({
          agentId,
          deploymentId: deployment.id,
        });
        setIssued({ deployment, token: response.token });
      },
      `Token rotated for ${describe(deployment)}.`,
      "Unable to issue deployment token.",
    );
  }

  const serving = loaded?.deployments.find(
    (deployment) => deployment.serving || deployment.id === loaded.deployment.servingDeploymentId,
  );

  return (
    <section className="max-w-2xl space-y-6 py-6">
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      {loaded === undefined ? (
        <p role="status">Loading deployment…</p>
      ) : (
        <>
          <form onSubmit={save} className="space-y-5">
            <div className="space-y-2">
              <Label>Deployment mode</Label>
              <Tabs
                value={mode}
                onValueChange={(value) =>
                  settings.setValue("mode", Number(value) as DeploymentMode, {
                    shouldDirty: true,
                  })
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
            {mode === DeploymentMode.SIDECAR && (
              <div className="space-y-2">
                <Label>When an owner fails</Label>
                <Tabs
                  value={settings.watch("failureMode")}
                  onValueChange={(value) =>
                    settings.setValue("failureMode", Number(value) as SidecarFailureMode, {
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
            )}
            <div className="space-y-2">
              <Label>Routing</Label>
              <Tabs
                value={settings.watch("routing")}
                onValueChange={(value) =>
                  settings.setValue("routing", Number(value) as DeploymentRouting, {
                    shouldDirty: true,
                  })
                }
              >
                <TabsList aria-label="Routing">
                  <TabsTrigger value={DeploymentRouting.LATEST}>Latest</TabsTrigger>
                  <TabsTrigger value={DeploymentRouting.MANUAL}>Manual</TabsTrigger>
                </TabsList>
              </Tabs>
              <p className="text-sm text-muted-foreground">
                {settings.watch("routing") === DeploymentRouting.MANUAL
                  ? "New deployments wait until you promote one."
                  : "The newest registered deployment serves automatically."}
              </p>
            </div>
            <Button type="submit" disabled={settings.formState.isSubmitting}>
              {settings.formState.isSubmitting ? "Saving…" : "Save deployment"}
            </Button>
          </form>

          <div className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <h3 className="font-medium">Deployments</h3>
              <Button variant="outline" size="sm" onClick={() => void refresh()}>
                Refresh
              </Button>
            </div>
            <p className="text-sm" data-testid="serving-summary">
              {serving ? (
                <>
                  Serving <span className="font-medium">{describe(serving)}</span>
                  {serving.commitSha && serving.label && (
                    <span className="text-muted-foreground">
                      {" "}
                      ({shortCommit(serving.commitSha)})
                    </span>
                  )}
                  .
                </>
              ) : (
                <span className="text-muted-foreground">
                  No deployment is serving. Register one below or from CI.
                </span>
              )}
            </p>
            {issued && (
              <div className="space-y-2 rounded-md border p-3">
                <Label htmlFor="deployment-token">
                  Copy this token now for {describe(issued.deployment)}
                </Label>
                <Input
                  id="deployment-token"
                  readOnly
                  value={issued.token}
                  autoComplete="off"
                  spellCheck={false}
                  onFocus={(event) => event.currentTarget.select()}
                />
                {issued.deployment.target === DeploymentTarget.SIDECAR ? (
                  <pre className="overflow-auto rounded-md bg-muted p-3 text-xs">
                    {sidecarSnippet(agentId, issued.token)}
                  </pre>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    Give this token to the deployed agent; CI can register future deployments with
                    RegisterDeployment.
                  </p>
                )}
              </div>
            )}
            {loaded.deployments.length === 0 ? (
              <p className="text-sm text-muted-foreground">No deployments registered yet.</p>
            ) : (
              <ul className="space-y-2">
                {loaded.deployments.map((deployment) => {
                  const registered = deployment.status === DeploymentStatus.REGISTERED;
                  const isServing =
                    deployment.serving || deployment.id === loaded.deployment.servingDeploymentId;
                  const instances = loaded.instances.filter(
                    (instance) => instance.deploymentId === deployment.id,
                  );
                  const name = describe(deployment);
                  return (
                    <li
                      key={deployment.id}
                      className="space-y-2 rounded-md border p-3"
                      aria-label={`Deployment ${name}`}
                    >
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium">{deployment.label || "Untitled"}</span>
                        {deployment.commitSha && (
                          <span className="font-mono text-xs text-muted-foreground">
                            {shortCommit(deployment.commitSha)}
                            {deployment.repository && ` · ${deployment.repository}`}
                          </span>
                        )}
                        <Badge variant="outline">
                          {deployment.source === DeploymentSource.CI ? "CI" : "Manual"}
                        </Badge>
                        <Badge variant="outline">{targetLabels[deployment.target]}</Badge>
                        <Badge variant={registered ? "secondary" : "destructive"}>
                          {registered ? "Registered" : "Retired"}
                        </Badge>
                        {isServing && <Badge>Serving</Badge>}
                      </div>
                      <p className="text-xs text-muted-foreground">
                        Created {formatDate(deployment.createdAt)}
                        {deployment.retiredAt && ` · Retired ${formatDate(deployment.retiredAt)}`}
                        {deployment.target === DeploymentTarget.DIRECT &&
                          deployment.endpointUrl && (
                            <>
                              {" · "}
                              <span className="font-mono">{deployment.endpointUrl}</span>
                            </>
                          )}
                        {deployment.target === DeploymentTarget.AWS_LAMBDA &&
                          deployment.targetReference && (
                            <>
                              {" · "}
                              <span className="font-mono">{deployment.targetReference}</span>
                            </>
                          )}
                      </p>
                      {registered && (
                        <div className="flex flex-wrap gap-2">
                          {!isServing && (
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={busy !== ""}
                              onClick={() => void promote(deployment)}
                              aria-label={`Promote ${name}`}
                            >
                              {busy === `promote:${deployment.id}` ? "Promoting…" : "Promote"}
                            </Button>
                          )}
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={busy !== ""}
                            onClick={() => void rotate(deployment)}
                            aria-label={`Rotate token for ${name}`}
                          >
                            {busy === `rotate:${deployment.id}` ? "Rotating…" : "Rotate token"}
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={busy !== ""}
                            onClick={() => void retire(deployment)}
                            aria-label={`Retire ${name}`}
                          >
                            {busy === `retire:${deployment.id}` ? "Retiring…" : "Retire"}
                          </Button>
                        </div>
                      )}
                      {instances.length === 0 ? (
                        <p className="text-xs text-muted-foreground">No live instances.</p>
                      ) : (
                        <ul className="space-y-1">
                          {instances.map((instance) => (
                            <li
                              key={instance.instanceId}
                              className="flex items-center gap-2 text-xs"
                            >
                              <span className="font-mono">{instance.instanceId}</span>
                              <span>{instance.ready ? "Ready" : "Unavailable"}</span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
          </div>

          <form onSubmit={submitRegister} className="space-y-4">
            <h3 className="font-medium">Register deployment</h3>
            <p className="text-sm text-muted-foreground">
              {loadedMode === DeploymentMode.SIDECAR
                ? "Every replica of this deployment dials in with the token you receive once."
                : "The gateway reaches this deployment directly. The token authenticates the deployed agent."}
            </p>
            <div className="space-y-2">
              <Label htmlFor="register-target">Target</Label>
              <NativeSelect id="register-target" {...register.register("target")}>
                {loadedMode === DeploymentMode.SIDECAR ? (
                  <option value={DeploymentTarget.SIDECAR}>Sidecar</option>
                ) : (
                  <>
                    <option value={DeploymentTarget.DIRECT}>Direct</option>
                    <option value={DeploymentTarget.AWS_LAMBDA}>AWS Lambda</option>
                  </>
                )}
              </NativeSelect>
            </div>
            {loadedMode !== DeploymentMode.SIDECAR &&
              registerTarget === DeploymentTarget.DIRECT && (
                <div className="space-y-2">
                  <Label htmlFor="register-endpoint">Agent endpoint URL</Label>
                  <Input
                    id="register-endpoint"
                    type="url"
                    {...register.register("endpointUrl", { required: true })}
                  />
                </div>
              )}
            {loadedMode !== DeploymentMode.SIDECAR &&
              registerTarget === DeploymentTarget.AWS_LAMBDA && (
                <div className="space-y-2">
                  <Label htmlFor="register-function">Lambda function</Label>
                  <Input
                    id="register-function"
                    placeholder="arn:aws:lambda:…"
                    {...register.register("targetReference", { required: true })}
                  />
                </div>
              )}
            <div className="space-y-2">
              <Label htmlFor="register-label">Label</Label>
              <Input id="register-label" {...register.register("label")} />
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="register-repository">Repository</Label>
                <Input
                  id="register-repository"
                  placeholder="org/repo"
                  {...register.register("repository")}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="register-commit">Commit SHA</Label>
                <Input
                  id="register-commit"
                  spellCheck={false}
                  {...register.register("commitSha")}
                />
              </div>
            </div>
            <Button type="submit" variant="outline" disabled={register.formState.isSubmitting}>
              {register.formState.isSubmitting ? "Registering…" : "Register deployment"}
            </Button>
          </form>
        </>
      )}
    </section>
  );
}
