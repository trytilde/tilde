import { useCallback, useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { Link } from "@tanstack/react-router";
import { timestampDate } from "@bufbuild/protobuf/wkt";
import { PencilIcon, PlusIcon } from "lucide-react";
import type { Routine, SignalType } from "@trytilde/contracts/tilde/management/v1/routines_pb.js";
import { Capability, type Connection } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, routines } from "@/client";
import { message } from "./skill-common";
import { RemoveButton } from "./remove-button";
import { TableSkeletonRows } from "./table-skeleton";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import { Textarea } from "./ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

const COLUMNS = 6;
const when = (routine: Routine) =>
  routine.lastRunAt ? timestampDate(routine.lastRunAt).toLocaleString() : undefined;

// Every page: the picker offers them all and existing routines name theirs by slug.
async function signalConnections(signal: AbortSignal) {
  const all: Connection[] = [];
  let pageToken = "";
  do {
    const page = await connections.listConnections(
      { capability: Capability.SIGNAL, pageSize: 100, pageToken },
      { signal },
    );
    all.push(...page.connections);
    pageToken = page.nextPageToken;
  } while (pageToken);
  return all;
}

/**
 * An agent's routines: each prompts the agent on one cron schedule (UTC) or one signal of a
 * signal-capable connection, starting a run in a new thread every time it fires.
 */
