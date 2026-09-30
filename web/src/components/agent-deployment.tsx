import { useCallback, useEffect, useRef, useState } from "react";
import { useForm } from "@trytilde/connection-ui";
import { deployments } from "@/client";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from "@/components/ui/alert-dialog";
import { InlineSaving, type InlineSavingState } from "./inline-saving";
import { DeploymentContents } from "./deployment-contents";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { date } from "@/features/tracing/format";
import {
  DeploymentRouting,
  DeploymentSource,
  DeploymentStatus,
  DeploymentTarget,
  SidecarFailureMode,
  type AgentDeployment as RegisteredDeployment,
  type AgentInstance,
  type Deployment,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { EllipsisIcon, GitBranchIcon, PlusIcon } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TildeLoader } from "@/components/loading-screen";

type SettingsFields = {
  failureMode: SidecarFailureMode;
  routing: DeploymentRouting;
};
type Loaded = {
  deployment: Deployment;
  deployments: RegisteredDeployment[];
  instances: AgentInstance[];
};
type IssuedToken = { deployment: RegisteredDeployment; token: string };

const targetLabels: Record<DeploymentTarget, string> = {
  [DeploymentTarget.UNSPECIFIED]: "Unknown",
  [DeploymentTarget.GATEWAY]: "Gateway",
  [DeploymentTarget.SIDECAR]: "Sidecar",
  [DeploymentTarget.LAMBDA]: "Lambda",
};

function shortCommit(sha: string, rows: RegisteredDeployment[] = []) {
  let length = 7;
  while (
    length < sha.length &&
    rows.some((row) => row.commitSha !== sha && row.commitSha.startsWith(sha.slice(0, length)))
  )
    length++;
  return sha.slice(0, length);
}
function shortId(id: string) {
  return id.split("-")[0] ?? id;
}
/** First line of the commit message, as a git provider would title it. */
function commitTitle(deployment: RegisteredDeployment) {
  return deployment.commitMessage.split("\n")[0]?.trim() ?? "";
}
/** The row's headline: the commit message from the git provider, else the label, else the id. */
export function describeDeployment(deployment: RegisteredDeployment) {
  return (
    commitTitle(deployment) ||
    deployment.label ||
    shortCommit(deployment.commitSha) ||
    shortId(deployment.id)
  );
}
/**
 * The deployments that can receive the agent's invocations, with what each shipped: the routing
 * target under Latest and every deployment with traffic weight under Weighted; with none
 * serving, invocations fall back to the newest registered deployment. `detail` is its short
 * commit and, when several share traffic, its weight.
 */
