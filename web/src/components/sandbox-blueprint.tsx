import { useCallback, useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import { timestampDate, type Timestamp } from "@bufbuild/protobuf/wkt";
import { formatDistanceToNow } from "date-fns";
import { PencilIcon, PlusIcon } from "lucide-react";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import {
  SandboxStatus,
  type Sandbox,
  type SandboxBlueprint,
  type SandboxReuse,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { sandboxes } from "@/client";
import { AgentTools } from "./agent-tools";
import { useAgentRegistry } from "./agent-stack";
import { DashboardNavigation, usePageCrumb } from "./dashboard-breadcrumbs";
import { RemoveButton } from "./remove-button";
import {
  BlueprintFields,
  blueprintValues,
  timingsOf,
  ConnectionCell,
  loadSandboxConnections,
  REUSE,
  reuseOf,
  Warning,
  type BlueprintValues,
  type SandboxConnections,
} from "./sandboxes";
import { message } from "./tool-connections";
import { TildeLoader } from "./loading-screen";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export const BLUEPRINT_TABS = ["settings", "environment", "tools", "sandboxes"] as const;
export type BlueprintTab = (typeof BLUEPRINT_TABS)[number];
/** How often the sandboxes table refetches while the page is visible. */
const LIVE_MS = 10_000;

/** One sandbox blueprint: its settings, environment, tools and sandboxes. */
export function SandboxBlueprintPage({
  blueprintId,
  tab,
  onTabChange,
}: {
  blueprintId: string;
  tab: BlueprintTab;
  onTabChange: (tab: BlueprintTab) => void;
}) {
  const [blueprint, setBlueprint] = useState<SandboxBlueprint>();
  const [available, setAvailable] = useState<SandboxConnections>({
    connections: [],
    providers: new Map(),
  });
  const [error, setError] = useState("");
  const registry = useAgentRegistry();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  usePageCrumb(pathname, blueprint?.name);
  useEffect(() => {
    const abort = new AbortController();
    void Promise.all([
      sandboxes.getSandboxBlueprint({ id: blueprintId }, { signal: abort.signal }),
      loadSandboxConnections(abort.signal),
    ])
      .then(([{ blueprint }, found]) => {
        if (!blueprint) throw new Error("Blueprint not found.");
        setBlueprint(blueprint);
        setAvailable(found);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      });
    return () => abort.abort();
  }, [blueprintId]);
  if (!blueprint)
    return error ? (
      <p role="alert" className="p-6 text-sm text-destructive">
        {error}
      </p>
    ) : (
      <TildeLoader className="flex-1" />
    );
  const padded = "mx-auto grid w-full max-w-5xl gap-6 px-4 py-6 lg:px-8";
  return (
    <Tabs
      className="min-h-0 flex-1 gap-0"
      value={tab}
      onValueChange={(value) => onTabChange(value as BlueprintTab)}
    >
      <DashboardNavigation>
        <TabsList aria-label="Blueprint settings">
          <TabsTrigger value="settings">Settings</TabsTrigger>
          <TabsTrigger value="environment">Environment</TabsTrigger>
          <TabsTrigger value="tools">Tools</TabsTrigger>
          <TabsTrigger value="sandboxes">Sandboxes</TabsTrigger>
        </TabsList>
      </DashboardNavigation>
      <TabsContent value="settings" className={padded}>
        <BlueprintSettings
          blueprint={blueprint}
          available={available}
          registry={registry}
          onSaved={setBlueprint}
        />
      </TabsContent>
      <TabsContent value="environment" className={padded}>
        <BlueprintEnvironment blueprint={blueprint} onChanged={setBlueprint} />
      </TabsContent>
      <TabsContent value="tools" className="flex min-h-0 flex-1 flex-col">
        {tab === "tools" && <AgentTools blueprintId={blueprint.id} />}
      </TabsContent>
      <TabsContent value="sandboxes" className={padded}>
        {tab === "sandboxes" && (
          <BlueprintSandboxes blueprintId={blueprint.id} registry={registry} />
        )}
      </TabsContent>
    </Tabs>
  );
}

function BlueprintSettings({
  blueprint,
  available,
  registry,
  onSaved,
}: {
  blueprint: SandboxBlueprint;
  available: SandboxConnections;
  registry: Map<string, Agent>;
  onSaved: (blueprint: SandboxBlueprint) => void;
}) {
  const navigate = useNavigate();
  const form = useForm<BlueprintValues>({ defaultValues: blueprintValues(blueprint) });
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [busy, setBusy] = useState(false);
  const saved = blueprintValues(blueprint);
  const reuseChanged = form.watch("reuse") !== saved.reuse;
  const connectionChanged = form.watch("connectionId") !== saved.connectionId;
  const current = form.watch();
  const timingsChanged = JSON.stringify(timingsOf(current)) !== JSON.stringify(timingsOf(saved));
  const used = blueprint.agentIds;
  async function remove() {
    setBusy(true);
    setError("");
    try {
      await sandboxes.deleteSandboxBlueprint({ id: blueprint.id });
      void navigate({ to: "/sandboxes" });
    } catch (e) {
      setError(message(e));
      setDeleting(false);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <section className="space-y-3">
        <h2 className="text-lg font-semibold">{blueprint.name}</h2>
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <ConnectionCell id={blueprint.connectionId} available={available} />
          <Badge variant="outline">{REUSE[reuseOf(blueprint.reuse)].label}</Badge>
        </div>
      </section>
      <form
        className="grid max-w-2xl gap-4"
        aria-label="Blueprint settings"
        onSubmit={form.handleSubmit(async (values) => {
          setError("");
          // Only the fields that changed are sent.
          const changed = <K extends keyof BlueprintValues>(key: K, value: BlueprintValues[K]) =>
            value !== saved[key] ? value : undefined;
          try {
            const { blueprint: next } = await sandboxes.updateSandboxBlueprint({
              id: blueprint.id,
              name: changed("name", values.name.trim()),
              connectionId: changed("connectionId", values.connectionId),
              template: changed("template", values.template.trim()),
              reuse: reuseChanged ? (Number(values.reuse) as SandboxReuse) : undefined,
              timings: timingsChanged ? timingsOf(values) : undefined,
            });
            if (next) {
              onSaved(next);
              form.reset(blueprintValues(next));
            }
          } catch (e) {
            setError(message(e));
          }
        })}
      >
        <BlueprintFields form={form} available={available} current={blueprint.connectionId} />
        {(reuseChanged || connectionChanged) && (
          <Warning>
            Saving terminates this blueprint's existing sandboxes and their files
            {connectionChanged
              ? ", which run on the current connection"
              : `, which were shared ${REUSE[reuseOf(blueprint.reuse)].label.toLowerCase()}`}
            . New ones launch as agents need them.
          </Warning>
        )}
        <p className="text-xs text-muted-foreground">
          A new template applies to sandboxes launched afterwards.
        </p>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="flex justify-end">
          <Button type="submit" disabled={!form.formState.isDirty || form.formState.isSubmitting}>
            {form.formState.isSubmitting ? "Saving…" : "Save"}
          </Button>
        </div>
      </form>
      <section className="grid max-w-2xl gap-3 border-t pt-6" aria-label="Delete blueprint">
        <h3 className="text-sm font-medium">Delete blueprint</h3>
        {used.length ? (
          <div className="text-xs text-muted-foreground">
            <p>
              {used.length === 1 ? "An agent uses" : `${used.length} agents use`} this blueprint.
              Remove the sandbox from their Capabilities first:
            </p>
            <ul className="mt-2 list-disc pl-5">
              {used.map((id) => (
                <li key={id}>
                  <Link
                    to="/agent/$agentId/capabilities"
                    params={{ agentId: id }}
                    className="text-foreground underline"
                  >
                    {registry.get(id)?.name ?? id}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            Its sandboxes are terminated, and its environment and tools are lost.
          </p>
        )}
        <div>
          <Button
            variant="destructive"
            disabled={!!used.length || busy}
            onClick={() => setDeleting(true)}
          >
            Delete blueprint
          </Button>
        </div>
      </section>
      <AlertDialog open={deleting} onOpenChange={(next) => !busy && setDeleting(next)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {blueprint.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              Its sandboxes are terminated, and its environment and tools are lost.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void remove()}>
              Delete
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

/** Environment variables by name; values are write-only and never shown again. */
function BlueprintEnvironment({
  blueprint,
  onChanged,
}: {
  blueprint: SandboxBlueprint;
  onChanged: (blueprint: SandboxBlueprint) => void;
}) {
  // The variable being set: "" adds a new one.
  const [editing, setEditing] = useState<string>();
  const [error, setError] = useState("");
  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <p className="max-w-2xl text-sm text-muted-foreground">
          Exported in every shell in this blueprint's sandboxes, and refreshed whenever a sandbox
          reconnects. Values are stored encrypted and never shown after saving.
        </p>
        <Button className="gap-2" onClick={() => setEditing("")}>
          <PlusIcon />
          Add variable
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border">
        <Table aria-label="Environment variables">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Value</TableHead>
              <TableHead className="w-24 text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {blueprint.envNames.length ? (
              blueprint.envNames.map((name) => (
                <TableRow key={name}>
                  <TableCell>
                    <code className="text-xs font-medium">{name}</code>
                  </TableCell>
                  <TableCell className="text-muted-foreground">••••••••</TableCell>
                  <TableCell>
                    <span className="flex justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Replace ${name}`}
                        title={`Replace ${name}`}
                        onClick={() => setEditing(name)}
                      >
                        <PencilIcon />
                      </Button>
                      <RemoveButton
                        label={`Delete ${name}`}
                        title={`Delete ${name}?`}
                        description="Shells in sandboxes that reconnect afterwards no longer have it."
                        confirmLabel="Delete"
                        onConfirm={() =>
                          sandboxes
                            .deleteSandboxEnvVar({ blueprintId: blueprint.id, name })
                            .then(({ blueprint }) => blueprint && onChanged(blueprint))
                            .catch((e) => setError(message(e)))
                        }
                      />
                    </span>
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={3} className="h-24 text-center text-muted-foreground">
                  No environment variables.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <EnvVarDialog
        blueprintId={blueprint.id}
        name={editing}
        onClose={() => setEditing(undefined)}
        onSaved={onChanged}
      />
    </>
  );
}

const ENV_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
function EnvVarDialog({
  blueprintId,
  name,
  onClose,
  onSaved,
}: {
  blueprintId: string;
  /** Replacing this variable's value; "" adds a new one; undefined is closed. */
  name?: string;
  onClose: () => void;
  onSaved: (blueprint: SandboxBlueprint) => void;
}) {
  const form = useForm<{ name: string; value: string }>({
    values: { name: name ?? "", value: "" },
  });
  const [error, setError] = useState("");
  function close() {
    setError("");
    form.reset({ name: "", value: "" });
    onClose();
  }
  const replacing = !!name;
  return (
    <Dialog open={name !== undefined} onOpenChange={(open) => !open && close()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{replacing ? `Replace ${name}` : "Add variable"}</DialogTitle>
          <DialogDescription>
            The value is write-only: it is never shown again once saved.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { blueprint } = await sandboxes.setSandboxEnvVar({
                blueprintId,
                name: values.name.trim(),
                value: values.value,
              });
              if (blueprint) onSaved(blueprint);
              close();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <div className="space-y-2">
            <Label htmlFor="env-name">Name</Label>
            <Input
              id="env-name"
              readOnly={replacing}
              placeholder="GITHUB_TOKEN"
              aria-invalid={!!form.formState.errors.name}
              {...form.register("name", {
                validate: (value) =>
                  ENV_NAME.test(value.trim()) ||
                  "Letters, digits and underscores, not starting with a digit.",
              })}
            />
            {form.formState.errors.name && (
              <p role="alert" className="text-xs text-destructive">
                {form.formState.errors.name.message}
              </p>
            )}
          </div>
          <div className="space-y-2">
            <Label htmlFor="env-value">Value</Label>
            <Input id="env-value" type="password" autoComplete="off" {...form.register("value")} />
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={close}
              disabled={form.formState.isSubmitting}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Loads now, then again every LIVE_MS while the page is visible. `load` must be stable. */
function useLive<T>(load: (signal: AbortSignal) => Promise<T>) {
  const [data, setData] = useState<T>();
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    const run = () =>
      load(abort.signal).then(
        (next) => {
          setData(next);
          setError("");
        },
        (e) => {
          if (!abort.signal.aborted) setError(message(e));
        },
      );
    void run();
    const timer = window.setInterval(() => {
      if (!document.hidden) void run();
    }, LIVE_MS);
    return () => {
      abort.abort();
      window.clearInterval(timer);
    };
  }, [load, attempt]);
  return { data, error, refresh: () => setAttempt((n) => n + 1) };
}

const STATUS: Record<
  SandboxStatus,
  { label: string; variant: "default" | "secondary" | "outline" | "destructive" }
> = {
  [SandboxStatus.UNSPECIFIED]: { label: "Unknown", variant: "outline" },
  [SandboxStatus.STARTING]: { label: "Starting", variant: "secondary" },
  [SandboxStatus.RUNNING]: { label: "Running", variant: "default" },
  [SandboxStatus.SLEEPING]: { label: "Sleeping", variant: "outline" },
  [SandboxStatus.FAILED]: { label: "Failed", variant: "destructive" },
};

function Ago({ at }: { at?: Timestamp }) {
  if (!at) return <span className="text-muted-foreground">—</span>;
  const date = timestampDate(at);
  return (
    <time title={date.toLocaleString()}>{formatDistanceToNow(date, { addSuffix: true })}</time>
  );
}

/** The blueprint's VMs, live: what each is keyed by, its state, and a way to terminate it. */
function BlueprintSandboxes({
  blueprintId,
  registry,
}: {
  blueprintId: string;
  registry: Map<string, Agent>;
}) {
  const load = useCallback(
    (signal: AbortSignal) =>
      sandboxes.listSandboxes({ blueprintId }, { signal }).then((r) => r.sandboxes),
    [blueprintId],
  );
  const { data, error, refresh } = useLive(load);
  const [actionError, setActionError] = useState("");
  return (
    <>
      <p className="max-w-2xl text-sm text-muted-foreground">
        Sandboxes launch when an agent needs one and sleep between runs. A terminated sandbox is
        launched again, empty, the next time it is needed.
      </p>
      {(error || actionError) && (
        <p role="alert" className="text-sm text-destructive">
          {actionError || error}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border">
        <Table aria-label="Sandboxes">
          <TableHeader className="bg-muted/50">
            <TableRow>
              <TableHead>Status</TableHead>
              <TableHead>Keyed by</TableHead>
              <TableHead>Provider ID</TableHead>
              <TableHead>Connected</TableHead>
              <TableHead>Last used</TableHead>
              <TableHead>Error</TableHead>
              <TableHead className="w-16 text-right">
                <span className="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data?.length ? (
              data.map((sandbox) => (
                <TableRow key={sandbox.id}>
                  <TableCell>
                    <Badge variant={STATUS[sandbox.status].variant}>
                      {STATUS[sandbox.status].label}
                    </Badge>
                  </TableCell>
                  <TableCell className="whitespace-normal">
                    <KeyedBy sandbox={sandbox} registry={registry} />
                  </TableCell>
                  <TableCell>
                    <code className="text-xs">{sandbox.providerSandboxId ?? "—"}</code>
                  </TableCell>
                  <TableCell>
                    <span
                      role="img"
                      aria-label={sandbox.connected ? "Connected" : "Not connected"}
                      title={sandbox.connected ? "Connected" : "Not connected"}
                      className={`inline-block size-2 rounded-full ${sandbox.connected ? "bg-emerald-500" : "bg-muted-foreground/40"}`}
                    />
                  </TableCell>
                  <TableCell>
                    <Ago at={sandbox.lastUsedAt} />
                  </TableCell>
                  <TableCell className="max-w-64 text-xs whitespace-normal text-destructive">
                    {sandbox.error}
                  </TableCell>
                  <TableCell className="text-right">
                    <RemoveButton
                      label={`Terminate sandbox ${sandbox.providerSandboxId ?? sandbox.id}`}
                      title="Terminate this sandbox?"
                      description="The VM and everything on it are destroyed at the provider. The next run that needs it launches a new one."
                      confirmLabel="Terminate"
                      onConfirm={() =>
                        sandboxes
                          .terminateSandbox({ id: sandbox.id })
                          .then(() => {
                            setActionError("");
                            refresh();
                          })
                          .catch((e) => setActionError(message(e)))
                      }
                    />
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={7} className="h-24 text-center text-muted-foreground">
                  {data
                    ? "No sandboxes running."
                    : error
                      ? "Unable to load sandboxes."
                      : "Loading sandboxes…"}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
    </>
  );
}

/** The agent, thread and person a sandbox's reuse mode keys it by. */
function KeyedBy({ sandbox, registry }: { sandbox: Sandbox; registry: Map<string, Agent> }) {
  const parts = [
    sandbox.agentId && (
      <Link
        key="agent"
        to="/agent/$agentId/capabilities"
        params={{ agentId: sandbox.agentId }}
        className="hover:underline"
      >
        {registry.get(sandbox.agentId)?.name ?? "Agent"}
      </Link>
    ),
    sandbox.threadId && (
      <span key="thread">
        Thread <code className="text-xs">{sandbox.threadId}</code>
      </span>
    ),
    sandbox.identityId && (
      <span key="identity">
        Person <code className="text-xs">{sandbox.identityId}</code>
      </span>
    ),
  ].filter(Boolean);
  return (
    <span className="flex flex-col gap-0.5">
      <span className="flex flex-wrap items-center gap-x-2">
        {parts.length ? parts : "Every agent"}
      </span>
      <span className="text-xs text-muted-foreground">{REUSE[reuseOf(sandbox.reuse)].label}</span>
    </span>
  );
}
