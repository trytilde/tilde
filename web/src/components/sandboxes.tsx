import { useCallback, useEffect, useState } from "react";
import { useForm, type UseFormReturn } from "react-hook-form";
import { Link, useNavigate } from "@tanstack/react-router";
import { PlusIcon, TriangleAlertIcon } from "lucide-react";
import type { Connection, Provider } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import {
  SandboxReuse,
  type SandboxBlueprint,
} from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { NativeSelect } from "@trytilde/connection-ui";
import { connections, sandboxes } from "@/client";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { ProviderIcon } from "./provider-icon";
import { CatalogSearchField } from "./tool-catalog";
import { loadToolConnections, message } from "./tool-connections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

/** The tool providers whose connections launch sandboxes. */
const SANDBOX_PROVIDERS = ["e2b", "modal"] as const;

type Reuse = Exclude<SandboxReuse, SandboxReuse.UNSPECIFIED>;
/** How each reuse mode reads; `warning` names the data that crosses sessions or agents. */
export const REUSE: Record<Reuse, { label: string; description: string; warning?: string }> = {
  [SandboxReuse.THREAD]: {
    label: "Per session",
    description: "Each agent conversation gets its own sandbox.",
  },
  [SandboxReuse.AGENT]: {
    label: "Per agent",
    description: "One sandbox for all of an agent's sessions.",
    warning: "Files from one user's session are visible to every other session of the agent.",
  },
  [SandboxReuse.AGENT_IDENTITY]: {
    label: "Per agent and person",
    description:
      "One sandbox per person per agent, across their sessions and channels. Falls back to per session when the person isn't verified.",
    warning: "A person's files carry across all of their sessions with the agent.",
  },
  [SandboxReuse.GLOBAL]: {
    label: "Shared by all agents",
    description: "Every agent using this blueprint shares one sandbox.",
    warning: "Data crosses agents and sessions; concurrent runs share one filesystem.",
  },
  [SandboxReuse.GLOBAL_IDENTITY]: {
    label: "Shared per person",
    description:
      "One sandbox per person, shared by every agent using this blueprint. Falls back to per session when the person isn't verified.",
    warning: "A person's files are visible to every agent using this blueprint.",
  },
};
// Unspecified reuse means THREAD.
export const reuseOf = (reuse: SandboxReuse): Reuse => reuse || SandboxReuse.THREAD;

/** Every E2B and Modal connection, whatever its status, with its provider. */
export async function loadSandboxConnections(signal?: AbortSignal) {
  const [lists, found] = await Promise.all([
    Promise.all(SANDBOX_PROVIDERS.map((providerId) => loadToolConnections({ providerId }, signal))),
    Promise.all(
      SANDBOX_PROVIDERS.map((id) =>
        connections.getProvider({ id }, { signal }).then(
          ({ provider }) => provider,
          () => undefined,
        ),
      ),
    ),
  ]);
  const providers = new Map<string, Provider>();
  for (const provider of found) if (provider) providers.set(provider.id, provider);
  return { connections: lists.flat(), providers };
}
export type SandboxConnections = Awaited<ReturnType<typeof loadSandboxConnections>>;

/** A blueprint's connection as a table cell: its provider's icon and its name. */
export function ConnectionCell({ id, available }: { id: string; available: SandboxConnections }) {
  const connection = available.connections.find((c) => c.id === id);
  const provider = connection && available.providers.get(connection.providerId);
  return (
    <span className="flex items-center gap-2">
      <ProviderIcon iconUrl={provider?.iconUrl} />
      <span>{connection?.name ?? "Unknown connection"}</span>
      {provider && <span className="text-xs text-muted-foreground">{provider.name}</span>}
    </span>
  );
}

export type BlueprintValues = {
  name: string;
  connectionId: string;
  template: string;
  /** A SandboxReuse, as the select's string value. */
  reuse: string;
  /** Timings in the units the form shows them in. */
  sleepAfterMinutes: number;
  terminateAfterHours: number;
  connectTimeoutSeconds: number;
};
export const blueprintValues = (blueprint?: SandboxBlueprint): BlueprintValues => ({
  name: blueprint?.name ?? "",
  connectionId: blueprint?.connectionId ?? "",
  template: blueprint?.template ?? "",
  reuse: String(reuseOf(blueprint?.reuse ?? SandboxReuse.THREAD)),
  sleepAfterMinutes: (blueprint?.timings?.sleepAfterSeconds ?? 600) / 60,
  terminateAfterHours: (blueprint?.timings?.terminateAfterSeconds ?? 604_800) / 3600,
  connectTimeoutSeconds: blueprint?.timings?.connectTimeoutSeconds ?? 120,
});
/** The form's timings in seconds, as the API takes them. */
export const timingsOf = (values: BlueprintValues) => ({
  sleepAfterSeconds: Math.round(values.sleepAfterMinutes * 60),
  terminateAfterSeconds: Math.round(values.terminateAfterHours * 3600),
  connectTimeoutSeconds: Math.round(values.connectTimeoutSeconds),
});

