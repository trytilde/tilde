import { TildeChatKey } from "./tilde-chat-provider";
import { LoadingReveal } from "@trytilde/connection-ui";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  CableIcon,
  CheckIcon,
  CopyIcon,
  PencilIcon,
  PlusIcon,
  SettingsIcon,
  SquareArrowOutUpRightIcon,
} from "lucide-react";
import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections, inference } from "@/client";
import { InferenceUsageChart, dollars } from "./inference-usage-chart";
import {
  BudgetAction,
  BudgetPeriod,
  BudgetScope,
  type Budget,
  type ConnectionUsage,
} from "@trytilde/contracts/tilde/management/v1/inference_pb.js";
import { RemoveButton } from "./remove-button";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { Badge } from "./ui/badge";
import { ProviderIcon } from "./provider-icon";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { randomUUID } from "@/lib/browser-crypto";
import { SkeletonLines } from "@/components/table-skeleton";

type Setup = { title: string; url: string; connectionId: string };
const methodsFor = (provider: Provider, capability: Capability) =>
  provider.connectionTypes.filter((method) => method.capabilities.includes(capability));
/** Every agent owns one Tilde connection, created with it; nobody adds or removes one. */
const TILDE = "tilde";
const addable = (providers: Provider[]) => providers.filter((provider) => provider.id !== TILDE);
const message = (error: unknown) =>
  error instanceof Error ? error.message : "Unable to update providers.";
/** Copy per capability; the assignment mechanics are identical. */
const copy: Record<
  number,
  { section: string; loading: string; none: string; unassigned: string; existing: string }
> = {
  [Capability.CHANNEL]: {
    section: "Chat providers",
    loading: "Loading chat providers",
    none: "No chat providers available.",
    unassigned: "No chat providers assigned",
    existing: "Choose a connection whose chat capability is available.",
  },
  [Capability.INFERENCE]: {
    section: "Inference providers",
    loading: "Loading inference providers",
    none: "No inference providers available.",
    unassigned: "No inference providers assigned",
    existing: "Choose an existing API key; several agents may share one.",
  },
};

