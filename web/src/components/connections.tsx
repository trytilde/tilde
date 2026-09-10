import { randomUUID } from "@/lib/browser-crypto";
import { authInterceptor } from "@/auth";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ConnectionsService } from "@/gen/tilde/management/v1/connections_pb.js";
import {
  ClientAuthentication,
  Capability,
  type Provider,
  type Connection,
} from "@/gen/tilde/types/v1/connections_pb.js";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useForm, Textarea } from "@trytilde/connection-ui";

export const connections = createClient(
  ConnectionsService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
const message = (error: unknown) => (error instanceof Error ? error.message : "Request failed");
const labelClass = "grid gap-2 text-sm";
const selectClass = "h-9 rounded-md border bg-background px-3 text-sm";

export function ConnectionsPage() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [items, setItems] = useState<Connection[]>([]);
  const [providerCursor, setProviderCursor] = useState("");
  const [connectionCursor, setConnectionCursor] = useState("");
  const [selected, setSelected] = useState("");
  const [typeId, setTypeId] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [custom, setCustom] = useState(false);
  const setupWindow = useRef<{ window: Window; connectionId: string; origin: string } | null>(null);
  useEffect(() => {
    const completed = (event: MessageEvent) => {
      if (
        event.origin === setupWindow.current?.origin &&
        event.source === setupWindow.current?.window &&
        event.data?.type === "tilde.connection.complete" &&
        event.data.connectionId === setupWindow.current.connectionId
      )
        void refresh().catch((error) => setError(message(error)));
    };
    window.addEventListener("message", completed);
    return () => window.removeEventListener("message", completed);
  }, []);
  async function openSetup(
    start: () => Promise<{ brokeringUrl: string; connection?: Connection }>,
  ) {
    const popup = window.open(
      "about:blank",
      "tilde-connection-setup",
      "popup,width=580,height=780",
    );
    try {
      const response = await start();
      if (popup && !popup.closed) {
        setupWindow.current = {
          window: popup,
          connectionId: response.connection?.id ?? "",
          origin: new URL(response.brokeringUrl).origin,
        };
        popup.location.assign(response.brokeringUrl);
        await refresh();
      } else {
        window.location.assign(response.brokeringUrl);
      }
    } catch (error) {
      popup?.close();
      throw error;
    }
  }

  async function refresh() {
    const [catalog, list] = await Promise.all([
      connections.listProviders({ pageSize: 30 }),
      connections.listConnections({ pageSize: 30 }),
    ]);
    setProviders(catalog.providers);
    setProviderCursor(catalog.nextPageToken);
    setItems(list.connections);
    setConnectionCursor(list.nextPageToken);
  }
  useEffect(() => {
    void refresh().catch((error) => setError(message(error)));
  }, []);
  async function act(fn: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  }
  async function start(event: FormEvent) {
    event.preventDefault();
    await act(async () => {
      await openSetup(() =>
        connections.startConnection({
          id: randomUUID(),
          name,
          providerId: selected,
          typeId,
        }),
      );
    });
  }
  const provider = providers.find((provider) => provider.id === selected);
  return (
    <section className="mx-auto w-full max-w-5xl space-y-8 p-6 lg:p-10">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-2xl font-semibold">Connections</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            Connect your accounts and apps. Credentials stay private.
          </p>
        </div>
        <Button variant="outline" onClick={() => setCustom(!custom)}>
          Add custom provider
        </Button>
      </div>
      {error && (
        <p
          role="alert"
          className="rounded-lg border border-destructive/30 p-3 text-sm text-destructive"
        >
          {error}
        </p>
      )}
      {custom && (
        <CustomProvider
          onRegistered={async () => {
            setCustom(false);
            await refresh();
          }}
        />
      )}
      <form className="grid gap-4 rounded-xl border p-5 md:grid-cols-3" onSubmit={start}>
        <label className={labelClass}>
          Provider
          <select
            required
            className={selectClass}
            value={selected}
            onChange={(event) => {
              const p = providers.find((p) => p.id === event.target.value);
              setSelected(event.target.value);
              setTypeId(p?.connectionTypes.length === 1 ? p.connectionTypes[0].id : "");
            }}
          >
            <option value="">Select provider</option>
            {providers.map((provider) => (
              <option key={provider.id} value={provider.id}>
                {provider.name}
              </option>
            ))}
          </select>
        </label>
        <label className={labelClass}>
          How to connect
          <select
            required
            className={selectClass}
            value={typeId}
            onChange={(event) => setTypeId(event.target.value)}
          >
            <option value="">Select type</option>
            {provider?.connectionTypes.map((type) => (
              <option key={type.id} value={type.id}>
                {type.name}
                {type.capabilities.includes(Capability.CHANNEL) ? " · Channel" : ""}
              </option>
            ))}
          </select>
        </label>
        <label className={labelClass}>
          Name
          <Input
            required
            maxLength={200}
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="My account"
          />
        </label>
        <div className="md:col-span-3">
          <Button disabled={busy || !selected || !typeId}>
            {busy ? "Starting…" : "Connect account"}
          </Button>
        </div>
      </form>
      {providerCursor && (
        <Button
          variant="outline"
          disabled={busy}
          onClick={() =>
            void act(async () => {
              const page = await connections.listProviders({
                pageToken: providerCursor,
                pageSize: 30,
              });
              setProviders((previous) => [...previous, ...page.providers]);
              setProviderCursor(page.nextPageToken);
            })
          }
        >
          Load more providers
        </Button>
      )}
      <div className="space-y-3">
        {items.length === 0 && <p className="text-sm text-muted-foreground">No connections yet.</p>}
        {items.map((connection) => (
          <article
            key={connection.id}
            className="flex flex-wrap items-center justify-between gap-4 rounded-xl border p-4"
          >
            <div>
              <h3 className="font-medium">{connection.name}</h3>
              {connection.webhookUrl && (
                <code className="block max-w-lg break-all text-xs text-muted-foreground">
                  {connection.webhookUrl}
                </code>
              )}
              {connection.associatedAgents.map((a) => (
                <p key={`${a.capability}:${a.id}`} className="text-sm">
                  {a.name} · Chat
                </p>
              ))}
              <p className="text-xs text-muted-foreground">
                {connection.providerId} · {connection.typeId} ·{" "}
                {connection.status.replaceAll("_", " ")}
                {connection.capabilities.includes(Capability.CHANNEL) ? " · Channel" : ""}
              </p>
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
                disabled={busy}
                onClick={() =>
                  void act(async () => {
                    await openSetup(() => connections.reconnect({ id: connection.id }));
                  })
                }
              >
                {connection.status === "requires_action" ? "Continue setup" : "Reconnect"}
              </Button>
              {connection.status !== "disconnected" && (
                <Button
                  variant="destructive"
                  disabled={busy}
                  onClick={() => {
                    if (
                      window.confirm(
                        `Disconnect ${connection.name}? Tilde will discard its credentials. Your provider account or app will remain.`,
                      )
                    )
                      void act(async () => {
                        await connections.disconnect({ id: connection.id });
                        await refresh();
                      });
                  }}
                >
                  Disconnect
                </Button>
              )}
            </div>
          </article>
        ))}
      </div>
      {connectionCursor && (
        <Button
          variant="outline"
          disabled={busy}
          onClick={() =>
            void act(async () => {
              const page = await connections.listConnections({
                pageToken: connectionCursor,
                pageSize: 30,
              });
              setItems((previous) => [...previous, ...page.connections]);
              setConnectionCursor(page.nextPageToken);
            })
          }
        >
          Load more connections
        </Button>
      )}
    </section>
  );
}

