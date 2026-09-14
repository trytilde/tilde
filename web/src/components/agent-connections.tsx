import { LoadingReveal } from "@trytilde/connection-ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { CableIcon, PlusIcon } from "lucide-react";
import {
  Capability,
  type Connection,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { connections } from "@/client";
import { Button } from "./ui/button";
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

type Setup = { title: string; url: string; connectionId: string };
const channelMethods = (provider: Provider) =>
  provider.connectionTypes.filter((method) => method.capabilities.includes(Capability.CHANNEL));
const message = (error: unknown) =>
  error instanceof Error ? error.message : "Unable to update chat providers.";

/** Catalog-driven menus allocate chat in the create request, before credentials are brokered. */
export function AgentConnections({ agentId }: { agentId: string }) {
  const [assigned, setAssigned] = useState<Connection[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [cursor, setCursor] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [existing, setExisting] = useState<Provider | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [setupLoaded, setSetupLoaded] = useState(false);
  const [setupError, setSetupError] = useState("");
  const [retry, setRetry] = useState<(() => Promise<void>) | null>(null);
  const creation = useRef<{ key: string; id: string } | null>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const setupAttempt = useRef(0);
  const assignment = { capability: Capability.CHANNEL, agentId };
  const refresh = useCallback(async () => {
    const page = await connections.listConnections({ agentId, pageSize: 30 });
    setAssigned(page.connections);
    setCursor(page.nextPageToken);
  }, [agentId]);

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
            { pageSize: 100, pageToken },
            { signal: abort.signal },
          );
          catalog.push(...page.providers.filter((provider) => channelMethods(provider).length));
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
  }, [refresh]);

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
    <section className="space-y-5" aria-label="Chat providers">
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {loading && (
        <p role="status" className="text-sm text-muted-foreground">
          Loading chat providers…
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        {providers.map((provider) => {
          const methods = channelMethods(provider);
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
      {!loading && !providers.length && !error && (
        <p className="text-sm text-muted-foreground">No chat providers available.</p>
      )}
      <div className="space-y-3">
        {assigned.map((connection) => (
          <div
            key={connection.id}
            className="flex flex-wrap items-center justify-between gap-3 rounded-xl border p-4"
          >
            <div className="flex min-w-0 items-center gap-3">
              <ProviderIcon
                iconUrl={
                  providers.find((provider) => provider.id === connection.providerId)?.iconUrl
                }
              />
              <div className="min-w-0 text-sm">
                <strong className="block truncate" title={connection.name}>
                  {connection.name}
                </strong>
                <p className="truncate text-muted-foreground">
                  {providers.find((provider) => provider.id === connection.providerId)?.name ??
                    connection.providerId}
                </p>
              </div>
              <ConnectionStatusBadge status={connection.status} />
            </div>
            <div className="flex gap-2">
              {connection.status !== "ready" && connection.status !== "failed" && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void broker(`Configure ${connection.name}`, () =>
                      connections.reconnect({ id: connection.id }),
                    )
                  }
                >
                  Configure
                </Button>
              )}
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() =>
                  void act(async () => {
                    await connections.unassignCapability({
                      connectionId: connection.id,
                      assignment,
                    });
                    await refresh();
                  })
                }
              >
                Remove
              </Button>
            </div>
          </div>
        ))}
        {!assigned.length && !loading && (
          <div className="flex min-h-48 w-full items-center justify-center rounded-xl border border-dashed bg-card p-6 text-center text-sm text-muted-foreground">
            No chat providers assigned
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
            <DialogDescription>
              Choose a connection whose chat capability is available.
            </DialogDescription>
          </DialogHeader>
          {existing && (
            <ExistingConnections
              key={existing.id}
              provider={existing}
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

/** The API paginates across providers; load every page before declaring none available. */
function ExistingConnections({
  provider,
  busy,
  onSelect,
}: {
  provider: Provider;
  busy: boolean;
  onSelect: (connection: Connection) => void;
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
            { unassignedChannelOnly: true, pageSize: 100, pageToken },
            { signal: abort.signal },
          );
          matches.push(
            ...page.connections.filter((connection) => connection.providerId === provider.id),
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
  }, [provider.id, attempt]);
  return (
    <div className="max-h-[50dvh] space-y-2 overflow-y-auto">
      {loading ? (
        <p role="status">Loading connections…</p>
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
function ConnectionStatusBadge({ status }: { status: string }) {
  const presentation = connectionStatuses[status] ?? {
    label: status.replaceAll("_", " "),
    color: "border-border bg-muted text-muted-foreground",
  };
  return (
    <Badge variant="outline" className={presentation.color}>
      {presentation.label}
    </Badge>
  );
}
