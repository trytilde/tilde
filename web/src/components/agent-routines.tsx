import { useCallback, useEffect, useRef, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { Link } from "@tanstack/react-router";
import { timestampDate } from "@bufbuild/protobuf/wkt";
import { CableIcon, ChevronDownIcon, ClockIcon, PencilIcon, PlusIcon, ZapIcon } from "lucide-react";
import type {
  Routine,
  SignalType,
  SignalVariable,
} from "@trytilde/contracts/tilde/management/v1/routines_pb.js";
import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, routines } from "@/client";
import { randomUUID } from "@/lib/browser-crypto";
import { message, pillClass } from "./skill-common";
import { ChooseDialog } from "./choose-dialog";
import { ConnectionSetupDialog, type Brokering } from "./connection-setup-dialog";
import { RemoveButton } from "./remove-button";
import { TableSkeletonRows } from "./table-skeleton";
import { TemplateInput, VariableBadge, type TemplateVariable } from "./template-input";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";

const COLUMNS = 6;
/** What a cron routine's templates may name; rendered by the server (`routines::tick`). */
const CRON_VARIABLES: TemplateVariable[] = [
  {
    key: "scheduled_at",
    description: "The time this run was scheduled for, in UTC.",
    example: "2026-10-05T09:00:00+00:00",
  },
];
const when = (routine: Routine) =>
  routine.lastRunAt ? timestampDate(routine.lastRunAt).toLocaleString() : undefined;
/** A provider's way to connect that emits signals; the catalog shows one card per method. */
type SignalMethod = { provider: Provider; typeId: string };

async function all<T>(page: (pageToken: string) => Promise<{ items: T[]; next: string }>) {
  const items: T[] = [];
  let pageToken = "";
  do {
    const result = await page(pageToken);
    items.push(...result.items);
    pageToken = result.next;
  } while (pageToken);
  return items;
}

/** Which routine the dialog edits, or the trigger a new one starts with. */
type Editing = { routine: Routine } | { kind: "cron" } | { kind: "signal"; connectionId: string };

/**
 * An agent's routines: each prompts the agent on one cron schedule (UTC) or one signal of a
 * signal-capable connection, starting a run in a new thread every time it fires. New routines
 * start from two pills above the table: the Tilde catalog (an existing signal-capable connection,
 * or a new one set up from the catalog of providers that emit signals) or a schedule.
 */