export async function servingContents(agentId: string, signal?: AbortSignal) {
  const response = await deployments.getDeployment({ agentId }, { signal });
  const registered = response.deployments
    .filter((d) => d.status === DeploymentStatus.REGISTERED)
    .sort((a, b) => Number((b.createdAt?.seconds ?? 0n) - (a.createdAt?.seconds ?? 0n)));
  const serving = registered.filter((d) => d.serving);
  const targets = serving.length ? serving : registered.slice(0, 1);
  const weighted = response.deployment?.routing === DeploymentRouting.WEIGHTED;
  const contents = await Promise.all(
    targets.map((target) =>
      deployments.getDeploymentContents({ agentId, deploymentId: target.id }, { signal }),
    ),
  );
  return targets.map((deployment, index) => ({
    deployment,
    name: describeDeployment(deployment),
    detail: [
      deployment.commitSha.slice(0, 7),
      weighted && targets.length > 1 ? `${deployment.trafficWeight}% of traffic` : "",
    ]
      .filter(Boolean)
      .join(" · "),
    contents: contents[index]!,
  }));
}
/** The commit page on the git provider. Accepts `owner/repo` (GitHub) or a full repository URL. */
export function commitUrl(repository: string, commitSha: string) {
  const repo = repository
    .trim()
    .replace(/\.git$/, "")
    .replace(/\/+$/, "");
  if (!repo || !commitSha) return undefined;
  if (/^https?:\/\//.test(repo)) return `${repo}/commit/${commitSha}`;
  if (/^[\w.-]+\/[\w.-]+$/.test(repo)) return `https://github.com/${repo}/commit/${commitSha}`;
  return undefined;
}
function toDate(timestamp: { seconds: bigint } | undefined) {
  return timestamp ? new Date(Number(timestamp.seconds) * 1000) : undefined;
}
function formatDate(timestamp: { seconds: bigint } | undefined) {
  const value = toDate(timestamp);
  return value ? date(value.toISOString()) : "";
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

function activeDeployment(deployment: RegisteredDeployment) {
  return deployment.status === DeploymentStatus.REGISTERED && deployment.routable;
}
function weightFields(rows: RegisteredDeployment[]) {
  return {
    weights: Object.fromEntries(rows.map((row) => [row.id, String(row.trafficWeight ?? 0)])),
  };
}
type WeightChange = { deploymentId: string; weight: number };
type WeightDialog =
  | { kind: "confirm"; weights: WeightChange[] }
  | { kind: "error"; message: string };

export function AgentDeployment({ agentId }: { agentId: string }) {
  const settings = useForm<SettingsFields>({
    defaultValues: {
      failureMode: SidecarFailureMode.REASSIGN,
      routing: DeploymentRouting.LATEST,
    },
  });
  const traffic = useForm<{ weights: Record<string, string> }>({ defaultValues: { weights: {} } });
  const [dialog, setDialog] = useState<WeightDialog | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  function showDialog(next: WeightDialog) {
    setDialog(next);
    setDialogOpen(true);
  }
  const [loaded, setLoaded] = useState<Loaded>();
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [issued, setIssued] = useState<IssuedToken>();
  const [busy, setBusy] = useState("");
  const registration = useForm({
    defaultValues: { label: "", target: DeploymentTarget.GATEWAY, targetReference: "" },
  });
  const [registerOpen, setRegisterOpen] = useState(false);
  const [registerError, setRegisterError] = useState("");
  const [confirmation, setConfirmation] = useState<{
    kind: "rotate" | "retire";
    deployment: RegisteredDeployment;
  } | null>(null);
  const [confirmationError, setConfirmationError] = useState("");
  const [contentsOf, setContentsOf] = useState<RegisteredDeployment>();
  const weighted = settings.watch("routing") === DeploymentRouting.WEIGHTED;
  const saving = useRef<AbortController | null>(null);
  const [feedback, setFeedback] = useState<{
    field?: keyof SettingsFields;
    state: InlineSavingState;
    attempt: number;
    error?: string;
  }>({ state: "idle", attempt: 0 });
  const locked = feedback.state === "saving" || busy !== "";

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
      traffic.reset(weightFields(response.deployments ?? []));
      settings.reset({
        failureMode: response.deployment.failureMode || SidecarFailureMode.REASSIGN,
        routing: response.deployment.routing || DeploymentRouting.LATEST,
      });
      return response.deployment;
    },
    [agentId, settings.reset, traffic.reset],
  );

  useEffect(() => {
    const abort = new AbortController();
    setLoaded(undefined);
    setIssued(undefined);
    setRegisterOpen(false);
    setConfirmation(null);
    setDialog(null);
    setDialogOpen(false);
    setNotice("");
    setError("");
    setFeedback({ state: "idle", attempt: 0 });
    void load(abort.signal).catch((error) => {
      if (!abort.signal.aborted) setError(message(error, "Unable to load deployment."));
    });
    return () => {
      abort.abort();
      saving.current?.abort();
      saving.current = null;
    };
  }, [load]);

  useEffect(() => {
    if (locked || !loaded) return;
    const abort = new AbortController();
    let pending = false;
    const timer = setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const response = await deployments.getDeployment({ agentId }, { signal: abort.signal });
        if (abort.signal.aborted || !response.deployment) return;
        setLoaded({
          deployment: response.deployment,
          deployments: [...response.deployments].sort(byNewest),
          instances: response.instances,
        });
        if (!traffic.formState.isDirty) {
          traffic.reset(weightFields(response.deployments));
          settings.reset({
            failureMode: response.deployment.failureMode || SidecarFailureMode.REASSIGN,
            routing: response.deployment.routing || DeploymentRouting.LATEST,
          });
        }
      } catch {
        /* Keep the last snapshot; weight saves revalidate on the server. */
      } finally {
        pending = false;
      }
    }, 3000);
    return () => {
      abort.abort();
      clearInterval(timer);
    };
  }, [agentId, locked, !!loaded, traffic.formState.isDirty, traffic.reset, settings.reset]);

  async function save(field: keyof SettingsFields, value: SettingsFields[keyof SettingsFields]) {
    const previous = settings.getValues();
    if (saving.current || busy || value === previous[field]) return;
    const abort = new AbortController();
    saving.current = abort;
    settings.setValue(field, value, { shouldDirty: true });
    setError("");
    setNotice("");
    setFeedback((current) => ({ field, state: "saving", attempt: current.attempt + 1 }));
    try {
      const { deployment } = await deployments.setDeployment(
        { agentId, ...previous, [field]: value },
        { signal: abort.signal },
      );
      if (abort.signal.aborted) return;
      if (!deployment) throw new Error("Deployment was not returned.");
      settings.reset({
        failureMode: deployment.failureMode || SidecarFailureMode.REASSIGN,
        routing: deployment.routing || DeploymentRouting.LATEST,
      });
      if (loaded) {
        const reseed = previous.routing !== deployment.routing;
        const rows = loaded.deployments.map((item) => ({
          ...item,
          trafficWeight: reseed
            ? item.id === deployment.servingDeploymentId
              ? 100
              : 0
            : item.trafficWeight,
          serving:
            item.routable &&
            (deployment.routing === DeploymentRouting.WEIGHTED && !reseed
              ? item.trafficWeight > 0
              : item.id === deployment.servingDeploymentId),
        }));
        setLoaded({ ...loaded, deployment, deployments: rows });
        if (reseed) traffic.reset(weightFields(rows));
      }
      setFeedback((current) => ({ ...current, state: "success" }));
    } catch (error) {
      if (abort.signal.aborted) return;
      settings.reset(previous);
      const text = message(error, "Unable to save deployment settings.");
      setFeedback((current) => ({ ...current, state: "error", error: text }));
      setError(text);
    } finally {
      if (saving.current === abort) saving.current = null;
    }
  }

  function reviewWeights() {
    if (!loaded || locked) return;
    const rows = loaded.deployments.filter((row) => row.status === DeploymentStatus.REGISTERED);
    const values = traffic.getValues("weights");
    const weights: WeightChange[] = [];
    for (const row of rows) {
      if (!row.routable) {
        weights.push({ deploymentId: row.id, weight: 0 });
        continue;
      }
      const value = (values[row.id] ?? "").trim();
      if (!/^\d+$/.test(value) || Number(value) > 100) {
        showDialog({
          kind: "error",
          message: `Enter a whole percentage from 0 to 100 for ${describeDeployment(row)}.`,
        });
        return;
      }
      weights.push({ deploymentId: row.id, weight: Number(value) });
    }
    const total = weights.reduce((sum, row) => sum + row.weight, 0);
    if (total !== 100) {
      showDialog({
        kind: "error",
        message: `Traffic weights total ${total}%. They must add up to 100%.`,
      });
      return;
    }
    showDialog({ kind: "confirm", weights });
  }

  async function confirmWeights() {
    if (dialog?.kind !== "confirm" || locked) return;
    const abort = new AbortController();
    saving.current = abort;
    setBusy("weights");
    setError("");
    setNotice("");
    try {
      const response = await deployments.setDeploymentWeights(
        { agentId, weights: dialog.weights },
        { signal: abort.signal },
      );
      if (abort.signal.aborted) return;
      if (!response.deployment) throw new Error("Deployment was not returned.");
      setLoaded((current) =>
        current
          ? {
              ...current,
              deployment: response.deployment!,
              deployments: [...response.deployments].sort(byNewest),
            }
          : current,
      );
      traffic.reset(weightFields(response.deployments));
      setDialogOpen(false);
    } catch (error) {
      if (!abort.signal.aborted)
        showDialog({ kind: "error", message: message(error, "Unable to save traffic weights.") });
    } finally {
      if (saving.current === abort) {
        saving.current = null;
        setBusy("");
      }
    }
  }

  function saveFeedback(field: keyof SettingsFields, label: string) {
    return (
      <InlineSaving
        state={feedback.field === field ? feedback.state : "idle"}
        label={label}
        error={feedback.error}
        resetKey={feedback.attempt}
      />
    );
  }

  async function act(
    key: string,
    run: () => Promise<void>,
    successNotice: string,
    fallback: string,
  ) {
    if (saving.current || busy) return;
    setBusy(key);
    setError("");
    setNotice("");
    try {
      await run();
      await load().catch((error) => setError(message(error, "Unable to refresh deployments.")));
      setNotice(successNotice);
      return true;
    } catch (error) {
      setError(message(error, fallback));
      setConfirmationError(message(error, fallback));
      return false;
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
      `${describeDeployment(deployment)} is now serving.`,
      "Unable to promote deployment.",
    );
  }
  function retire(deployment: RegisteredDeployment) {
    return act(
      `retire:${deployment.id}`,
      async () => {
        await deployments.retireDeployment({ agentId, deploymentId: deployment.id });
      },
      `${describeDeployment(deployment)} retired.`,
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
      `Token rotated for ${describeDeployment(deployment)}.`,
      "Unable to issue deployment token.",
    );
  }

  async function register(values: {
    label: string;
    target: DeploymentTarget;
    targetReference: string;
  }) {
    if (locked) return;
    setBusy("register");
    setRegisterError("");
    setError("");
    setNotice("");
    try {
      const targetReference = values.targetReference.trim();
      const response = await deployments.registerDeployment({
        agentId,
        source: DeploymentSource.MANUAL,
        target: values.target,
        label: values.label.trim(),
        ...(values.target === DeploymentTarget.LAMBDA ? { targetReference } : {}),
      });
      if (!response.deployment) throw new Error("Deployment was not returned.");
      setRegisterOpen(false);
      registration.reset();
      setIssued(
        response.token ? { deployment: response.deployment, token: response.token } : undefined,
      );
      setNotice(`${describeDeployment(response.deployment)} registered.`);
      // Registration succeeded even if refreshing the table fails; don't offer a duplicate submission.
      await load().catch((error) => setError(message(error, "Unable to refresh deployments.")));
    } catch (error) {
      setRegisterError(message(error, "Unable to register deployment."));
    } finally {
      setBusy("");
    }
  }

  async function confirmAction() {
    if (!confirmation || locked) return;
    setConfirmationError("");
    const success = await (confirmation.kind === "rotate" ? rotate : retire)(
      confirmation.deployment,
    );
    if (success) setConfirmation(null);
  }

  return (
    <section className="flex min-h-0 w-full flex-1 flex-col">
      {error && (
        <p role="alert" className="px-4 py-2 text-sm text-destructive">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="px-4 py-2 text-sm">
          {notice}
        </p>
      )}
      {loaded === undefined ? (
        <TildeLoader />
      ) : (
        <>
          <section aria-label="Deployment settings" className="shrink-0 border-b px-4 py-5 lg:px-6">
            <div className="flex flex-wrap items-start gap-x-6 gap-y-4">
              <div className="max-w-64 space-y-2">
                <div className="flex items-center gap-2">
                  <Label>Routing</Label>
                  {saveFeedback("routing", "Routing")}
                </div>
                <Tabs
                  value={settings.watch("routing")}
                  onValueChange={(value) =>
                    void save("routing", Number(value) as DeploymentRouting)
                  }
                >
                  <TabsList aria-label="Routing">
                    <TabsTrigger value={DeploymentRouting.LATEST} disabled={locked}>
                      Latest
                    </TabsTrigger>
                    <TabsTrigger value={DeploymentRouting.WEIGHTED} disabled={locked}>
                      Weighted
                    </TabsTrigger>
                  </TabsList>
                </Tabs>
                <p className="text-[10px] text-muted-foreground">
                  {settings.watch("routing") === DeploymentRouting.WEIGHTED
                    ? "New conversations follow the configured traffic weights."
                    : "The newest available deployment serves automatically."}
                </p>
              </div>
              {loaded.deployments.some(
                (item) => item.target === DeploymentTarget.SIDECAR && activeDeployment(item),
              ) && (
                <div className="max-w-64 space-y-2">
                  <div className="flex items-center gap-2">
                    <Label>When an owner fails</Label>
                    {saveFeedback("failureMode", "Failure mode")}
                  </div>
                  <Tabs
                    value={settings.watch("failureMode")}
                    onValueChange={(value) =>
                      void save("failureMode", Number(value) as SidecarFailureMode)
                    }
                  >
                    <TabsList aria-label="Failure mode">
                      <TabsTrigger value={SidecarFailureMode.REASSIGN} disabled={locked}>
                        Assign to new node
                      </TabsTrigger>
                      <TabsTrigger value={SidecarFailureMode.STOP} disabled={locked}>
                        Stop
                      </TabsTrigger>
                    </TabsList>
                  </Tabs>
                </div>
              )}
            </div>
          </section>

          <section aria-label="Deployments" className="flex min-h-0 flex-1 flex-col">
            <div
              role="toolbar"
              aria-label="Deployment actions"
              className="tracing-table flex shrink-0 justify-end border-b px-2 py-1.5"
            >
              <Button
                size="sm"
                className="rounded-none text-[11px]"
                disabled={locked}
                onClick={() => {
                  registration.reset();
                  setRegisterError("");
                  setRegisterOpen(true);
                }}
              >
                <PlusIcon /> Register deployment
              </Button>
            </div>
            <div className="min-h-0 flex-1 overflow-auto">
              <table
                aria-label="Deployments"
                className="tracing-table w-full min-w-[760px] table-fixed border-collapse text-[11px] [&_th]:h-8 [&_th]:border-r [&_th]:border-border/60 [&_th]:px-2 [&_th]:py-0 [&_th]:text-[11px] [&_th]:font-semibold [&_th:last-child]:border-r-0 [&_td]:h-7 [&_td]:border-r [&_td]:border-border/60 [&_td]:px-2 [&_td]:py-0 [&_td]:whitespace-nowrap [&_td]:text-[11px] [&_td:last-child]:border-r-0"
              >
                <colgroup>
                  <col />
                  {weighted && (
                    <col
                      style={{
                        width: traffic.formState.isDirty
                          ? "calc(14ch + 66px)"
                          : "calc(14ch + 16px)",
                      }}
                    />
                  )}
                  <col style={{ width: 80 }} />
                  <col style={{ width: 116 }} />
                  <col
                    style={{
                      width: `calc(${Math.max(7, ...loaded.deployments.map((row) => shortCommit(row.commitSha, loaded.deployments).length))}ch + 16px)`,
                    }}
                  />
                  <col style={{ width: 120 }} />
                  <col style={{ width: "calc(17ch + 16px)" }} />
                  <col style={{ width: 40 }} />
                </colgroup>
                <TableHeader className="sticky top-0 z-10 bg-muted shadow-[inset_0_-1px_0_var(--border)]">
                  <TableRow>
                    <TableHead>Deployment</TableHead>
                    {weighted && (
                      <TableHead>
                        <div className="flex items-center justify-between gap-2">
                          <span>Traffic weight</span>
                          {traffic.formState.isDirty && (
                            <Button
                              type="button"
                              size="xs"
                              variant="outline"
                              className="h-6 rounded-none px-2 text-[10px]"
                              disabled={locked}
                              onClick={reviewWeights}
                            >
                              Save
                            </Button>
                          )}
                        </div>
                      </TableHead>
                    )}
                    <TableHead>Type</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Commit</TableHead>
                    <TableHead>Branch</TableHead>
                    <TableHead className="text-right">Created</TableHead>
                    <TableHead className="w-10 px-2">
                      <span className="sr-only">Actions</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {loaded.deployments.map((deployment) => {
                    const registered = deployment.status === DeploymentStatus.REGISTERED;
                    const instances = loaded.instances.filter(
                      (instance) => instance.deploymentId === deployment.id,
                    );
                    const ready = instances.filter((instance) => instance.ready).length;
                    const name = describeDeployment(deployment);
                    const hash = shortCommit(deployment.commitSha, loaded.deployments);
                    const link = commitUrl(deployment.repository, deployment.commitSha);
                    const status = !registered
                      ? "Retired"
                      : !deployment.routable
                        ? "Offline"
                        : deployment.serving
                          ? "Serving"
                          : "Standby";
                    return (
                      <TableRow
                        key={deployment.id}
                        aria-label={`Deployment ${name}`}
                        className="h-7 border-b border-border/50 hover:bg-muted/60"
                      >
                        <TableCell className="overflow-hidden">
                          <button
                            type="button"
                            className="block max-w-full cursor-pointer truncate hover:underline"
                            title={`${name} · ${deployment.source === DeploymentSource.CI ? "CI" : "Manual"} · view prompts and skills`}
                            onClick={() => setContentsOf(deployment)}
                          >
                            {name}
                          </button>
                        </TableCell>
                        {weighted && (
                          <TableCell>
                            {activeDeployment(deployment) ? (
                              <div className="flex items-center gap-1">
                                <Input
                                  type="number"
                                  min={0}
                                  max={100}
                                  step={1}
                                  defaultValue={String(deployment.trafficWeight)}
                                  aria-label={`Traffic weight for ${name}`}
                                  disabled={locked}
                                  className="h-6 w-12 rounded-none px-1 text-right text-[11px] md:text-[11px]"
                                  {...traffic.register(`weights.${deployment.id}`)}
                                />
                                <span className="text-muted-foreground">%</span>
                              </div>
                            ) : (
                              <span
                                className="text-muted-foreground"
                                title={
                                  registered ? "No live, ready connection" : "Retired deployment"
                                }
                              >
                                —
                              </span>
                            )}
                          </TableCell>
                        )}
                        <TableCell>{targetLabels[deployment.target]}</TableCell>
                        <TableCell className="overflow-hidden">
                          <span
                            className="flex items-center gap-1.5"
                            title={
                              deployment.retiredAt
                                ? `Retired ${formatDate(deployment.retiredAt)}`
                                : undefined
                            }
                          >
                            <span
                              aria-hidden
                              className={`size-1.5 shrink-0 rounded-full ${deployment.serving && deployment.routable ? "bg-emerald-500" : deployment.routable ? "bg-sky-500" : "bg-muted-foreground/40"}`}
                            />
                            <span>
                              {status}
                              {registered && deployment.target !== DeploymentTarget.LAMBDA
                                ? ` (${ready})`
                                : ""}
                            </span>
                          </span>
                        </TableCell>
                        <TableCell className="overflow-hidden">
                          {hash ? (
                            link ? (
                              <a
                                href={link}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="block truncate hover:underline"
                                title={deployment.commitSha}
                              >
                                {hash}
                              </a>
                            ) : (
                              <span title={deployment.commitSha}>{hash}</span>
                            )
                          ) : (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </TableCell>
                        <TableCell className="overflow-hidden">
                          {deployment.branch ? (
                            <span className="flex items-center gap-1" title={deployment.branch}>
                              <GitBranchIcon className="size-3 shrink-0 text-muted-foreground" />
                              <span className="truncate">{deployment.branch}</span>
                            </span>
                          ) : (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </TableCell>
                        <TableCell
                          className="text-right text-muted-foreground"
                          title={[formatDate(deployment.createdAt), deployment.commitAuthor]
                            .filter(Boolean)
                            .join(" · ")}
                        >
                          {formatDate(deployment.createdAt)}
                        </TableCell>
                        <TableCell className="text-right">
                          {registered && (
                            <DropdownMenu>
                              <DropdownMenuTrigger
                                disabled={locked}
                                render={
                                  <Button
                                    variant="ghost"
                                    size="icon-xs"
                                    aria-label={`Actions for ${name}`}
                                  />
                                }
                              >
                                <EllipsisIcon />
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align="end">
                                {!weighted && !deployment.serving && (
                                  <DropdownMenuItem
                                    disabled={!deployment.routable}
                                    onClick={() => void promote(deployment)}
                                  >
                                    Promote
                                  </DropdownMenuItem>
                                )}
                                <DropdownMenuItem
                                  onClick={() => {
                                    setConfirmationError("");
                                    setConfirmation({ kind: "rotate", deployment });
                                  }}
                                >
                                  Rotate token
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => {
                                    setConfirmationError("");
                                    setConfirmation({ kind: "retire", deployment });
                                  }}
                                >
                                  Retire
                                </DropdownMenuItem>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          )}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                  {loaded.deployments.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={weighted ? 8 : 7} className="text-muted-foreground">
                        No deployments registered yet.
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </table>
            </div>
          </section>
        </>
      )}
      <Sheet
        open={!!contentsOf}
        onOpenChange={(open) => {
          if (!open) setContentsOf(undefined);
        }}
      >
        <SheetContent className="w-full overflow-y-auto sm:max-w-md">
          <SheetHeader>
            <SheetTitle>{contentsOf ? describeDeployment(contentsOf) : ""}</SheetTitle>
            <SheetDescription>
              Prompts and skills this deployment shipped, registered by <code>tilde deploy</code>{" "}
              from its code.
              {contentsOf?.commitSha && ` Commit ${contentsOf.commitSha.slice(0, 7)}.`}
            </SheetDescription>
          </SheetHeader>
          <div className="px-4 pb-4">
            {contentsOf && <DeploymentContents agentId={agentId} deploymentId={contentsOf.id} />}
          </div>
        </SheetContent>
      </Sheet>
      <Dialog
        open={!!issued}
        onOpenChange={(open) => {
          if (!open) setIssued(undefined);
        }}
      >
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Deployment token</DialogTitle>
            <DialogDescription>
              Copy this token now. It won't be shown again after you close this dialog.
            </DialogDescription>
          </DialogHeader>
          {issued && (
            <div className="min-w-0 space-y-2">
              <Label htmlFor="deployment-token">
                Copy this token now for {describeDeployment(issued.deployment)}
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
          <DialogFooter>
            <Button variant="outline" onClick={() => setIssued(undefined)}>
              Done
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog
        open={registerOpen}
        onOpenChange={(open) => {
          if (busy !== "register") setRegisterOpen(open);
        }}
      >
        <DialogContent showCloseButton={busy !== "register"}>
          <DialogHeader>
            <DialogTitle>Register deployment</DialogTitle>
            <DialogDescription>
              {registration.watch("target") === DeploymentTarget.LAMBDA
                ? "Lambda deployments can receive traffic immediately under Latest routing."
                : "This deployment stays offline until a ready agent connects with its token."}
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={registration.handleSubmit(register)} className="space-y-3">
            <fieldset disabled={busy === "register"} className="space-y-3">
              <div className="space-y-1">
                <Label htmlFor="deployment-label">Name</Label>
                <Input
                  id="deployment-label"
                  className="h-7 text-xs"
                  {...registration.register("label")}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="deployment-type">Type</Label>
                <Select
                  value={registration.watch("target")}
                  onValueChange={(value) => {
                    registration.setValue("target", Number(value) as DeploymentTarget);
                    registration.setValue("targetReference", "");
                  }}
                >
                  <SelectTrigger id="deployment-type" className="h-7 w-full rounded-none text-xs">
                    <SelectValue>{targetLabels[registration.watch("target")]}</SelectValue>
                  </SelectTrigger>
                  <SelectContent className="rounded-none">
                    {[
                      DeploymentTarget.GATEWAY,
                      DeploymentTarget.SIDECAR,
                      DeploymentTarget.LAMBDA,
                    ].map((target) => (
                      <SelectItem key={target} value={target}>
                        {targetLabels[target]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              {registration.watch("target") === DeploymentTarget.LAMBDA && (
                <div className="space-y-1">
                  <Label htmlFor="deployment-reference">Function ARN</Label>
                  <Input
                    id="deployment-reference"
                    className="h-7 text-xs"
                    required
                    {...registration.register("targetReference")}
                  />
                </div>
              )}
            </fieldset>
            {registerError && (
              <p role="alert" className="text-xs text-destructive">
                {registerError}
              </p>
            )}
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                disabled={busy === "register"}
                onClick={() => setRegisterOpen(false)}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={busy === "register"}>
                {busy === "register" ? "Registering…" : "Register"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={!!confirmation}
        onOpenChange={(open) => {
          if (!open && !busy) setConfirmation(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmation?.kind === "rotate" ? "Rotate deployment token?" : "Retire deployment?"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirmation?.kind === "rotate"
                ? `Replace the token for ${describeDeployment(confirmation.deployment)}? The old token will stop authenticating. Update your agent or sidecar with the new token so it can reconnect.`
                : confirmation
                  ? `Retire ${describeDeployment(confirmation.deployment)}? It will stop receiving routed work and its token will be invalidated. Existing conversations assigned to it may be interrupted.`
                  : ""}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {confirmationError && (
            <p role="alert" className="text-xs text-destructive">
              {confirmationError}
            </p>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={!!busy}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={locked}
              onClick={() => void confirmAction()}
            >
              {busy
                ? "Working…"
                : confirmation?.kind === "rotate"
                  ? "Rotate token"
                  : "Retire deployment"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog
        open={dialogOpen}
        onOpenChange={(open) => {
          if (busy !== "weights") setDialogOpen(open);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {dialog?.kind === "error"
                ? "Unable to save traffic weights"
                : "Confirm traffic weights"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {dialog?.kind === "error"
                ? dialog.message
                : "Apply this traffic split to new conversations? Existing conversations keep their assigned deployment."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {dialog?.kind === "confirm" && (
            <dl className="tracing-table space-y-2 text-xs">
              {dialog.weights.map((row) => (
                <div key={row.deploymentId} className="flex justify-between gap-4">
                  <dt className="truncate">
                    {describeDeployment(
                      loaded!.deployments.find((item) => item.id === row.deploymentId)!,
                    )}
                  </dt>
                  <dd>{row.weight}%</dd>
                </div>
              ))}
            </dl>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy === "weights"}>
              {dialog?.kind === "error" ? "Close" : "Cancel"}
            </AlertDialogCancel>
            {dialog?.kind === "confirm" && (
              <AlertDialogAction
                disabled={busy === "weights"}
                onClick={() => void confirmWeights()}
              >
                {busy === "weights" ? "Saving…" : "Confirm changes"}
              </AlertDialogAction>
            )}
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