/** Catalog-driven menus allocate the capability in the create request, before credentials are brokered. */
export function AgentConnections({
  agentId,
  capability = Capability.CHANNEL,
}: {
  agentId: string;
  capability?: Capability;
}) {
  const text = copy[capability];
  const [assigned, setAssigned] = useState<Connection[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [cursor, setCursor] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [existing, setExisting] = useState<Provider | null>(null);
  const [catalogOpen, setCatalogOpen] = useState(false);
  const [usage, setUsage] = useState<ConnectionUsage[]>([]);
  const [budgets, setBudgets] = useState<Budget[]>([]);
  const [spendVersion, setSpendVersion] = useState(0);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [setupLoaded, setSetupLoaded] = useState(false);
  const [setupError, setSetupError] = useState("");
  const [retry, setRetry] = useState<(() => Promise<void>) | null>(null);
  const creation = useRef<{ key: string; id: string } | null>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const setupAttempt = useRef(0);
  const assignment = { capability, agentId };
  const refresh = useCallback(async () => {
    const page = await connections.listConnections({ agentId, capability, pageSize: 30 });
    setAssigned(page.connections);
    setCursor(page.nextPageToken);
    if (capability === Capability.INFERENCE) {
      const [spend, limits] = await Promise.all([
        inference.getUsage({ agentId }),
        inference.listBudgets({ scope: BudgetScope.AGENT, scopeId: agentId }),
      ]);
      setUsage(spend.connections);
      setBudgets(limits.budgets);
      setSpendVersion((v) => v + 1);
    }
  }, [agentId, capability]);

  useEffect(() => {
    const abort = new AbortController();
    void (async () => {
      setLoading(true);
      setError("");
      try {
        const catalog: Provider[] = [];
        let pageToken = "";
        do {
          const page = await connections.listProviders(
            { capability, pageSize: 100, pageToken },
            { signal: abort.signal },
          );
          catalog.push(...page.providers);
          pageToken = page.nextPageToken;
        } while (pageToken);
        if (!abort.signal.aborted) setProviders(catalog);
        await refresh();
      } catch (error) {
        if (!abort.signal.aborted) setError(message(error));
      } finally {
        if (!abort.signal.aborted) setLoading(false);
      }
    })();
    return () => abort.abort();
  }, [refresh, capability]);

  useEffect(() => {
    const completed = (event: MessageEvent) => {
      if (
        !setup?.url ||
        event.source !== frame.current?.contentWindow ||
        event.origin !== new URL(setup.url).origin ||
        event.data?.connectionId !== setup.connectionId
      )
        return;
      if (!["tilde.connection.complete", "tilde.connection.cancelled"].includes(event.data?.type))
        return;
      setSetup(null);
      void refresh().catch((error) => setError(message(error)));
    };
    window.addEventListener("message", completed);
    return () => window.removeEventListener("message", completed);
  }, [setup, refresh]);

  async function act(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  }
  async function broker(
    title: string,
    start: () => Promise<{ brokeringUrl: string; connection?: Connection }>,
  ) {
    const attempt = ++setupAttempt.current;
    setSetupLoaded(false);
    setSetup({ title, url: "", connectionId: "" });
    setSetupError("");
    setRetry(null);
    setBusy(true);
    try {
      const result = await start();
      void refresh().catch((error) => setError(message(error)));
      if (attempt !== setupAttempt.current) return;
      const url = new URL(result.brokeringUrl);
      if (!["https:", "http:"].includes(url.protocol) || !result.connection?.id)
        throw new Error("Unable to open connection setup.");
      setSetup({ title, url: url.href, connectionId: result.connection.id });
    } catch (error) {
      if (attempt !== setupAttempt.current) return;
      setSetupError(message(error));
      setRetry(() => () => broker(title, start));
    } finally {
      if (attempt === setupAttempt.current) setBusy(false);
    }
  }
  function add(provider: Provider, typeId: string) {
    const key = JSON.stringify([agentId, provider.id, typeId]);
    if (creation.current?.key !== key) creation.current = { key, id: randomUUID() };
    const id = creation.current.id;
    void broker(`Connect ${provider.name}`, async () => {
      const result = await connections.startConnection({
        id,
        name: provider.name,
        providerId: provider.id,
        typeId,
        assignments: [assignment],
      });
      if (creation.current?.id === id) creation.current = null;
      return result;
    });
  }

  return (
    <section className="space-y-5" aria-label={text.section}>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {loading && <SkeletonLines label={text.loading} />}
      {capability === Capability.INFERENCE ? (
        <div className="flex justify-end">
          <Button disabled={busy} onClick={() => setCatalogOpen(true)}>
            <PlusIcon />
            Add inference provider
          </Button>
          <Dialog open={catalogOpen} onOpenChange={setCatalogOpen}>
            <DialogContent className="sm:max-w-3xl">
              <DialogHeader>
                <DialogTitle>Add inference provider</DialogTitle>
                <DialogDescription>
                  Connect a new API key, or reuse a connection another agent already has.
                </DialogDescription>
              </DialogHeader>
              <ul className="m-0 grid list-none grid-cols-2 gap-x-2 gap-y-0.5 p-0 max-sm:grid-cols-1">
                {addable(providers).map((provider) => (
                  <li
                    key={provider.id}
                    className="flex min-w-0 items-center gap-3 rounded-2xl px-3 py-2.5 hover:bg-muted/60"
                  >
                    <ProviderIcon iconUrl={provider.iconUrl} />
                    <div className="flex min-w-0 flex-1 flex-col">
                      <h3 className="m-0 truncate text-[13px] leading-[18px] font-medium">
                        {provider.name}
                      </h3>
                    </div>
                    <DropdownMenu>
                      <DropdownMenuTrigger
                        disabled={busy}
                        render={
                          <button
                            type="button"
                            aria-label={`Add ${provider.name}`}
                            className="inline-flex size-6 cursor-pointer items-center justify-center rounded text-muted-foreground hover:text-foreground disabled:cursor-default disabled:opacity-50 [&_svg]:size-3.5"
                          />
                        }
                      >
                        <PlusIcon />
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          onClick={() => {
                            setCatalogOpen(false);
                            add(provider, methodsFor(provider, capability)[0].id);
                          }}
                        >
                          <PlusIcon />
                          Create new
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={() => {
                            setCatalogOpen(false);
                            setExisting(provider);
                          }}
                        >
                          <CableIcon />
                          Use existing
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </li>
                ))}
              </ul>
            </DialogContent>
          </Dialog>
        </div>
      ) : (
        <div className="flex flex-wrap gap-2">
          {addable(providers).map((provider) => {
            const methods = methodsFor(provider, capability);
            return (
              <DropdownMenu key={provider.id}>
                <DropdownMenuTrigger
                  disabled={busy}
                  render={
                    <Button
                      variant="outline"
                      className="h-[45px] w-[205px] max-w-full cursor-pointer gap-2 rounded-full border-border bg-background px-4 text-sm font-semibold shadow-sm hover:bg-muted/50"
                    />
                  }
                >
                  <ProviderIcon iconUrl={provider.iconUrl} />
                  <span className="min-w-0 truncate" title={provider.name}>
                    {provider.name}
                  </span>
                </DropdownMenuTrigger>
                <DropdownMenuContent>
                  <DropdownMenuItem onClick={() => setExisting(provider)}>
                    <CableIcon />
                    Use existing connection
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  {methods.length === 1 ? (
                    <DropdownMenuItem onClick={() => add(provider, methods[0].id)}>
                      <PlusIcon />
                      Add new connection
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuGroup>
                      <DropdownMenuLabel>Or add new</DropdownMenuLabel>
                      {methods.map((method) => (
                        <DropdownMenuItem key={method.id} onClick={() => add(provider, method.id)}>
                          <PlusIcon />
                          {method.name}
                        </DropdownMenuItem>
                      ))}
                    </DropdownMenuGroup>
                  )}
                </DropdownMenuContent>
              </DropdownMenu>
            );
          })}
        </div>
      )}
      {!loading && !providers.length && !error && (
        <p className="text-sm text-muted-foreground">{text.none}</p>
      )}
      <div className="space-y-3">
        {capability === Capability.INFERENCE && (
          <InferenceUsageChart
            agentId={agentId}
            refreshKey={spendVersion}
            names={Object.fromEntries(
              assigned.map((connection) => [connection.id, connection.slug]),
            )}
          />
        )}
        {assigned.length > 0 && (
          <ProviderConnectionsTable
            capability={capability}
            agentId={agentId}
            assigned={assigned}
            providers={providers}
            busy={busy}
            usage={usage}
            budgets={budgets}
            onBudget={(connection, limitMicros) =>
              act(async () => {
                const existing = budgets.find(
                  (b) => b.connectionId === connection.id && b.period === BudgetPeriod.MONTH,
                );
                if (limitMicros === null) {
                  if (existing) await inference.deleteBudget({ id: existing.id });
                } else {
                  await inference.setBudget({
                    scope: BudgetScope.AGENT,
                    scopeId: agentId,
                    connectionId: connection.id,
                    period: BudgetPeriod.MONTH,
                    limitMicros,
                    action: BudgetAction.BLOCK,
                  });
                }
                await refresh();
              })
            }
            onAlias={(connection, alias) =>
              act(async () => {
                await connections.assignCapability({
                  connectionId: connection.id,
                  assignment: { ...assignment, alias: alias || undefined },
                });
                await refresh();
              })
            }
            onConfigure={(connection) =>
              void broker(`Configure ${connection.name}`, () =>
                connections.reconnect({ id: connection.id }),
              )
            }
            onRemove={(connection) =>
              act(async () => {
                await connections.unassignCapability({ connectionId: connection.id, assignment });
                await refresh();
              })
            }
          />
        )}
        {!assigned.length && !loading && (
          <div className="flex min-h-48 w-full items-center justify-center rounded-xl border border-dashed bg-card p-6 text-center text-sm text-muted-foreground">
            {text.unassigned}
          </div>
        )}
        {cursor && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() =>
              void act(async () => {
                const page = await connections.listConnections({
                  agentId,
                  capability,
                  pageToken: cursor,
                  pageSize: 30,
                });
                setAssigned((current) => [...current, ...page.connections]);
                setCursor(page.nextPageToken);
              })
            }
          >
            More assigned connections
          </Button>
        )}
      </div>
      <Dialog
        open={!!existing}
        onOpenChange={(open) => {
          if (!open && !busy) setExisting(null);
        }}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Use an existing {existing?.name} connection</DialogTitle>
            <DialogDescription>{text.existing}</DialogDescription>
          </DialogHeader>
          {existing && (
            <ExistingConnections
              key={existing.id}
              provider={existing}
              capability={capability}
              assigned={assigned}
              busy={busy}
              onSelect={(connection) =>
                void act(async () => {
                  await connections.assignCapability({ connectionId: connection.id, assignment });
                  setExisting(null);
                  await refresh();
                  if (connection.status !== "ready" && connection.status !== "failed")
                    await broker(`Configure ${connection.name}`, () =>
                      connections.reconnect({ id: connection.id }),
                    );
                })
              }
            />
          )}
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
        </DialogContent>
      </Dialog>
      <Dialog
        open={!!setup}
        onOpenChange={(open) => {
          if (!open) {
            setupAttempt.current += 1;
            setBusy(false);
            setSetup(null);
            void refresh().catch((error) => setError(message(error)));
          }
        }}
      >
        <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-2xl" showCloseButton={false}>
          <DialogTitle className="sr-only">{setup?.title}</DialogTitle>
          <LoadingReveal
            loading={!setupLoaded && !setupError}
            label="Loading connection setup"
            className="h-[min(680px,75dvh)]"
          >
            {setupError ? (
              <div className="space-y-3 p-5">
                <p role="alert" className="text-destructive">
                  {setupError}
                </p>
                <Button onClick={() => void retry?.()}>Retry setup</Button>
              </div>
            ) : setup?.url ? (
              <iframe
                ref={frame}
                onLoad={() => setSetupLoaded(true)}
                title={setup.title}
                src={setup.url}
                referrerPolicy="no-referrer"
                sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
                className="h-full w-full border-0"
              />
            ) : null}
          </LoadingReveal>
        </DialogContent>
      </Dialog>
    </section>
  );
}