export function AgentRoutines({ agentId }: { agentId: string }) {
  const [list, setList] = useState<Routine[]>([]);
  const [sources, setSources] = useState<Connection[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // `null` creates a routine; a routine edits it.
  const [editing, setEditing] = useState<Routine | null>();
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const response = await routines.listRoutines({ agentId }, { signal });
      if (!signal?.aborted) setList(response.routines);
    },
    [agentId],
  );
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    void Promise.all([refresh(abort.signal), signalConnections(abort.signal).then(setSources)])
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load routines."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);
  async function act(work: () => Promise<unknown>, fallback: string) {
    setError("");
    try {
      await work();
      await refresh();
    } catch (e) {
      setError(message(e, fallback));
    }
  }
  const slugOf = (id: string) => sources.find((c) => c.id === id)?.slug ?? "Unknown connection";
  return (
    <section className="space-y-5" aria-label="Routines">
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div className="flex justify-end">
        <Button
          className="cursor-pointer gap-2"
          disabled={loading}
          onClick={() => setEditing(null)}
        >
          <PlusIcon />
          New routine
        </Button>
      </div>
      <div className="overflow-hidden rounded-xl border">
        <Table aria-label="Routines">
          <TableHeader className="bg-background">
            <TableRow>
              <TableHead className="h-11 w-16 px-5">On</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Trigger</TableHead>
              <TableHead>Next run</TableHead>
              <TableHead>Last run</TableHead>
              <TableHead className="w-20" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableSkeletonRows columns={COLUMNS} />
            ) : list.length === 0 ? (
              <TableRow>
                <TableCell colSpan={COLUMNS} className="py-10 text-center text-muted-foreground">
                  No routines yet. A routine prompts this agent on a schedule or when a connection
                  emits a signal.
                </TableCell>
              </TableRow>
            ) : (
              list.map((routine) => (
                <TableRow key={routine.id}>
                  <TableCell className="px-5">
                    <Switch
                      aria-label={`${routine.enabled ? "Disable" : "Enable"} ${routine.name}`}
                      checked={routine.enabled}
                      onCheckedChange={(enabled) =>
                        void act(
                          () =>
                            routines.updateRoutine({
                              id: routine.id,
                              name: routine.name,
                              prompt: routine.prompt,
                              enabled,
                              trigger: routine.trigger,
                            }),
                          "Unable to update the routine.",
                        )
                      }
                    />
                  </TableCell>
                  <TableCell className="font-medium">{routine.name}</TableCell>
                  <TableCell>
                    {routine.trigger.case === "cron" ? (
                      <span>
                        <code className="font-mono text-xs">{routine.trigger.value.schedule}</code>
                        <span className="ml-1 text-xs text-muted-foreground">UTC</span>
                      </span>
                    ) : routine.trigger.case === "signal" ? (
                      <span className="grid">
                        <code className="font-mono text-xs">
                          {routine.trigger.value.signalType}
                        </code>
                        <span className="text-xs text-muted-foreground">
                          {slugOf(routine.trigger.value.connectionId)}
                        </span>
                      </span>
                    ) : null}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {routine.nextRunAt
                      ? timestampDate(routine.nextRunAt).toLocaleString()
                      : routine.trigger.case === "signal"
                        ? "On signal"
                        : "—"}
                  </TableCell>
                  <TableCell>
                    {routine.lastError ? (
                      <span className="text-destructive" title={routine.lastError}>
                        Failed {when(routine)}
                      </span>
                    ) : routine.lastThreadId ? (
                      <Link
                        className="underline-offset-4 hover:underline"
                        to="/agent/$agentId/sessions"
                        params={{ agentId }}
                        search={{ inspectSession: routine.lastThreadId }}
                      >
                        {when(routine)}
                      </Link>
                    ) : (
                      <span className="text-muted-foreground">Never</span>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Edit ${routine.name}`}
                        onClick={() => setEditing(routine)}
                      >
                        <PencilIcon />
                      </Button>
                      <RemoveButton
                        label={`Delete ${routine.name}`}
                        title="Delete routine?"
                        description={`${routine.name} stops running. Threads it started are kept.`}
                        confirmLabel="Delete"
                        onConfirm={() =>
                          act(
                            () => routines.deleteRoutine({ id: routine.id }),
                            "Unable to delete the routine.",
                          )
                        }
                      />
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
      {editing !== undefined && (
        <RoutineDialog
          agentId={agentId}
          routine={editing ?? undefined}
          sources={sources}
          onClose={() => setEditing(undefined)}
          onSaved={() => {
            setEditing(undefined);
            void refresh().catch((e) => setError(message(e, "Unable to load routines.")));
          }}
        />
      )}
    </section>
  );
}

type RoutineForm = {
  name: string;
  prompt: string;
  enabled: boolean;
  kind: "cron" | "signal";
  schedule: string;
  connectionId: string;
  signalType: string;
};
const kinds = [
  { value: "cron", label: "Schedule" },
  { value: "signal", label: "Signal" },
];

function RoutineDialog({
  agentId,
  routine,
  sources,
  onClose,
  onSaved,
}: {
  agentId: string;
  routine?: Routine;
  sources: Connection[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [error, setError] = useState("");
  const [signalTypes, setSignalTypes] = useState<SignalType[]>([]);
  const trigger = routine?.trigger;
  const form = useForm<RoutineForm>({
    defaultValues: {
      name: routine?.name ?? "",
      prompt: routine?.prompt ?? "",
      enabled: routine?.enabled ?? true,
      kind: trigger?.case === "signal" ? "signal" : "cron",
      schedule: trigger?.case === "cron" ? trigger.value.schedule : "0 9 * * 1-5",
      connectionId: trigger?.case === "signal" ? trigger.value.connectionId : "",
      signalType: trigger?.case === "signal" ? trigger.value.signalType : "",
    },
  });
  const { errors, isSubmitting } = form.formState;
  const kind = form.watch("kind");
  const connectionId = form.watch("connectionId");
  useEffect(() => {
    setSignalTypes([]);
    if (!connectionId) return;
    const abort = new AbortController();
    routines
      .listSignalTypes({ connectionId }, { signal: abort.signal })
      .then((response) => setSignalTypes(response.signalTypes))
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load signal types."));
      });
    return () => abort.abort();
  }, [connectionId]);
  async function submit(values: RoutineForm) {
    setError("");
    const fields = {
      name: values.name.trim(),
      prompt: values.prompt,
      enabled: values.enabled,
      trigger:
        values.kind === "cron"
          ? { case: "cron" as const, value: { schedule: values.schedule.trim() } }
          : {
              case: "signal" as const,
              value: { connectionId: values.connectionId, signalType: values.signalType },
            },
    };
    try {
      if (routine) await routines.updateRoutine({ id: routine.id, ...fields });
      else await routines.createRoutine({ agentId, ...fields });
      onSaved();
    } catch (e) {
      setError(message(e, "Unable to save the routine."));
    }
  }
  const required = (label: string) => (v: string) => !!v.trim() || `Enter ${label}.`;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{routine ? "Edit routine" : "New routine"}</DialogTitle>
          <DialogDescription>
            Each time the trigger fires, the agent starts a run in a new thread with this prompt.
          </DialogDescription>
        </DialogHeader>
        <form className="grid gap-4" onSubmit={form.handleSubmit(submit)}>
          <div className="grid gap-2">
            <Label htmlFor="routine-name">Name</Label>
            <Input
              id="routine-name"
              placeholder="Daily digest"
              aria-invalid={!!errors.name}
              {...form.register("name", { validate: required("a name") })}
            />
            {errors.name && <p className="m-0 text-xs text-destructive">{errors.name.message}</p>}
          </div>
          <div className="grid gap-2">
            <Label htmlFor="routine-kind">Trigger</Label>
            <Controller
              control={form.control}
              name="kind"
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange} items={kinds}>
                  <SelectTrigger id="routine-kind">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {kinds.map((k) => (
                      <SelectItem key={k.value} value={k.value}>
                        {k.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
          </div>
          {kind === "cron" ? (
            <div className="grid gap-2">
              <Label htmlFor="routine-schedule">Cron schedule (UTC)</Label>
              <Input
                id="routine-schedule"
                className="font-mono"
                placeholder="0 9 * * 1-5"
                aria-invalid={!!errors.schedule}
                {...form.register("schedule", {
                  validate: (v) =>
                    v.trim().split(/\s+/).length === 5 ||
                    "Enter five fields: minute hour day-of-month month day-of-week.",
                })}
              />
              {errors.schedule ? (
                <p className="m-0 text-xs text-destructive">{errors.schedule.message}</p>
              ) : (
                <p className="m-0 text-xs text-muted-foreground">
                  For example <code>0 9 * * 1-5</code> runs at 09:00 UTC on weekdays and{" "}
                  <code>*/30 * * * *</code> every half hour.
                </p>
              )}
            </div>
          ) : sources.length === 0 ? (
            <p className="m-0 text-sm text-muted-foreground">
              No connection emits signals yet. Connect a GitHub App on the{" "}
              <Link className="underline underline-offset-4" to="/tools/connections">
                Connections
              </Link>{" "}
              page, or as one of this agent's chat providers.
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-3">
              <div className="grid gap-2">
                <Label htmlFor="routine-connection">Connection</Label>
                <Controller
                  control={form.control}
                  name="connectionId"
                  rules={{ validate: required("a connection") }}
                  render={({ field }) => (
                    <Select
                      value={field.value}
                      onValueChange={(value) => {
                        field.onChange(value);
                        form.setValue("signalType", "");
                      }}
                      items={sources.map((c) => ({ value: c.id, label: c.slug }))}
                    >
                      <SelectTrigger id="routine-connection" aria-invalid={!!errors.connectionId}>
                        <SelectValue placeholder="Choose" />
                      </SelectTrigger>
                      <SelectContent>
                        {sources.map((c) => (
                          <SelectItem key={c.id} value={c.id}>
                            {c.slug}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="routine-signal">Signal</Label>
                <Controller
                  control={form.control}
                  name="signalType"
                  rules={{ validate: required("a signal") }}
                  render={({ field }) => (
                    <Select
                      value={field.value}
                      onValueChange={field.onChange}
                      disabled={!connectionId}
                      items={signalTypes.map((s) => ({ value: s.id, label: s.id }))}
                    >
                      <SelectTrigger id="routine-signal" aria-invalid={!!errors.signalType}>
                        <SelectValue placeholder="Choose" />
                      </SelectTrigger>
                      <SelectContent>
                        {signalTypes.map((s) => (
                          <SelectItem key={s.id} value={s.id}>
                            <span className="grid">
                              <span className="font-mono text-xs">{s.id}</span>
                              <span className="text-xs text-muted-foreground">{s.description}</span>
                            </span>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                />
              </div>
              {(errors.connectionId || errors.signalType) && (
                <p className="col-span-2 m-0 text-xs text-destructive">
                  {errors.connectionId?.message ?? errors.signalType?.message}
                </p>
              )}
              <p className="col-span-2 m-0 text-xs text-muted-foreground">
                The signal's details are appended to the prompt.
              </p>
            </div>
          )}
          <div className="grid gap-2">
            <Label htmlFor="routine-prompt">Prompt</Label>
            <Textarea
              id="routine-prompt"
              rows={6}
              placeholder="Summarise yesterday's activity and post it to the team."
              aria-invalid={!!errors.prompt}
              {...form.register("prompt", { validate: required("a prompt") })}
            />
            {errors.prompt && (
              <p className="m-0 text-xs text-destructive">{errors.prompt.message}</p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Controller
              control={form.control}
              name="enabled"
              render={({ field }) => (
                <Switch
                  id="routine-enabled"
                  checked={field.value}
                  onCheckedChange={field.onChange}
                />
              )}
            />
            <Label htmlFor="routine-enabled">Enabled</Label>
          </div>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={isSubmitting || (kind === "signal" && sources.length === 0)}
            >
              {routine ? "Save" : "Create routine"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