/**
 * A blueprint's name, connection, template, reuse mode and timings, for creating one or editing
 * it. Only ready connections are offered, besides the one the blueprint already uses.
 */
export function BlueprintFields({
  form,
  available,
  current,
}: {
  form: UseFormReturn<BlueprintValues>;
  available: SandboxConnections;
  current?: string;
}) {
  const { register, watch, formState } = form;
  const offered = available.connections.filter(
    (c: Connection) => c.status === "ready" || c.id === current,
  );
  const chosen = available.connections.find((c) => c.id === watch("connectionId"));
  const reuse = REUSE[Number(watch("reuse")) as Reuse] ?? REUSE[SandboxReuse.THREAD];
  return (
    <>
      <div className="space-y-2">
        <Label htmlFor="blueprint-name">Name</Label>
        <Input
          id="blueprint-name"
          aria-invalid={!!formState.errors.name}
          {...register("name", { validate: (value) => !!value.trim() })}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="blueprint-connection">Connection</Label>
        <NativeSelect
          id="blueprint-connection"
          aria-invalid={!!formState.errors.connectionId}
          {...register("connectionId", { required: true })}
        >
          <option value="">Choose a connection</option>
          {offered.map((connection) => (
            <option key={connection.id} value={connection.id}>
              {`${connection.name} · ${available.providers.get(connection.providerId)?.name ?? connection.providerId}`}
            </option>
          ))}
        </NativeSelect>
        {!offered.length && (
          <p className="text-xs text-muted-foreground">
            No ready E2B or Modal connection. Connect{" "}
            <Link
              to="/tools/catalog/$providerId"
              params={{ providerId: "e2b" }}
              className="underline"
            >
              E2B
            </Link>{" "}
            or{" "}
            <Link
              to="/tools/catalog/$providerId"
              params={{ providerId: "modal" }}
              className="underline"
            >
              Modal
            </Link>{" "}
            first.
          </p>
        )}
      </div>
      <div className="space-y-2">
        <Label htmlFor="blueprint-template">Template</Label>
        <Input
          id="blueprint-template"
          aria-describedby="blueprint-template-help"
          aria-invalid={!!formState.errors.template}
          placeholder={
            chosen?.providerId === "modal" ? "ghcr.io/acme/sandbox:latest" : "my-template"
          }
          {...register("template", { validate: (value) => !!value.trim() })}
        />
        <p id="blueprint-template-help" className="text-xs text-muted-foreground">
          E2B: a template ID or name. Modal: a registry image such as{" "}
          <code>ghcr.io/acme/sandbox:latest</code> or an image ID (<code>im-…</code>). The image
          must have the <code>tilde</code> CLI on its PATH: Tilde starts{" "}
          <code>tilde sandbox connect</code> in it.
        </p>
      </div>
      <div className="space-y-2">
        <Label htmlFor="blueprint-reuse">Reuse</Label>
        <NativeSelect
          id="blueprint-reuse"
          aria-describedby="blueprint-reuse-help"
          {...register("reuse")}
        >
          {Object.entries(REUSE).map(([value, { label }]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </NativeSelect>
        <p id="blueprint-reuse-help" className="text-xs text-muted-foreground">
          {reuse.description}
        </p>
        {reuse.warning && <Warning>{reuse.warning}</Warning>}
      </div>
      <div className="grid gap-4 sm:grid-cols-3">
        <div className="space-y-2">
          <Label htmlFor="blueprint-sleep">Sleep after (minutes)</Label>
          <Input
            id="blueprint-sleep"
            type="number"
            min={1}
            max={1440}
            aria-invalid={!!formState.errors.sleepAfterMinutes}
            {...register("sleepAfterMinutes", { valueAsNumber: true, min: 1, max: 1440 })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="blueprint-terminate">Terminate after (hours)</Label>
          <Input
            id="blueprint-terminate"
            type="number"
            min={1}
            max={720}
            aria-invalid={!!formState.errors.terminateAfterHours}
            {...register("terminateAfterHours", {
              valueAsNumber: true,
              min: 1,
              max: 720,
              validate: (hours, values) => hours * 60 >= values.sleepAfterMinutes,
            })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="blueprint-connect">Connect timeout (seconds)</Label>
          <Input
            id="blueprint-connect"
            type="number"
            min={10}
            max={600}
            aria-invalid={!!formState.errors.connectTimeoutSeconds}
            {...register("connectTimeoutSeconds", { valueAsNumber: true, min: 10, max: 600 })}
          />
        </div>
        <p className="text-xs text-muted-foreground sm:col-span-3">
          A running sandbox sleeps once unused for the first, and any sandbox is terminated once
          unused for the second. Launching or waking one fails the agent's invocation if its{" "}
          <code>tilde</code> process has not connected within the timeout.
        </p>
      </div>
    </>
  );
}

export function Warning({ children }: { children: React.ReactNode }) {
  return (
    <p
      role="note"
      className="flex items-start gap-2 rounded-md bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-400"
    >
      <TriangleAlertIcon aria-hidden="true" className="mt-px size-3.5 shrink-0" />
      <span>{children}</span>
    </p>
  );
}

/** Sandbox blueprints: how agents' sandboxes are launched, configured and shared. */
export function SandboxesPage() {
  const navigate = useNavigate();
  const [blueprints, setBlueprints] = useState<SandboxBlueprint[]>([]);
  const [available, setAvailable] = useState<SandboxConnections>({
    connections: [],
    providers: new Map(),
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [query, setQuery] = useState("");
  const search = useDebouncedValue(query.trim());
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const [list, found] = await Promise.all([
        sandboxes.listSandboxBlueprints({ search: search || undefined }, { signal }),
        loadSandboxConnections(signal),
      ]);
      setBlueprints(list.blueprints);
      setAvailable(found);
      setError("");
    },
    [search],
  );
  useEffect(() => {
    const abort = new AbortController();
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  return (
    <div className="flex flex-col">
      <div
        className="trace-controls flex min-h-10 items-center gap-2 border-b bg-background px-3 py-1.5"
        role="group"
        aria-label="Blueprint filter bar"
      >
        <CatalogSearchField label="Search blueprints" value={query} onChange={setQuery} />
      </div>
      <div className="flex flex-col gap-6 p-4 lg:p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="max-w-2xl text-sm text-muted-foreground">
            A blueprint launches VMs for agents through an E2B or Modal connection. Give an agent a
            sandbox in its Capabilities.
          </p>
          <Button className="gap-2" onClick={() => setCreating(true)}>
            <PlusIcon />
            New blueprint
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <div className="overflow-hidden rounded-lg border">
          <Table aria-label="Sandbox blueprints">
            <TableHeader className="bg-muted/50">
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Connection</TableHead>
                <TableHead>Template</TableHead>
                <TableHead>Reuse</TableHead>
                <TableHead>Agents</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {blueprints.length ? (
                blueprints.map((blueprint) => (
                  <TableRow key={blueprint.id}>
                    <TableCell className="h-12 font-medium">
                      <Link
                        to="/sandboxes/$blueprintId/settings"
                        params={{ blueprintId: blueprint.id }}
                        className="hover:underline"
                      >
                        {blueprint.name}
                      </Link>
                    </TableCell>
                    <TableCell>
                      <ConnectionCell id={blueprint.connectionId} available={available} />
                    </TableCell>
                    <TableCell>
                      <code className="text-xs">{blueprint.template}</code>
                    </TableCell>
                    <TableCell>{REUSE[reuseOf(blueprint.reuse)].label}</TableCell>
                    <TableCell>{blueprint.agentIds.length}</TableCell>
                  </TableRow>
                ))
              ) : (
                <TableRow>
                  <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                    {loading
                      ? "Loading blueprints…"
                      : error
                        ? "Unable to load blueprints."
                        : search
                          ? "No blueprints match this search."
                          : "No sandbox blueprints yet."}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </div>
      <CreateBlueprintDialog
        open={creating}
        available={available}
        onOpenChange={setCreating}
        onCreated={(blueprint) =>
          void navigate({
            to: "/sandboxes/$blueprintId/settings",
            params: { blueprintId: blueprint.id },
          })
        }
      />
    </div>
  );
}

function CreateBlueprintDialog({
  open,
  available,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  available: SandboxConnections;
  onOpenChange: (open: boolean) => void;
  onCreated: (blueprint: SandboxBlueprint) => void;
}) {
  const form = useForm<BlueprintValues>({ defaultValues: blueprintValues() });
  const [error, setError] = useState("");
  function close() {
    onOpenChange(false);
    setError("");
    form.reset(blueprintValues());
  }
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>New blueprint</DialogTitle>
          <DialogDescription>
            What Tilde launches for an agent's sandbox, and how sandboxes are shared.
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={form.handleSubmit(async (values) => {
            setError("");
            try {
              const { blueprint } = await sandboxes.createSandboxBlueprint({
                name: values.name.trim(),
                connectionId: values.connectionId,
                template: values.template.trim(),
                reuse: Number(values.reuse) as SandboxReuse,
                timings: timingsOf(values),
              });
              if (blueprint) onCreated(blueprint);
              close();
            } catch (e) {
              setError(message(e));
            }
          })}
        >
          <BlueprintFields form={form} available={available} />
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
              Create blueprint
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
