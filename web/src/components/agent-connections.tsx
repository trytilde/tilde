import { useEffect, useRef, useState } from "react";
import { Capability, type Connection, type Provider } from "@/gen/tilde/types/v1/connections_pb.js";
import { connections } from "./connections";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { useForm } from "@trytilde/connection-ui";
import { randomUUID } from "@/lib/browser-crypto";

type Fields = { name: string; provider: string; method: string; connection: string };
/** One create request allocates chat before the independent setup window completes. */
export function AgentConnections({ agentId }: { agentId: string }) {
  const [assigned, setAssigned] = useState<Connection[]>([]);
  const [available, setAvailable] = useState<Connection[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [cursor, setCursor] = useState("");
  const [availableCursor, setAvailableCursor] = useState("");
  const [providerCursor, setProviderCursor] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [mode, setMode] = useState<"existing" | "new">("existing");
  const setupWindow = useRef<{ window: Window; origin: string; connectionId: string } | null>(null);
  const creation = useRef<{ key: string; id: string } | null>(null);
  const { register, handleSubmit, watch, setValue } = useForm<Fields>({
    defaultValues: { name: "", provider: "", method: "", connection: "" },
  });
  const selected = providers.find((p) => p.id === watch("provider"));
  const assignment = { capability: Capability.CHANNEL, agentId };
  async function refresh() {
    const [assigned, available, catalog] = await Promise.all([
      connections.listConnections({ agentId, pageSize: 30 }),
      connections.listConnections({ unassignedChannelOnly: true, pageSize: 30 }),
      connections.listProviders({ pageSize: 30 }),
    ]);
    setAssigned(assigned.connections);
    setCursor(assigned.nextPageToken);
    setAvailable(available.connections);
    setAvailableCursor(available.nextPageToken);
    setProviders(catalog.providers);
    setProviderCursor(catalog.nextPageToken);
  }
  useEffect(() => {
    void refresh().catch((e) => setError(String(e)));
    const completed = (event: MessageEvent) => {
      const expected = setupWindow.current;
      if (
        expected &&
        event.source === expected.window &&
        event.origin === expected.origin &&
        event.data?.type === "tilde.connection.complete" &&
        event.data.connectionId === expected.connectionId
      ) {
        void refresh().catch((e) => setError(String(e)));
      }
    };
    window.addEventListener("message", completed);
    return () => window.removeEventListener("message", completed);
  }, [agentId]);
  async function act(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to update chat providers");
    } finally {
      setBusy(false);
    }
  }
  const submit = handleSubmit(async (fields) =>
    act(async () => {
      if (mode === "existing") {
        await connections.assignCapability({ connectionId: fields.connection, assignment });
        await refresh();
        return;
      }
      const popup = window.open(
        "about:blank",
        "tilde-connection-setup",
        "popup,width=580,height=780",
      );
      try {
        const key = JSON.stringify([agentId, fields.provider, fields.method, fields.name]);
        if (creation.current?.key !== key) creation.current = { key, id: randomUUID() };
        const result = await connections.startConnection({
          id: creation.current.id,
          name: fields.name,
          providerId: fields.provider,
          typeId: fields.method,
          assignments: [assignment],
        });
        creation.current = null;
        if (popup) {
          setupWindow.current = {
            window: popup,
            origin: new URL(result.brokeringUrl).origin,
            connectionId: result.connection?.id ?? "",
          };
          popup.location.assign(result.brokeringUrl);
        } else {
          window.location.assign(result.brokeringUrl);
        }
        await refresh();
      } catch (e) {
        popup?.close();
        throw e;
      }
    }),
  );
  return (
    <section className="space-y-4 border-t pt-5">
      <div className="flex items-center justify-between">
        <h3 className="font-medium">Chat providers</h3>
        <Button size="sm" variant="outline" disabled={busy} onClick={() => void act(refresh)}>
          Refresh
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {assigned.map((c) => (
        <div key={c.id} className="flex items-center justify-between gap-2 rounded border p-3">
          <div className="text-sm">
            <strong>{c.name}</strong>
            <p>
              {c.accountLabel ?? c.providerId} · {c.status.replaceAll("_", " ")}
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() =>
              void act(async () => {
                await connections.unassignCapability({ connectionId: c.id, assignment });
                await refresh();
              })
            }
          >
            Remove
          </Button>
        </div>
      ))}
      {!assigned.length && (
        <p className="text-sm text-muted-foreground">No chat providers assigned.</p>
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
              setAssigned([...assigned, ...page.connections]);
              setCursor(page.nextPageToken);
            })
          }
        >
          More assigned connections
        </Button>
      )}
      <div className="flex gap-2">
        <Button
          type="button"
          variant={mode === "existing" ? "default" : "outline"}
          onClick={() => setMode("existing")}
        >
          Use existing
        </Button>
        <Button
          type="button"
          variant={mode === "new" ? "default" : "outline"}
          onClick={() => setMode("new")}
        >
          Create connection
        </Button>
      </div>
      <form onSubmit={submit} className="grid gap-3">
        {mode === "existing" ? (
          <>
            <Label>Available connection</Label>
            <Select
              value={watch("connection")}
              onValueChange={(v) => setValue("connection", v ?? "")}
              items={available.map((c) => ({ value: c.id, label: `${c.name} (${c.providerId})` }))}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select a connection" />
              </SelectTrigger>
              <SelectContent>
                {available.map((c) => (
                  <SelectItem key={c.id} value={c.id}>
                    {c.name} ({c.providerId})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {availableCursor && (
              <Button
                type="button"
                variant="outline"
                onClick={() =>
                  void act(async () => {
                    const p = await connections.listConnections({
                      unassignedChannelOnly: true,
                      pageToken: availableCursor,
                      pageSize: 30,
                    });
                    setAvailable([...available, ...p.connections]);
                    setAvailableCursor(p.nextPageToken);
                  })
                }
              >
                More available connections
              </Button>
            )}
          </>
        ) : (
          <>
            <Label>Provider</Label>
            <Select
              value={watch("provider")}
              onValueChange={(v) => {
                setValue("provider", v ?? "");
                setValue("method", "");
              }}
              items={providers
                .filter((p) =>
                  p.connectionTypes.some((t) => t.capabilities.includes(Capability.CHANNEL)),
                )
                .map((p) => ({ value: p.id, label: p.name }))}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select provider" />
              </SelectTrigger>
              <SelectContent>
                {providers
                  .filter((p) =>
                    p.connectionTypes.some((t) => t.capabilities.includes(Capability.CHANNEL)),
                  )
                  .map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
            {providerCursor && (
              <Button
                type="button"
                variant="outline"
                onClick={() =>
                  void act(async () => {
                    const p = await connections.listProviders({
                      pageToken: providerCursor,
                      pageSize: 30,
                    });
                    setProviders([...providers, ...p.providers]);
                    setProviderCursor(p.nextPageToken);
                  })
                }
              >
                More providers
              </Button>
            )}
            <Label>How to connect</Label>
            <Select
              value={watch("method")}
              onValueChange={(v) => setValue("method", v ?? "")}
              items={
                selected?.connectionTypes
                  .filter((t) => t.capabilities.includes(Capability.CHANNEL))
                  .map((t) => ({ value: t.id, label: t.name })) ?? []
              }
            >
              <SelectTrigger>
                <SelectValue placeholder="Select method" />
              </SelectTrigger>
              <SelectContent>
                {selected?.connectionTypes
                  .filter((t) => t.capabilities.includes(Capability.CHANNEL))
                  .map((t) => (
                    <SelectItem key={t.id} value={t.id}>
                      {t.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
            <Label htmlFor="connection-name">Connection name</Label>
            <Input id="connection-name" {...register("name")} required maxLength={200} />
          </>
        )}
        <Button disabled={busy || (mode === "existing" ? !watch("connection") : !watch("method"))}>
          {busy ? "Saving…" : mode === "existing" ? "Add chat provider" : "Create and configure"}
        </Button>
      </form>
    </section>
  );
}