/** The server filters by provider; every page is loaded before declaring none available. */
function ExistingConnections({
  provider,
  busy,
  onSelect,
  capability,
  assigned,
}: {
  provider: Provider;
  busy: boolean;
  onSelect: (connection: Connection) => void;
  capability: Capability;
  assigned: Connection[];
}) {
  const [items, setItems] = useState<Connection[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    void (async () => {
      setLoading(true);
      setError("");
      try {
        const matches: Connection[] = [];
        let pageToken = "";
        do {
          const page = await connections.listConnections(
            { capability, providerId: provider.id, pageSize: 100, pageToken },
            { signal: abort.signal },
          );
          // The agent's own connections are few; they are left out here.
          matches.push(
            ...page.connections.filter(
              (connection) => !assigned.some((current) => current.id === connection.id),
            ),
          );
          pageToken = page.nextPageToken;
        } while (pageToken);
        if (!abort.signal.aborted) setItems(matches);
      } catch (error) {
        if (!abort.signal.aborted) setError(message(error));
      } finally {
        if (!abort.signal.aborted) setLoading(false);
      }
    })();
    return () => abort.abort();
  }, [provider.id, capability, assigned, attempt]);
  return (
    <div className="max-h-[50dvh] space-y-2 overflow-y-auto">
      {loading ? (
        <SkeletonLines label="Loading connections" />
      ) : error ? (
        <>
          <p role="alert">{error}</p>
          <Button onClick={() => setAttempt((value) => value + 1)}>Retry</Button>
        </>
      ) : items.length ? (
        items.map((connection) => (
          <Button
            key={connection.id}
            variant="outline"
            disabled={busy}
            className="h-auto w-full justify-between gap-3 py-3 text-left"
            onClick={() => onSelect(connection)}
          >
            <span>
              {connection.name}
              <span className="block text-xs font-normal text-muted-foreground">
                {connection.accountLabel ?? provider.name}
              </span>
            </span>
            <span className="text-xs font-normal">Use connection</span>
          </Button>
        ))
      ) : (
        <p className="text-muted-foreground">
          No available {provider.name} connections. Add a new connection from the provider menu.
        </p>
      )}
    </div>
  );
}