export function AgentRoutines({ agentId }: { agentId: string }) {
  const [list, setList] = useState<Routine[]>([]);
  const [sources, setSources] = useState<Connection[]>([]);
  const [choosing, setChoosing] = useState<"existing" | "new">();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<Editing>();
  const [setup, setSetup] = useState<(Brokering & { connectionId: string }) | null>(null);
  const [setupError, setSetupError] = useState("");
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const response = await routines.listRoutines({ agentId }, { signal });
      if (!signal?.aborted) setList(response.routines);
    },
    [agentId],
  );
  const loadSources = useCallback(async (signal?: AbortSignal) => {
    const found = await all((pageToken) =>
      connections
        .listConnections({ capability: Capability.SIGNAL, pageSize: 100, pageToken }, { signal })
        .then((page) => ({ items: page.connections, next: page.nextPageToken })),
    );
    if (!signal?.aborted) setSources(found);
    return found;
  }, []);
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    void Promise.all([refresh(abort.signal), loadSources(abort.signal)])
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load routines."));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh, loadSources]);
  // A finished setup opens the new routine on the connection it created.
  useEffect(() => {
    if (!setup?.url) return;
    const origin = new URL(setup.url).origin;
    const completed = (event: MessageEvent) => {
      if (
        event.origin !== origin ||
        event.data?.connectionId !== setup.connectionId ||
        event.data?.type !== "tilde.connection.complete"
      )
        return;
      setSetup(null);
      void loadSources()
        .then(() => setEditing({ kind: "signal", connectionId: setup.connectionId }))
        .catch((e) => setError(message(e, "Unable to load connections.")));
    };
    window.addEventListener("message", completed);
    return () => window.removeEventListener("message", completed);
  }, [setup, loadSources]);
  async function act(work: () => Promise<unknown>, fallback: string) {
    setError("");
    try {
      await work();
      await refresh();
    } catch (e) {
      setError(message(e, fallback));
    }
  }
  async function connect(provider: Provider, typeId: string) {
    const connectionId = randomUUID();
    const title = `Connect ${provider.name}`;
    setSetupError("");
    setSetup({ title, url: "", connectionId });
    try {
      const started = await connections.startConnection({
        id: connectionId,
        name: provider.name,
        providerId: provider.id,
        typeId,
        assignments: [],
      });
      const url = new URL(started.brokeringUrl);
      if (!["https:", "http:"].includes(url.protocol))
        throw new Error("Unable to open connection setup.");
      setSetup({ title, url: url.href, connectionId });
    } catch (e) {
      setSetupError(message(e, "Unable to start the connection setup."));
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
      <div className="flex flex-wrap gap-2" role="group" aria-label="Add a routine">
        <DropdownMenu>
          <DropdownMenuTrigger
            disabled={loading}
            render={<Button variant="outline" className={pillClass} />}
          >
            <ZapIcon />
            Tilde catalog
            <ChevronDownIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-64">
            <DropdownMenuItem onClick={() => setChoosing("existing")}>
              <CableIcon />
              Use existing connection
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => setChoosing("new")}>
              <PlusIcon />
              Set up new connection
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          variant="outline"
          className={pillClass}
          disabled={loading}
          onClick={() => setEditing({ kind: "cron" })}
        >
          <ClockIcon />
          Scheduled routine
        </Button>
      </div>
      <ChooseDialog<Connection>
        open={choosing === "existing"}
        title="Use an existing connection"
        description="Connections whose provider emits signals. Next, choose the signal that runs this agent."
        searchLabel="Search connections"
        listLabel="Signal connections"
        empty={(search) =>
          search
            ? "No signal connections match your search."
            : "No connections emit signals yet. Set up a new one from the Tilde catalog."
        }
        busy={false}
        load={async (search, signal) => {
          // Connections carry no icon; their providers do.
          const providers = await connections.listProviders(
            { capability: Capability.SIGNAL, pageSize: 100 },
            { signal },
          );
          const icons = new Map(providers.providers.map((p) => [p.id, p.iconUrl]));
          return (
            await all((pageToken) =>
              connections
                .listConnections(
                  {
                    capability: Capability.SIGNAL,
                    search,
                    status: "ready",
                    pageSize: 100,
                    pageToken,
                  },
                  { signal },
                )
                .then((page) => ({ items: page.connections, next: page.nextPageToken })),
            )
          ).map((connection) => ({
            id: connection.id,
            name: connection.slug,
            detail: connection.accountLabel ?? connection.name,
            iconUrl: icons.get(connection.providerId),
            value: connection,
          }));
        }}
        onClose={() => setChoosing(undefined)}
        onChoose={async (choice) => {
          setChoosing(undefined);
          setEditing({ kind: "signal", connectionId: choice.value.id });
        }}
      />
      <ChooseDialog<SignalMethod>
        open={choosing === "new"}
        title="Connect a provider"
        description="Providers whose connections emit signals. Set one up, then choose the signal that runs this agent."
        searchLabel="Search the catalog"
        listLabel="Signal providers"
        empty={(search) =>
          search ? "No signal providers match your search." : "No providers emit signals."
        }
        busy={false}
        load={async (search, signal) =>
          (
            await all((pageToken) =>
              connections
                .listProviders(
                  { capability: Capability.SIGNAL, search, pageSize: 100, pageToken },
                  { signal },
                )
                .then((page) => ({ items: page.providers, next: page.nextPageToken })),
            )
          ).flatMap((provider) => {
            const methods = provider.connectionTypes.filter((type) =>
              type.capabilities.includes(Capability.SIGNAL),
            );
            return methods.map((method) => ({
              id: `${provider.id}/${method.id}`,
              name: provider.name,
              detail: methods.length > 1 ? method.name : (provider.instructions ?? method.name),
              iconUrl: provider.iconUrl,
              value: { provider, typeId: method.id },
            }));
          })
        }
        onClose={() => setChoosing(undefined)}
        onChoose={async (choice) => {
          setChoosing(undefined);
          void connect(choice.value.provider, choice.value.typeId);
        }}
      />
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
                  No routines yet. Use the Tilde catalog to run this agent when a connection emits a
                  signal, or add a scheduled routine.
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
                              threadTitle: routine.threadTitle,
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
                        onClick={() => setEditing({ routine })}
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
      {editing && (
        <RoutineDialog
          agentId={agentId}
          editing={editing}
          sources={sources}
          onClose={() => setEditing(undefined)}
          onSaved={() => {
            setEditing(undefined);
            void refresh().catch((e) => setError(message(e, "Unable to load routines.")));
          }}
        />
      )}
      <ConnectionSetupDialog
        setup={setup}
        error={setupError}
        onClose={() => {
          setSetup(null);
          void loadSources().catch((e) => setError(message(e, "Unable to load connections.")));
        }}
      />
    </section>
  );
}

type RoutineForm = {
  name: string;
  prompt: string;
  threadTitle: string;
  schedule: string;
  connectionId: string;
  signalType: string;
};
type TemplateField = "prompt" | "threadTitle";