const defaultSchema = JSON.stringify(
  {
    type: "object",
    additionalProperties: false,
    properties: { api_key: { type: "string", title: "API key", writeOnly: true, minLength: 1 } },
    required: ["api_key"],
  },
  null,
  2,
);
type Registration = {
  id: string;
  name: string;
  categories: string;
  source: "static" | "oauth" | "custom";
  grant: string;
  schema: string;
  extra: string;
  authorization: string;
  token: string;
  scopes: string;
  auth: string;
  remote: boolean;
  endpoint: string;
  uiUrl: string;
  backendToken: string;
};
function CustomProvider({ onRegistered }: { onRegistered: () => Promise<void> }) {
  const form = useForm<Registration>({
    shouldUnregister: true,
    defaultValues: {
      id: "",
      name: "",
      categories: "other",
      source: "static",
      grant: "1",
      schema: defaultSchema,
      auth: "body",
      remote: false,
    },
  });
  const source = form.watch("source");
  const grant = form.watch("grant");
  const remote = form.watch("remote");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(data: Registration) {
    setBusy(true);
    setError("");
    try {
      if (source === "static") JSON.parse(data.schema);
      if (data.extra) JSON.parse(data.extra);
      await connections.registerProvider({
        backendToken: data.remote && data.backendToken ? data.backendToken : undefined,
        provider: {
          id: `custom/${data.id}`,
          name: data.name,
          categories: data.categories
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
          kind: data.remote
            ? { case: "remote", value: { endpoint: data.endpoint, uiUrl: data.uiUrl || undefined } }
            : { case: "configured", value: {} },
          connectionTypes: [
            {
              id: "account",
              name: "Account",
              credentialSource:
                data.source === "static"
                  ? { case: "static", value: { schemaJson: data.schema } }
                  : data.source === "custom"
                    ? { case: "custom", value: {} }
                    : {
                        case: "oauth",
                        value: {
                          grant: Number(data.grant),
                          configuration: {
                            authorizationUrl: data.grant === "1" ? data.authorization : undefined,
                            tokenUrl: data.token,
                            clientAuthentication:
                              data.auth === "none"
                                ? ClientAuthentication.NONE
                                : data.auth === "basic"
                                  ? ClientAuthentication.BASIC
                                  : ClientAuthentication.BODY,
                            pkce: true,
                            scopes: (data.scopes ?? "").split(/[ ,]+/).filter(Boolean),
                            scopeSeparator: " ",
                          },
                          additionalSchemaJson: data.extra || undefined,
                        },
                      },
            },
          ],
        },
      });
      form.reset();
      await onRegistered();
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form className="space-y-4 rounded-xl border p-5" onSubmit={form.handleSubmit(submit)}>
      <h3 className="font-medium">Register provider</h3>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      <div className="grid gap-4 md:grid-cols-3">
        <label className={labelClass}>
          Provider ID
          <Input required pattern="[A-Za-z0-9_.-]+" {...form.register("id", { required: true })} />
        </label>
        <label className={labelClass}>
          Name
          <Input required {...form.register("name", { required: true })} />
        </label>
        <label className={labelClass}>
          Categories
          <Input
            required
            placeholder="email, chat"
            {...form.register("categories", { required: true })}
          />
        </label>
      </div>
      <label className={labelClass}>
        Credential source
        <select
          className={selectClass}
          {...form.register("source")}
          onChange={(event) => {
            form.setValue("source", event.target.value as Registration["source"]);
            if (event.target.value === "custom") form.setValue("remote", true);
          }}
        >
          <option value="static">Static credentials</option>
          <option value="oauth">OAuth</option>
          <option value="custom">Custom setup</option>
        </select>
      </label>
      {source === "static" && (
        <label className={labelClass}>
          Credential JSON Schema
          <Textarea
            required
            className="min-h-48 font-mono"
            {...form.register("schema", { required: true })}
          />
        </label>
      )}
      {source === "oauth" && (
        <div className="grid gap-4 md:grid-cols-2">
          <label className={labelClass}>
            Grant
            <select className={selectClass} {...form.register("grant")}>
              <option value="1">Authorization code</option>
              <option value="2">Client credentials</option>
              <option value="3">JWT bearer</option>
            </select>
          </label>
          {grant === "1" && (
            <label className={labelClass}>
              Authorization URL
              <Input type="url" required {...form.register("authorization", { required: true })} />
            </label>
          )}
          <label className={labelClass}>
            Token URL
            <Input type="url" required {...form.register("token", { required: true })} />
          </label>
          <label className={labelClass}>
            Scopes
            <Input {...form.register("scopes")} />
          </label>
          {grant !== "3" && (
            <label className={labelClass}>
              Client authentication
              <select className={selectClass} {...form.register("auth")}>
                <option value="body">Request body</option>
                <option value="basic">HTTP Basic</option>
                {grant === "1" && <option value="none">Public client (PKCE)</option>}
              </select>
            </label>
          )}
          <label className={labelClass}>
            Additional credential schema (optional)
            <Textarea
              placeholder="Extra static fields, such as a signing key"
              {...form.register("extra")}
            />
          </label>
        </div>
      )}
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          {...form.register("remote")}
          onChange={(event) => form.setValue("remote", source === "custom" || event.target.checked)}
          checked={remote}
        />
        Remotely hosted provider
      </label>
      {remote && (
        <div className="grid gap-4">
          <label className={labelClass}>
            Backend URL
            <Input required type="url" {...form.register("endpoint", { required: true })} />
          </label>
          {source === "custom" && (
            <>
              <label className={labelClass}>
                UI distribution URL
                <Input type="url" required {...form.register("uiUrl", { required: true })} />
              </label>
              <label className={labelClass}>
                Backend token
                <Input
                  type="password"
                  required
                  minLength={32}
                  {...form.register("backendToken", { required: true })}
                />
              </label>
            </>
          )}
        </div>
      )}
      <Button disabled={busy} type="submit">
        {busy ? "Registering…" : "Register provider"}
      </Button>
    </form>
  );
}
export { BrokeringPage } from "@/connection-setup-host";
