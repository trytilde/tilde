import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { CopyIcon, RefreshCwIcon, ZapIcon } from "lucide-react";
import type { Connection, Provider } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ToolHostType, type ToolHost } from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { connections, toolHosts, tools as toolsClient } from "@/client";
import { usePageCrumb } from "./dashboard-breadcrumbs";
import { ProviderIcon } from "./provider-icon";
import { ConnectionSetupDialog, type Brokering } from "./connection-setup-dialog";
import { AccountSetupDialog } from "./tool-account-setup";
import { loadToolConnections, message, statusLabel } from "./tool-connections";
import { ToolTable } from "./tool-catalog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type Kind = "connection" | "host";
type Source =
  | { kind: "connection"; connection: Connection; provider?: Provider }
  // A host that publishes a provider is used through its instances (that provider's connections).
  | { kind: "host"; host: ToolHost; provider?: Provider; instances: Connection[] };

/**
 * One tool connection or tool host and the tools it offers. The agents using it are on its row of the Connections (or Remote servers) table; which of its tools an agent uses
 * is configured on the agent's Tools tab.
 */
export function ToolConnectionDetail({ id, kind }: { id: string; kind: Kind }) {
  const navigate = useNavigate();
  const [source, setSource] = useState<Source | null>(null);
  // Bumped by every refresh so the tool table reloads what it lists.
  const [version, setVersion] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [token, setToken] = useState("");
  // Deleting (or disconnecting) and rotating the token are confirmed first.
  const [confirm, setConfirm] = useState<"delete" | "rotate" | null>(null);
  const [setup, setSetup] = useState<Brokering | null>(null);
  // The catalog's setup form: reconnecting this account, or adding an instance of a host.
  const [form, setForm] = useState<"reconnect" | "instance" | null>(null);

  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const list =
        kind === "connection"
          ? await connections.getConnection({ id }, { signal })
          : await toolHosts.listToolHosts({}, { signal });
      if (kind === "connection") {
        const { connection } = list as { connection?: Connection };
        if (!connection) throw new Error("Connection not found.");
        const { provider } = await connections
          .getProvider({ id: connection.providerId }, { signal })
          .catch(() => ({ provider: undefined }));
        setSource({ kind, connection, provider });
      } else {
        const host = (list as { toolHosts: ToolHost[] }).toolHosts.find((h) => h.id === id);
        if (!host) throw new Error("Tool host not found.");
        const [provider, instances] = host.providerId
          ? await Promise.all([
              connections.getProvider({ id: host.providerId }, { signal }),
              loadToolConnections({ providerId: host.providerId }, signal),
            ])
          : [undefined, []];
        setSource({ kind, host, provider: provider?.provider, instances });
      }
      setVersion((current) => current + 1);
    },
    [id, kind],
  );
  useEffect(() => {
    const abort = new AbortController();
    setLoading(true);
    void refresh(abort.signal)
      .catch((e) => {
        if (!abort.signal.aborted) setError(message(e));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [refresh]);

  async function act(work: () => Promise<void>) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await work();
      await refresh();
      return true;
    } catch (e) {
      setError(message(e));
      return false;
    } finally {
      setBusy(false);
    }
  }
  const name = source?.kind === "connection" ? source.connection.name : source?.host.name;
  const parent =
    kind === "connection"
      ? ({ label: "Connections", to: "/tools/connections" } as const)
      : ({ label: "Remote servers", to: "/tools/remote-servers" } as const);
  usePageCrumb(`/tools/${id}`, name, parent);
  async function remove() {
    setBusy(true);
    setError("");
    try {
      if (kind === "connection") await connections.disconnect({ id });
      else await toolHosts.deleteToolHost({ id });
      setConfirm(null);
      void navigate({ to: parent.to });
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-6 p-4 lg:p-6">
      {error && !confirm && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="text-sm text-muted-foreground">
          {notice}
        </p>
      )}
      {loading && !source ? (
        <p role="status" className="text-sm text-muted-foreground">
          Loading…
        </p>
      ) : source ? (
        <>
          <header className="flex flex-wrap items-start justify-between gap-4">
            <div className="flex items-center gap-3">
              {source.kind === "host" ? (
                <ZapIcon aria-hidden="true" className="size-6 text-muted-foreground" />
              ) : (
                <ProviderIcon iconUrl={source.provider?.iconUrl} />
              )}
              <div>
                <h1 className="m-0 text-lg font-semibold">{name}</h1>
                <p className="m-0 text-sm text-muted-foreground">
                  {source.kind === "connection"
                    ? `${source.provider?.name ?? source.connection.providerId} · ${statusLabel(source.connection.status)}`
                    : `${source.host.type === ToolHostType.LAMBDA ? `Lambda · ${source.host.functionArn}` : "Connected host"} · ${source.host.available ? "Available" : "Offline"}`}
                </p>
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              {source.kind === "connection" && (
                <>
                  {source.provider?.connectionTypes.find(
                    (type) => type.id === source.connection.typeId,
                  )?.mcpServer &&
                    source.connection.status === "ready" && (
                      <Button
                        variant="outline"
                        disabled={busy}
                        onClick={() =>
                          void act(async () => {
                            const result = await toolsClient.refreshConnectionTools({
                              connectionId: id,
                            });
                            setNotice(
                              result.changed
                                ? `Tools updated: ${result.tools.length} available.`
                                : "The server describes the same tools as before.",
                            );
                          })
                        }
                      >
                        <RefreshCwIcon />
                        Refresh tools
                      </Button>
                    )}
                  <Button
                    variant="outline"
                    disabled={busy || !source.provider}
                    onClick={() => setForm("reconnect")}
                  >
                    {source.connection.status === "ready" ? "Reconnect" : "Finish setup"}
                  </Button>
                </>
              )}
              {source.kind === "host" && source.host.type === ToolHostType.LAMBDA && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void act(async () => {
                      await toolHosts.refreshToolHost({ id });
                    })
                  }
                >
                  <RefreshCwIcon />
                  Refresh tools
                </Button>
              )}
              {source.kind === "host" && source.host.type === ToolHostType.CONNECTED && (
                <Button variant="outline" disabled={busy} onClick={() => setConfirm("rotate")}>
                  Rotate token
                </Button>
              )}
              <Button variant="destructive" disabled={busy} onClick={() => setConfirm("delete")}>
                {source.kind === "connection" ? "Disconnect" : "Delete"}
              </Button>
            </div>
          </header>

          {source.kind === "host" && source.host.providerId ? (
            <section className="space-y-3" aria-label="Instances">
              <h2 className="m-0 text-base font-semibold">Instances</h2>
              <p className="m-0 text-sm text-muted-foreground">
                This tool needs credentials. Each instance is set up with its own, which the host
                receives with every call made on it. Agents use the tools through an instance.
              </p>
              {source.provider && (
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    disabled={busy || !source.host.available}
                    onClick={() => setForm("instance")}
                  >
                    Add instance
                  </Button>
                  {!source.host.available && (
                    <span className="self-center text-xs text-muted-foreground">
                      The host must be running to verify an instance&apos;s credentials.
                    </span>
                  )}
                </div>
              )}
              {source.instances.length ? (
                <ul className="m-0 divide-y rounded-lg border p-0">
                  {source.instances.map((instance) => (
                    <li key={instance.id} className="flex items-center gap-4 px-4 py-2 text-sm">
                      <Link
                        to="/tools/$toolId"
                        params={{ toolId: instance.id }}
                        search={{ kind: "connection" }}
                        className="min-w-0 flex-1 truncate font-medium hover:underline"
                      >
                        {instance.name}
                      </Link>
                      {instance.accountLabel && (
                        <span className="text-muted-foreground">{instance.accountLabel}</span>
                      )}
                      <span className="text-muted-foreground">{statusLabel(instance.status)}</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground">No instances yet.</p>
              )}
              <h3 className="m-0 text-sm font-semibold">Tools each instance offers</h3>
              <ToolTable
                of={{ toolHostId: id }}
                version={version}
                empty={
                  <p className="text-sm text-muted-foreground">
                    The host has not published any tools yet.
                  </p>
                }
              />
            </section>
          ) : (
            <>
              <section className="space-y-3" aria-label="Tools">
                <h2 className="m-0 text-base font-semibold">Tools</h2>
                {/* Tools are readable only once credentials exist; a pending setup has none yet. */}
                {source.kind === "connection" && source.connection.status !== "ready" ? (
                  <p className="text-sm text-muted-foreground">
                    Finish setup to see this connection's tools.
                  </p>
                ) : (
                  <ToolTable
                    of={source.kind === "connection" ? { connectionId: id } : { toolHostId: id }}
                    version={version}
                    empty={
                      <p className="text-sm text-muted-foreground">
                        {source.kind === "host"
                          ? "The host has not published any tools yet."
                          : "No tools available."}
                      </p>
                    }
                  />
                )}
              </section>
            </>
          )}
        </>
      ) : null}

      <Dialog open={!!token} onOpenChange={(open) => !open && setToken("")}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>New tool host token</DialogTitle>
            <DialogDescription>
              Shown once. Hosts using the old token lose access at their next call.
            </DialogDescription>
          </DialogHeader>
          <div className="flex gap-2">
            <Input readOnly value={token} aria-label="Tool host token" />
            <Button variant="outline" onClick={() => void navigator.clipboard.writeText(token)}>
              <CopyIcon />
              Copy
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <AlertDialog open={!!confirm} onOpenChange={(open) => !busy && !open && setConfirm(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirm === "rotate"
                ? `Rotate ${name}'s token?`
                : kind === "connection"
                  ? `Disconnect ${name}?`
                  : `Delete ${name}?`}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirm === "rotate"
                ? "The current token stops working: the host loses access at its next call until it is restarted with the new token."
                : kind === "connection"
                  ? "Its credentials are removed and its tools disappear from every agent using it."
                  : "Its tools disappear from every agent using it and its token stops working."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {error && (
            <p role="alert" className="m-0 text-sm text-destructive">
              {error}
            </p>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() =>
                confirm === "rotate"
                  ? void act(async () => {
                      setToken((await toolHosts.rotateToolHostToken({ id })).token);
                    }).then((done) => done && setConfirm(null))
                  : void remove()
              }
            >
              {confirm === "rotate"
                ? "Rotate token"
                : kind === "connection"
                  ? "Disconnect"
                  : "Delete"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      {form && source?.provider ? (
        <AccountSetupDialog
          provider={source.provider}
          existing={
            form === "reconnect" && source.kind === "connection"
              ? {
                  id: source.connection.id,
                  typeId: source.connection.typeId,
                  name: source.connection.name,
                }
              : undefined
          }
          iconId={source.provider.id}
          onStarted={() => void refresh().catch((e) => setError(message(e)))}
          onComplete={() => {
            setForm(null);
            void refresh().catch((e) => setError(message(e)));
          }}
          // A provider-owned method continues in its hosted setup page, as in the catalog.
          onCustom={(frame) => {
            setForm(null);
            setSetup(frame);
          }}
          onClose={() => {
            setForm(null);
            void refresh().catch((e) => setError(message(e)));
          }}
        />
      ) : null}
      <ConnectionSetupDialog
        setup={setup}
        error=""
        onClose={() => {
          setSetup(null);
          void refresh().catch((e) => setError(message(e)));
        }}
      />
    </div>
  );
}