/**
 * Creates or edits one routine. The form is on the left; the variables its templates may use
 * (the chosen connection's signal variables, or the schedule's) are on the right, where clicking
 * one inserts it into the template field last focused.
 */
function RoutineDialog({
  agentId,
  editing,
  sources,
  onClose,
  onSaved,
}: {
  agentId: string;
  editing: Editing;
  sources: Connection[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const routine = "routine" in editing ? editing.routine : undefined;
  const trigger = routine?.trigger;
  const kind = "kind" in editing ? editing.kind : trigger?.case === "signal" ? "signal" : "cron";
  const [error, setError] = useState("");
  const [signalTypes, setSignalTypes] = useState<SignalType[]>([]);
  const [signalVariables, setSignalVariables] = useState<SignalVariable[]>([]);
  const [target, setTarget] = useState<TemplateField>("prompt");
  const form = useForm<RoutineForm>({
    defaultValues: {
      name: routine?.name ?? "",
      prompt: routine?.prompt ?? "",
      threadTitle: routine?.threadTitle ?? "",
      schedule: trigger?.case === "cron" ? trigger.value.schedule : "0 9 * * 1-5",
      connectionId:
        trigger?.case === "signal"
          ? trigger.value.connectionId
          : "connectionId" in editing
            ? editing.connectionId
            : "",
      signalType: trigger?.case === "signal" ? trigger.value.signalType : "",
    },
  });
  const { errors, isSubmitting } = form.formState;
  const connectionId = form.watch("connectionId");
  const signalType = form.watch("signalType");
  const connection = sources.find((c) => c.id === connectionId);
  useEffect(() => {
    setSignalTypes([]);
    setSignalVariables([]);
    if (kind !== "signal" || !connectionId) return;
    const abort = new AbortController();
    routines
      .listSignalTypes({ connectionId }, { signal: abort.signal })
      .then((response) => {
        setSignalTypes(response.signalTypes);
        setSignalVariables(response.variables);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e, "Unable to load signal types."));
      });
    return () => abort.abort();
  }, [kind, connectionId]);
  // Choosing a signal fills an untouched thread title with that signal's default.
  const defaultTitle = useRef("");
  useEffect(() => {
    const chosen = signalTypes.find((t) => t.id === signalType);
    if (!chosen) return;
    const title = form.getValues("threadTitle");
    if (!title || title === defaultTitle.current)
      form.setValue("threadTitle", chosen.defaultThreadTitle);
    defaultTitle.current = chosen.defaultThreadTitle;
  }, [signalType, signalTypes, form]);
  // The chosen signal is the example of `signal_type`.
  const variables: TemplateVariable[] =
    kind === "cron"
      ? CRON_VARIABLES
      : signalVariables.map((v) =>
          v.key === "signal_type" && signalType ? { ...v, example: signalType } : v,
        );
  function insert(variable: TemplateVariable) {
    form.setValue(target, `${form.getValues(target)}{{ ${variable.key} }}`, { shouldDirty: true });
  }
  async function submit(values: RoutineForm) {
    setError("");
    const fields = {
      name: values.name.trim(),
      prompt: values.prompt,
      threadTitle: values.threadTitle.trim(),
      trigger:
        kind === "cron"
          ? { case: "cron" as const, value: { schedule: values.schedule.trim() } }
          : {
              case: "signal" as const,
              value: { connectionId: values.connectionId, signalType: values.signalType },
            },
    };
    try {
      // Enabling is the table's switch; an edit keeps it.
      if (routine)
        await routines.updateRoutine({ id: routine.id, enabled: routine.enabled, ...fields });
      else await routines.createRoutine({ agentId, ...fields });
      onSaved();
    } catch (e) {
      setError(message(e, "Unable to save the routine."));
    }
  }
  const required = (label: string) => (v: string) => !!v.trim() || `Enter ${label}.`;
  const title = routine
    ? "Edit routine"
    : kind === "cron"
      ? "New scheduled routine"
      : `New ${connection?.slug ?? "signal"} routine`;
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[min(860px,calc(100dvh-2rem))] gap-0 overflow-hidden p-0 sm:max-w-5xl">
        <form
          className="grid max-h-[inherit] grid-rows-[auto_minmax(0,1fr)_auto]"
          onSubmit={form.handleSubmit(submit)}
        >
          <DialogHeader className="border-b p-5">
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>
              {kind === "cron"
                ? "On each scheduled time the agent starts a run in a new thread with this prompt."
                : "Each time the signal arrives the agent starts a run in a new thread with this prompt, followed by the signal's details."}{" "}
              Type <code className="rounded bg-muted px-1 font-mono">{"{{"}</code> in the thread
              title or prompt to insert a variable.
            </DialogDescription>
          </DialogHeader>
          <div className="grid min-h-0 md:grid-cols-[minmax(0,1fr)_20rem]">
            <div className="grid content-start gap-4 overflow-y-auto p-5">
              <div className="grid gap-2">
                <Label htmlFor="routine-name">Name</Label>
                <Input
                  id="routine-name"
                  placeholder={kind === "cron" ? "Daily digest" : "Triage new issues"}
                  aria-invalid={!!errors.name}
                  {...form.register("name", { validate: required("a name") })}
                />
                {errors.name && (
                  <p className="m-0 text-xs text-destructive">{errors.name.message}</p>
                )}
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
              ) : (
                <div className="grid gap-3 sm:grid-cols-2">
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
                          <SelectTrigger
                            id="routine-connection"
                            className="w-full"
                            aria-invalid={!!errors.connectionId}
                          >
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
                    <Label htmlFor="routine-signal">When this signal arrives</Label>
                    <Controller
                      control={form.control}
                      name="signalType"
                      rules={{ validate: required("a signal") }}
                      render={({ field }) => (
                        <Select
                          value={field.value}
                          onValueChange={field.onChange}
                          disabled={!connectionId}
                          items={signalTypes.map((s) => ({ value: s.id, label: s.name }))}
                        >
                          <SelectTrigger
                            id="routine-signal"
                            className="w-full"
                            aria-invalid={!!errors.signalType}
                          >
                            <SelectValue placeholder="Choose" />
                          </SelectTrigger>
                          <SelectContent>
                            {signalTypes.map((s) => (
                              <SelectItem key={s.id} value={s.id}>
                                <span className="grid">
                                  <span>{s.name}</span>
                                  <span className="font-mono text-[11px] text-muted-foreground">
                                    {s.id}
                                  </span>
                                </span>
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    />
                  </div>
                  {(errors.connectionId || errors.signalType) && (
                    <p className="m-0 text-xs text-destructive sm:col-span-2">
                      {errors.connectionId?.message ?? errors.signalType?.message}
                    </p>
                  )}
                </div>
              )}
              <div className="grid gap-2">
                <Label htmlFor="routine-thread-title">Thread title</Label>
                <Controller
                  control={form.control}
                  name="threadTitle"
                  render={({ field }) => (
                    <TemplateInput
                      id="routine-thread-title"
                      value={field.value}
                      onChange={field.onChange}
                      onFocus={() => setTarget("threadTitle")}
                      variables={variables}
                      placeholder="The routine's name"
                    />
                  )}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="routine-prompt">Prompt</Label>
                <Controller
                  control={form.control}
                  name="prompt"
                  rules={{ validate: required("a prompt") }}
                  render={({ field }) => (
                    <TemplateInput
                      id="routine-prompt"
                      multiline
                      value={field.value}
                      onChange={field.onChange}
                      onFocus={() => setTarget("prompt")}
                      variables={variables}
                      invalid={!!errors.prompt}
                      placeholder={
                        kind === "cron"
                          ? "Summarise yesterday's activity and post it to the team."
                          : "Triage this and reply with next steps."
                      }
                    />
                  )}
                />
                {errors.prompt && (
                  <p className="m-0 text-xs text-destructive">{errors.prompt.message}</p>
                )}
              </div>
              {error && (
                <p role="alert" className="m-0 text-sm text-destructive">
                  {error}
                </p>
              )}
            </div>
            <aside
              aria-label="Variables"
              className="flex min-h-0 flex-col border-t bg-muted/30 md:border-t-0 md:border-l"
            >
              <div className="border-b px-4 py-3">
                <h3 className="m-0 text-sm font-medium">Variables</h3>
                <p className="m-0 text-xs text-muted-foreground">
                  Click one to add it to the {target === "prompt" ? "prompt" : "thread title"}.
                </p>
              </div>
              <ul className="m-0 grid list-none content-start gap-1 overflow-y-auto p-2">
                {kind === "signal" && !connectionId ? (
                  <li className="p-2 text-xs text-muted-foreground">
                    Choose a connection to see its variables.
                  </li>
                ) : (
                  variables.map((variable) => (
                    <li key={variable.key}>
                      <button
                        type="button"
                        className="grid w-full cursor-pointer gap-1 rounded-lg p-2 text-left hover:bg-muted"
                        onClick={() => insert(variable)}
                      >
                        <VariableBadge variable={variable} />
                        <span className="text-xs text-muted-foreground">
                          {variable.description}
                        </span>
                      </button>
                    </li>
                  ))
                )}
              </ul>
            </aside>
          </div>
          <DialogFooter className="m-0 border-t p-4">
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {routine ? "Save" : "Create routine"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