const connectionStatuses: Partial<Record<string, { label: string; color: string }>> = {
  ready: {
    label: "Ready",
    color: "border-success/20 bg-success/10 text-success",
  },
  requires_action: {
    label: "Requires action",
    color: "border-warning/20 bg-warning/10 text-warning",
  },
  failed: {
    label: "Failed",
    color: "border-destructive/20 bg-destructive/10 text-destructive",
  },
  disconnected: { label: "Disconnected", color: "border-border bg-muted text-muted-foreground" },
};
/** "Requires action" is a link into setup when `onAction` is given: pointer, hover and an arrow. */
function ConnectionStatusBadge({ status, onAction }: { status: string; onAction?: () => void }) {
  const presentation = connectionStatuses[status] ?? {
    label: status.replaceAll("_", " "),
    color: "border-border bg-muted text-muted-foreground",
  };
  if (status === "requires_action" && onAction)
    return (
      <Badge
        variant="outline"
        role="button"
        tabIndex={0}
        aria-label="Requires action: open setup"
        className={`${presentation.color} cursor-pointer gap-1 hover:bg-warning/20 focus-visible:ring-1 focus-visible:ring-ring`}
        onClick={onAction}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onAction();
          }
        }}
      >
        {presentation.label}
        <SquareArrowOutUpRightIcon className="size-3" />
      </Badge>
    );
  return (
    <Badge variant="outline" className={presentation.color}>
      {presentation.label}
    </Badge>
  );
}

