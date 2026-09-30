import { useEffect, useMemo, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { ExternalLinkIcon, KeyRoundIcon, ShieldCheckIcon, UserRoundIcon } from "lucide-react";
import {
  ClientAuthentication,
  OAuthClient,
  OAuthGrant,
  type ConnectionType,
  type Provider,
} from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { AuthDriver, type Brokering } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";
import { connectionSetup, connections } from "@/client";
import { randomUUID } from "@/lib/browser-crypto";
import { cn } from "@/lib/utils";
import {
  CatalogIcon,
  dialogButton,
  dialogChrome,
  dialogTitle,
  message,
  setupTitle,
  toolMethods,
} from "./tool-catalog";
import type { Brokering as FrameSetup } from "./connection-setup-dialog";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

type Property = {
  type?: string;
  title?: string;
  description?: string;
  writeOnly?: boolean;
  enum?: Array<string | number | boolean>;
  default?: string | number | boolean;
  contentMediaType?: string;
};
type Schema = { properties?: Record<string, Property>; required?: string[] };
export type CredentialField = {
  key: string;
  label: string;
  description?: string;
  required: boolean;
  secret: boolean;
  multiline: boolean;
  choices?: string[];
  initial: string;
};

/**
 * The credential inputs of a standard (static or OAuth) connection type, derived as the server
 * derives the setup's input schema. Undefined for custom types, whose provider owns the UI.
 */
export function credentialFields(type: ConnectionType): CredentialField[] | undefined {
  const source = type.credentialSource;
  let schema: Schema;
  if (source.case === "static") schema = JSON.parse(source.value.schemaJson || "{}") as Schema;
  else if (source.case === "oauth") {
    const extra = source.value.additionalSchemaJson
      ? (JSON.parse(source.value.additionalSchemaJson) as Schema)
      : {};
    const client = source.value.configuration?.client ?? OAuthClient.UNSPECIFIED;
    let base: Schema = { properties: {}, required: [] };
    if (source.value.grant === OAuthGrant.JWT_BEARER)
      base = {
        properties: {
          issuer: { type: "string", title: "Service account issuer" },
          private_key: {
            type: "string",
            title: "Private key (PEM)",
            writeOnly: true,
            contentMediaType: "application/x-pem-file",
          },
          subject: { type: "string", title: "Subject (optional)" },
        },
        required: ["issuer", "private_key"],
      };
    // A client registered at setup needs nothing from the person setting up.
    else if (
      client !== OAuthClient.DYNAMIC
    ) {
      base = {
        properties: { client_id: { type: "string", title: "Client ID" } },
        required: ["client_id"],
      };
      if (source.value.configuration?.clientAuthentication !== ClientAuthentication.NONE) {
        base.properties!.client_secret = {
          type: "string",
          title: "Client secret",
          writeOnly: true,
        };
        base.required!.push("client_secret");
      }
    }
    schema = {
      properties: { ...base.properties, ...extra.properties },
      required: [...(base.required ?? []), ...(extra.required ?? [])],
    };
  } else return undefined;
  const required = new Set(schema.required ?? []);
  // Schemas arrive with their keys sorted; required fields lead so the essential ones come first.
  const entries = Object.entries(schema.properties ?? {}).sort(
    ([left], [right]) => Number(required.has(right)) - Number(required.has(left)),
  );
  return entries.map(([key, property]) => ({
    key,
    label: property.title ?? key,
    description: property.description,
    required: required.has(key),
    secret: !!property.writeOnly || /secret|token|password|api_key/i.test(key),
    multiline: !!property.contentMediaType,
    choices:
      property.enum?.map(String) ?? (property.type === "boolean" ? ["true", "false"] : undefined),
    initial: property.default === undefined ? "" : String(property.default),
  }));
}
/** The authorization-code grant continues in the browser. */
function browserSignIn(type?: ConnectionType) {
  return (
    type?.credentialSource.case === "oauth" &&
    type.credentialSource.value.grant === OAuthGrant.AUTHORIZATION_CODE
  );
}

type Values = { typeId: string; name: string; fields: Record<string, string> };
type Started = {
  setupId: string;
  token: string;
  connectionId: string;
  url: string;
  typeId: string;
};

const input =
  "h-9 rounded-lg border-[0.5px] border-foreground/15 bg-muted px-3 text-[12.5px] text-foreground shadow-[inset_0_1px_2px_rgb(0_0_0/0.04)] outline-none focus:border-ring";
const label = "flex items-center gap-1.5 text-xs font-medium text-foreground/60";
const helper = "text-[11px] leading-4 text-foreground/40";

/**
 * Adds an account with dispatch's native credential form. The connection is started on the
 * first submit; later submits (after a refused key, for example) go to that same setup. With
 * `existing`, the same form reconnects that account: its method is fixed and its setup restarts.
 */
export function AccountSetupDialog({
  provider,
  initialTypeId,
  existing,
  iconId,
  onStarted,
  onComplete,
  onCustom,
  onClose,
}: {
  provider: Provider;
  /** The auth method chosen before the dialog opened. */
  initialTypeId?: string;
  /** An account to reconnect rather than a new one. */
  existing?: { id: string; typeId: string; name: string };
  iconId: string;
  onStarted: () => void;
  onComplete: (connectionId: string, name: string) => void;
  /** A provider-owned method continues in its hosted setup page. */
  onCustom: (setup: FrameSetup, connectionId: string) => void;
  onClose: () => void;
}) {
  const methods = toolMethods(provider);
  const form = useForm<Values>({
    defaultValues: {
      typeId:
        methods.find((m) => m.id === (existing?.typeId ?? initialTypeId))?.id ??
        methods[0]?.id ??
        "",
      name: existing?.name ?? "",
      fields: {},
    },
  });
  const typeId = form.watch("typeId");
  const method = methods.find((candidate) => candidate.id === typeId) ?? methods[0];
  const fields = useMemo(() => (method ? (credentialFields(method) ?? []) : []), [method]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const [authorizationUrl, setAuthorizationUrl] = useState("");
  const started = useRef<Started | null>(null);
  const completed = useRef(false);
  const browser = browserSignIn(method);

  useEffect(() => {
    for (const field of fields) form.setValue(`fields.${field.key}`, field.initial);
  }, [fields, form]);

  const values = form.watch();
  const missingRequired =
    !method ||
    !values.name?.trim() ||
    fields.some((field) => field.required && !values.fields?.[field.key]?.trim());

  const request = () => ({
    setupId: started.current!.setupId,
    connectionSetupToken: started.current!.token,
  });
  function finish() {
    if (completed.current) return;
    completed.current = true;
    onComplete(started.current!.connectionId, form.getValues("name").trim());
  }
  /** Moves the dialog to what the setup asks for next. */
  function follow(state?: Brokering) {
    switch (state?.action.case) {
      case "complete":
        finish();
        break;
      case "redirect":
        setAuthorizationUrl(state.action.value.url);
        break;
      case "working":
        setAuthorizationUrl((current) => current || "about:blank");
        break;
      case "formPost":
        onCustom(
          { title: setupTitle(provider.name), url: started.current!.url },
          started.current!.connectionId,
        );
        break;
      case "failed":
      case "cancelled":
        started.current = null;
        setAuthorizationUrl("");
        setError(
          `Setup ${state.action.case}${state.errorCode ? ` (${state.errorCode})` : ""}. Try again.`,
        );
        break;
      case "form":
        // Back at the form: the provider refused what was sent.
        if (authorizationUrl) {
          setAuthorizationUrl("");
          setError(
            state.errorCode
              ? `Authorization failed (${state.errorCode}).`
              : "Authorization was not completed.",
          );
        }
        break;
    }
  }

  // While the browser signs in, poll until the step completes or fails.
  useEffect(() => {
    if (!authorizationUrl || !started.current) return;
    const timer = setInterval(() => {
      void connectionSetup
        .getSetup(request())
        .then((result) => follow(result.state))
        .catch(() => {
          /* Transient; the next poll retries. */
        });
    }, 1000);
    return () => clearInterval(timer);
  }, [authorizationUrl]);

  async function submit(values: Values) {
    if (!method || submitting) return;
    setSubmitting(true);
    setError("");
    try {
      const name = values.name.trim();
      if (!started.current) {
        const id = existing?.id ?? randomUUID();
        const result = existing
          ? await connections.reconnect({ id })
          : await connections.startConnection({
              id,
              name,
              providerId: provider.id,
              typeId: method.id,
              assignments: [],
            });
        const url = new URL(result.brokeringUrl, window.location.origin);
        started.current = {
          setupId: url.pathname.split("/").filter(Boolean).at(-1) ?? "",
          token: url.searchParams.get("connection_setup_token") ?? "",
          connectionId: result.connection?.id ?? id,
          url: url.href,
          typeId: method.id,
        };
        onStarted();
      }
      if (!credentialFields(method)) {
        onCustom(
          { title: setupTitle(provider.name), url: started.current.url },
          started.current.connectionId,
        );
        return;
      }
      let state = (await connectionSetup.getSetup(request())).state;
      if (state?.action.case !== "form") return follow(state);
      if (state.connectionName !== name)
        state = (
          await connectionSetup.setConnectionName({ ...request(), actionId: state.actionId, name })
        ).state;
      const payload = fields.flatMap((field) => {
        const value = values.fields[field.key] ?? "";
        return value.trim() || field.required ? [{ key: field.key, value }] : [];
      });
      const next = { ...request(), actionId: state?.actionId ?? "", fields: payload };
      const result =
        state?.authDriver === AuthDriver.STATIC
          ? await connectionSetup.saveCredentials(next)
          : await connectionSetup.startOAuth(next);
      if (result.state?.action.case === "redirect")
        window.open(result.state.action.value.url, "_blank", "noopener");
      follow(result.state);
    } catch (e) {
      // A refused submit returns the setup to its form; the next submit fetches its new action.
      setError(message(e));
    } finally {
      setSubmitting(false);
    }
  }

  async function done() {
    try {
      const state = (await connectionSetup.getSetup(request())).state;
      if (state?.action.case === "complete") return finish();
    } catch {
      /* Closing leaves the account pending; its setup can be finished from its page. */
    }
    onClose();
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !submitting) onClose();
      }}
    >
      <DialogContent
        className={cn(dialogChrome, "max-h-[80vh] overflow-y-auto sm:max-w-[460px]")}
        showCloseButton={false}
      >
        <form className="grid gap-4" onSubmit={form.handleSubmit(submit)}>
          <header className="flex items-start gap-3">
            <CatalogIcon id={iconId} name={provider.name} iconUrl={provider.iconUrl} />
            <div className="min-w-0 flex-1">
              <DialogTitle className={dialogTitle}>
                {existing ? `Reconnect ${existing.name}` : setupTitle(provider.name)}
              </DialogTitle>
            </div>
          </header>

          {authorizationUrl ? (
            <div className="grid gap-4">
              <div className="flex gap-3 rounded-xl bg-muted p-3 text-[12.5px] text-foreground/60">
                <ExternalLinkIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
                <p className="m-0 leading-[18px]" role="status">
                  Waiting for {provider.name} authorization. Finish signing in with your browser,
                  then return here.
                </p>
              </div>
              <div className="flex justify-end gap-2">
                <Button
                  className={dialogButton}
                  disabled={authorizationUrl === "about:blank"}
                  onClick={() => window.open(authorizationUrl, "_blank", "noopener")}
                  type="button"
                  variant="outline"
                >
                  <ExternalLinkIcon aria-hidden="true" className="size-3.5" />
                  Reopen authorization
                </Button>
                <Button className={dialogButton} onClick={() => void done()} type="button">
                  Done
                </Button>
              </div>
            </div>
          ) : (
            <>
              {methods.length > 1 ? (
                <label className="grid gap-1.5">
                  <span className={label}>
                    <ShieldCheckIcon aria-hidden="true" className="size-3.5" />
                    Sign-in method
                  </span>
                  <select
                    className={input}
                    // The started setup, and an existing account, are bound to their method.
                    disabled={!!started.current || !!existing}
                    {...form.register("typeId", {
                      onChange: () => form.setValue("fields", {}),
                    })}
                  >
                    {methods.map((candidate) => (
                      <option key={candidate.id} value={candidate.id}>
                        {candidate.name}
                      </option>
                    ))}
                  </select>
                </label>
              ) : !method ? (
                <p className="m-0 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
                  No connection methods are available for this provider.
                </p>
              ) : null}

              <label className="grid gap-1.5">
                <span className={label}>
                  <UserRoundIcon aria-hidden="true" className="size-3.5" />
                  {provider.accountNameLabel || "Account name"}
                </span>
                <input
                  autoFocus
                  className={input}
                  required
                  {...form.register("name", { required: true })}
                />
                <span className={helper}>
                  Used to identify this account when choosing it for an agent.
                </span>
              </label>

              {fields.map((field) => (
                <label className="grid gap-1.5" key={`${method?.id}:${field.key}`}>
                  <span className={label}>
                    <KeyRoundIcon aria-hidden="true" className="size-3.5" />
                    {field.label}
                    {field.required ? null : (
                      <em className="font-normal text-foreground/40 not-italic">(optional)</em>
                    )}
                  </span>
                  {field.choices ? (
                    <select
                      className={input}
                      required={field.required}
                      {...form.register(`fields.${field.key}`)}
                    >
                      <option value="">Select…</option>
                      {field.choices.map((choice) => (
                        <option key={choice} value={choice}>
                          {choice}
                        </option>
                      ))}
                    </select>
                  ) : field.multiline ? (
                    <textarea
                      className={cn(input, "h-auto min-h-24 resize-y py-2")}
                      required={field.required}
                      rows={5}
                      spellCheck={false}
                      {...form.register(`fields.${field.key}`)}
                    />
                  ) : (
                    <input
                      autoComplete={field.secret ? "new-password" : "off"}
                      className={input}
                      required={field.required}
                      spellCheck={false}
                      type={field.secret ? "password" : "text"}
                      {...form.register(`fields.${field.key}`)}
                    />
                  )}
                  {field.description ? <span className={helper}>{field.description}</span> : null}
                </label>
              ))}

              {browser ? (
                <div className="flex gap-3 rounded-xl border-[0.5px] border-foreground/10 bg-background p-3 text-[11.5px] leading-4 text-foreground/60">
                  <ExternalLinkIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
                  <div className="grid gap-1">
                    <p className="m-0">
                      You’ll continue in your browser to sign in and authorize this account.
                    </p>
                    {/* An app of one's own must list Tilde's callback among its redirect URLs. */}
                    {fields.some((field) => field.key === "client_id") ? (
                      <p className="m-0">
                        Register this redirect URL with your app:{" "}
                        <code className="break-all">
                          {new URL("/connections/callback", window.location.href).href}
                        </code>
                      </p>
                    ) : null}
                  </div>
                </div>
              ) : null}

              {error ? (
                <p
                  className="m-0 rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive"
                  role="alert"
                >
                  {error}
                </p>
              ) : null}

              <div className="flex justify-end gap-2 pt-1">
                <Button
                  className={dialogButton}
                  disabled={submitting}
                  onClick={onClose}
                  type="button"
                  variant="outline"
                >
                  Cancel
                </Button>
                <Button
                  className={dialogButton}
                  disabled={submitting || missingRequired}
                  type="submit"
                >
                  {submitting ? (
                    "Connecting…"
                  ) : browser ? (
                    <>
                      Continue
                      <ExternalLinkIcon aria-hidden="true" className="size-3.5" />
                    </>
                  ) : (
                    "Connect"
                  )}
                </Button>
              </div>
            </>
          )}
        </form>
      </DialogContent>
    </Dialog>
  );
}