/** Icon-only row action: no button chrome, just the glyph on the row's background. */
function IconAction({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="inline-flex size-6 cursor-pointer items-center justify-center rounded text-muted-foreground hover:text-foreground disabled:cursor-default disabled:opacity-50 [&_svg]:size-3.5"
    >
      {children}
    </button>
  );
}
/** Copy with a self-positioned "Copied" note; no popover machinery so it always renders. */
function CopyAction({
  value,
  label,
  disabled,
}: {
  value: string;
  label: string;
  disabled?: boolean;
}) {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);
  return (
    <span className="relative inline-flex">
      <IconAction
        label={label}
        disabled={disabled}
        onClick={() => {
          setCopyError(false);
          void navigator.clipboard
            .writeText(value)
            .then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1200);
            })
            .catch(() => setCopyError(true));
        }}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </IconAction>
      {copyError && (
        <span role="alert" className="text-xs text-destructive">
          Unable to copy
        </span>
      )}
      {copied && (
        <span
          role="status"
          className="pointer-events-none absolute -top-7 left-1/2 z-10 -translate-x-1/2 whitespace-nowrap rounded-md bg-foreground px-2 py-1 text-[11px] text-background shadow-sm"
        >
          Copied to clipboard
        </span>
      )}
    </span>
  );
}
/** Alias shown as text with copy and edit; the pencil swaps in an inline input with a save icon. */
function AliasCell({
  slug,
  alias,
  busy,
  onSave,
}: {
  slug: string;
  alias: string;
  busy: boolean;
  onSave: (alias: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(alias);
  const save = () => {
    setEditing(false);
    if (draft.trim() !== alias) void onSave(draft.trim());
  };
  if (editing)
    return (
      <span className="flex h-7 w-52 items-center gap-1">
        <Input
          autoFocus
          aria-label={`Alias for ${slug}`}
          className="h-7 min-w-0 flex-1 px-1.5 text-xs"
          value={draft}
          placeholder="e.g. fast"
          disabled={busy}
          onChange={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") save();
            if (event.key === "Escape") {
              setDraft(alias);
              setEditing(false);
            }
          }}
        />
        <IconAction label="Save alias" onClick={save} disabled={busy}>
          <CheckIcon />
        </IconAction>
      </span>
    );
  return (
    <span className="flex h-7 w-52 items-center gap-1">
      <code className="truncate text-xs">
        {alias || <span className="text-muted-foreground">none</span>}
      </code>
      <CopyAction value={alias} label="Copy alias" disabled={!alias} />
      <IconAction
        label="Edit alias"
        disabled={busy}
        onClick={() => {
          setDraft(alias);
          setEditing(true);
        }}
      >
        <PencilIcon />
      </IconAction>
    </span>
  );
}
/** One row per assigned connection, with the provider ID scoped to this agent and capability. */
function ProviderConnectionsTable({
  capability,
  agentId,
  assigned,
  providers,
  busy,
  onAlias,
  onConfigure,
  onRemove,
  usage = [],
  budgets = [],
  onBudget,
}: {
  capability: Capability;
  agentId: string;
  assigned: Connection[];
  providers: Provider[];
  busy: boolean;
  onAlias: (connection: Connection, alias: string) => Promise<void>;
  onConfigure: (connection: Connection) => void;
  onRemove: (connection: Connection) => Promise<void>;
  usage?: ConnectionUsage[];
  budgets?: Budget[];
  /** Inference only; `null` clears the monthly cap for this connection. */
  onBudget?: (connection: Connection, limitMicros: bigint | null) => Promise<void>;
}) {
  const spentOn = (connection: Connection) =>
    usage.find((u) => u.connectionId === connection.id)?.costMicros;
  const unpricedOn = (connection: Connection) =>
    usage.find((u) => u.connectionId === connection.id)?.unpricedRequests;
  const budgetOf = (connection: Connection) =>
    budgets.find((b) => b.connectionId === connection.id && b.period === BudgetPeriod.MONTH);
  const aliasOf = (connection: Connection) =>
    connection.associatedAgents.find(
      (agent) => agent.id === agentId && agent.capability === Capability.INFERENCE,
    )?.alias ?? "";
  return (
    <div className="overflow-hidden rounded-xl border">
      <Table aria-label={copy[capability].section} className="table-fixed">
        <TableHeader className="bg-muted/50">
          <TableRow>
            <TableHead className="h-11 w-14 px-5" />
            <TableHead className="h-11">Provider</TableHead>
            <TableHead className="h-11">Account</TableHead>
            <TableHead className="h-11">Provider ID</TableHead>
            {capability === Capability.INFERENCE && (
              <>
                <TableHead className="h-11 w-56">Alias</TableHead>
                <TableHead className="h-11 w-28">Spent</TableHead>
                <TableHead className="h-11 w-44">Budget</TableHead>
              </>
            )}
            <TableHead className="h-11 px-5 text-right">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {assigned.map((connection) => {
            const provider = providers.find((provider) => provider.id === connection.providerId);
            return (
              <TableRow key={connection.id}>
                <TableCell className="h-14 px-5 py-2">
                  <ProviderIcon iconUrl={provider?.iconUrl} />
                </TableCell>
                <TableCell className="font-medium">
                  {provider?.name ?? connection.providerId}
                </TableCell>
                <TableCell>
                  <span className="flex items-center gap-2">
                    {connection.name}
                    {connection.providerId === TILDE ? (
                      <Badge variant="outline" className={connectionStatuses.ready?.color}>
                        Always enabled
                      </Badge>
                    ) : (
                      <ConnectionStatusBadge
                        status={connection.status}
                        onAction={() => onConfigure(connection)}
                      />
                    )}
                  </span>
                </TableCell>
                <TableCell>
                  <span className="flex items-center gap-1">
                    <code className="text-xs">{connection.slug}</code>
                    <CopyAction
                      value={connection.slug}
                      label="Copy provider ID"
                      disabled={connection.status !== "ready"}
                    />
                  </span>
                </TableCell>
                {capability === Capability.INFERENCE && (
                  <TableCell>
                    <AliasCell
                      key={`${connection.id}:${aliasOf(connection)}`}
                      slug={connection.slug}
                      alias={aliasOf(connection)}
                      busy={busy}
                      onSave={(alias) => onAlias(connection, alias)}
                    />
                  </TableCell>
                )}
                {capability === Capability.INFERENCE && (
                  <>
                    <TableCell className="tabular-nums">
                      {dollars(spentOn(connection))}
                      {(unpricedOn(connection) ?? 0n) > 0n && (
                        <span
                          className="ml-1 text-[10px] text-muted-foreground"
                          title="Calls whose response carried no usable token usage are not priced"
                        >
                          +{String(unpricedOn(connection))} unpriced
                        </span>
                      )}
                    </TableCell>
                    <TableCell>
                      <BudgetCell
                        key={`${connection.id}:${budgetOf(connection)?.limitMicros ?? ""}`}
                        slug={connection.slug}
                        budget={budgetOf(connection)}
                        busy={busy}
                        onSave={(limit) => onBudget?.(connection, limit) ?? Promise.resolve()}
                      />
                    </TableCell>
                  </>
                )}
                <TableCell className="px-5">
                  {connection.providerId === TILDE ? (
                    <span className="flex items-center justify-end gap-1">
                      <TildeChatKey key={agentId} agentId={agentId} disabled={busy} />
                    </span>
                  ) : (
                    <span className="flex items-center justify-end gap-1">
                      {connection.status !== "ready" && connection.status !== "failed" && (
                        <IconAction
                          label="Configure"
                          disabled={busy}
                          onClick={() => onConfigure(connection)}
                        >
                          <SettingsIcon />
                        </IconAction>
                      )}
                      <RemoveButton
                        size="icon-xs"
                        label="Remove"
                        title={`Remove ${connection.name} from this agent?`}
                        description="The agent can no longer use this connection. The connection itself is kept and can be assigned again."
                        disabled={busy}
                        onConfirm={() => onRemove(connection)}
                      />
                    </span>
                  )}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}

/** Monthly cap for one connection: dollars as text, pencil swaps in an input with a save icon. */
function BudgetCell({
  slug,
  budget,
  busy,
  onSave,
}: {
  slug: string;
  budget: Budget | undefined;
  busy: boolean;
  onSave: (limitMicros: bigint | null) => Promise<void>;
}) {
  const current = budget ? (Number(budget.limitMicros) / 1_000_000).toFixed(2) : "";
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(current);
  const save = () => {
    setEditing(false);
    const trimmed = draft.trim();
    if (trimmed === current) return;
    if (trimmed === "") return void onSave(null);
    const amount = Number(trimmed);
    if (Number.isFinite(amount) && amount >= 0) void onSave(BigInt(Math.round(amount * 1_000_000)));
  };
  const exhausted = budget?.exhaustedUntil !== undefined;
  if (editing)
    return (
      <span className="flex h-7 w-40 items-center gap-1">
        <span className="text-xs text-muted-foreground">$</span>
        <Input
          autoFocus
          aria-label={`Monthly budget for ${slug}`}
          className="h-7 min-w-0 flex-1 px-1.5 text-xs tabular-nums"
          inputMode="decimal"
          value={draft}
          placeholder="no cap"
          disabled={busy}
          onChange={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") save();
            if (event.key === "Escape") {
              setDraft(current);
              setEditing(false);
            }
          }}
        />
        <IconAction label="Save budget" onClick={save} disabled={busy}>
          <CheckIcon />
        </IconAction>
      </span>
    );
  return (
    <span className="flex h-7 w-40 items-center gap-1">
      <span className={`text-xs tabular-nums ${exhausted ? "text-warning" : ""}`}>
        {budget ? `$${current} / mo` : <span className="text-muted-foreground">no cap</span>}
      </span>
      <IconAction
        label="Edit budget"
        disabled={busy}
        onClick={() => {
          setDraft(current);
          setEditing(true);
        }}
      >
        <PencilIcon />
      </IconAction>
    </span>
  );
}
