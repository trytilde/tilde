import { ArrowRightIcon, CopyIcon, CheckIcon } from "lucide-react";
import { LoadingReveal } from "./loading-reveal.js";
import { TildeWordmark } from "./wordmark.js";
import { Input } from "./components/input.js";
import { Label } from "./components/controls.js";
import { useForm, FormProvider, type UseFormReturn } from "react-hook-form";
import type { SetupValues } from "@trytilde/sdk/connection-setup";
import { Button, buttonVariants } from "./components/button.js";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { AuthDriver, type Brokering } from "@trytilde/sdk/connection-setup";

import { createConnectionSetupClient } from "@trytilde/sdk/connection-setup";
// Iframe widths stay narrow even on desktop, so use the primary input's capabilities.
const continueClass =
  "h-11 w-full gap-2 px-5 [@media(hover:hover)_and_(pointer:fine)]:h-12 [@media(hover:hover)_and_(pointer:fine)]:w-auto [@media(hover:hover)_and_(pointer:fine)]:px-6 [@media(hover:hover)_and_(pointer:fine)]:text-base";
type Client = ReturnType<typeof createConnectionSetupClient>;
export function useSetup() {
  const formId = useId();
  const accountNameForm = useForm<{ name: string }>({ defaultValues: { name: "" } });
  const nameInitialized = useRef(false);
  const client = useRef<Client | null>(null);
  const [state, setState] = useState<Brokering>();
  const latest = useRef<Brokering | undefined>(undefined);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function run(operation: (client: Client) => Promise<Brokering>) {
    setBusy(true);
    setError("");
    try {
      if (!client.current) throw new Error("Setup is loading");
      const updated = await operation(client.current);
      latest.current = updated;
      if (!nameInitialized.current) {
        // StartConnection uses the provider title as a temporary name until this form is filled.
        accountNameForm.setValue(
          "name",
          updated.connectionName === updated.providerName ? "" : updated.connectionName,
        );
        nameInitialized.current = true;
      }
      setState(updated);
      return updated;
    } catch (error) {
      setError(error instanceof Error ? error.message : "Setup failed");
      try {
        if (client.current) {
          const updated = await client.current.getSetup();
          latest.current = updated;
          setState(updated);
        }
      } catch {
        /* Keep the original operation error if the API is unavailable. */
      }
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    client.current = createConnectionSetupClient();
    void run((client) => client.getSetup());
    return () => {
      client.current?.dispose();
      client.current = null;
    };
  }, []);
  useEffect(() => {
    const inDialog = window.parent !== window.top;
    if (
      state?.action.case !== "working" &&
      !(inDialog && ["redirect", "formPost"].includes(state?.action.case ?? ""))
    )
      return;
    const timer = setTimeout(() => void run((client) => client.getSetup()), 2000);
    return () => clearTimeout(timer);
  }, [state]);
  const actionId = () => latest.current?.actionId ?? "";
  async function withName(operation: (client: Client) => Promise<Brokering>) {
    if (!(await accountNameForm.trigger())) return;
    return run(async (client) => {
      const updated = await client.setConnectionName(
        actionId(),
        accountNameForm.getValues("name").trim(),
      );
      latest.current = updated;
      setState(updated);
      return operation(client);
    });
  }
  return {
    state,
    error,
    busy,
    formId,
    accountNameForm,
    saveDraft: (draft: Record<string, string>) =>
      run((client) => client.saveDraft(actionId(), draft)),
    startOAuth: (fields: Record<string, string>) =>
      withName((client) => client.startOAuth(actionId(), fields)),
    saveCredentials: (fields: Record<string, string>) =>
      withName((client) => client.saveCredentials(actionId(), fields)),
    executeProviderAction: (action: string, fields: Record<string, string> = {}) =>
      withName((client) => client.executeProviderAction(actionId(), action, fields)),
    submit: (fields: Record<string, string>) =>
      withName((client) =>
        state?.authDriver === AuthDriver.STATIC
          ? client.saveCredentials(actionId(), fields)
          : [
                AuthDriver.OAUTH_CODE,
                AuthDriver.OAUTH_CLIENT_CREDENTIALS,
                AuthDriver.OAUTH_JWT_BEARER,
              ].includes(state?.authDriver ?? 0)
            ? client.startOAuth(actionId(), fields)
            : client.executeProviderAction(actionId(), "continue", fields),
      ),
    cancel: () => run((client) => client.cancelSetup()),
  };
}
export type Setup = ReturnType<typeof useSetup>;
// Optional presentation helpers. Providers can replace these with any React components.
export function Page({ setup, children }: { setup: Setup; children: ReactNode }) {
  const { state, error, busy } = setup;
  const [failedIcon, setFailedIcon] = useState<string>();
  const action = state?.action;
  return (
    <LoadingReveal loading={!state && !error} label="Loading connection setup">
      <main className="mx-auto max-w-lg space-y-5 p-6">
        <header className="space-y-5">
          <div className="flex items-center gap-4">
            <TildeWordmark markSize={24} />
            <span aria-hidden="true" className="text-xl text-muted-foreground">
              ×
            </span>
            {state?.iconUrl && state.iconUrl !== failedIcon ? (
              <img
                src={state.iconUrl}
                alt={state.providerName}
                width={32}
                height={32}
                referrerPolicy="no-referrer"
                onError={() => setFailedIcon(state.iconUrl)}
                className="size-8 object-contain"
              />
            ) : (
              <span
                className="inline-flex size-8 items-center justify-center rounded bg-muted font-semibold"
                aria-hidden="true"
              >
                {state?.providerName?.slice(0, 1)}
              </span>
            )}
          </div>
          <div className="space-y-2">
            <h1 className="text-2xl font-semibold">
              {state ? `Connect ${state.providerName} to Tilde` : "Connect to Tilde"}
            </h1>
            {state?.instructions && (
              <p className="whitespace-pre-line text-sm text-muted-foreground">
                {state.instructions}
              </p>
            )}
            {!!state?.setupInstructions.length && (
              <ol className="list-decimal space-y-2 pl-5 text-sm text-muted-foreground">
                {state.setupInstructions.map((instruction, index) => (
                  <li key={index}>{instruction}</li>
                ))}
              </ol>
            )}
          </div>
        </header>
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        {action?.case === "form" && (
          <>
            <div className="space-y-2">
              <Label htmlFor={`${setup.formId}-name`}>
                {state?.accountNameLabel || "Account name"}
              </Label>
              <Input
                id={`${setup.formId}-name`}
                form={setup.formId}
                disabled={busy}
                required
                aria-invalid={!!setup.accountNameForm.formState.errors.name}
                {...setup.accountNameForm.register("name", {
                  validate: (name) =>
                    !name.trim()
                      ? "Enter an account name."
                      : new TextEncoder().encode(name.trim()).length <= 200 ||
                        "Account name is too long.",
                })}
              />
              {setup.accountNameForm.formState.errors.name && (
                <p role="alert" className="text-sm text-destructive">
                  {setup.accountNameForm.formState.errors.name.message}
                </p>
              )}
            </div>
          </>
        )}
        {state?.webhookUrl && <WebhookUrl key={state.webhookUrl} url={state.webhookUrl} />}
        {action?.case === "form" && children}
        {action?.case === "redirect" && (
          <div className="flex justify-end">
            <a
              className={buttonVariants({ className: continueClass })}
              href={action.value.url}
              target={window.parent === window.top ? "_top" : "_blank"}
              rel="noreferrer"
            >
              Continue with provider
              <ArrowRightIcon aria-hidden="true" className="size-5" />
            </a>
          </div>
        )}
        {action?.case === "formPost" && <ProviderPost action={action.value} />}
        {action?.case === "working" && <p role="status">Setup is in progress…</p>}
        {action?.case === "complete" && (
          <p role="status">Connection setup is complete. You can close this window.</p>
        )}
        {action?.case === "failed" && (
          <p role="status">
            Setup failed ({state?.errorCode}). Reconnect to try again. Existing credentials are
            preserved.
          </p>
        )}
        {action?.case === "cancelled" && (
          <p role="status">Setup cancelled. Your provider account and apps remain.</p>
        )}
        {window.parent === window.top &&
          state &&
          !["complete", "failed", "cancelled"].includes(action?.case ?? "") && (
            <Button variant="secondary" disabled={busy} onClick={() => void setup.cancel()}>
              Cancel
            </Button>
          )}
      </main>
    </LoadingReveal>
  );
}
/** Consistent form defaults; credential requirements still belong to each provider. */
export function useConnectionForm(defaultValues: SetupValues = {}) {
  return useForm<SetupValues>({ defaultValues, shouldUnregister: true });
}
export function Form({
  setup,
  form,
  children,
  onSubmit,
}: {
  setup: Setup;
  form: UseFormReturn<SetupValues>;
  children: ReactNode;
  onSubmit?: (values: SetupValues) => Promise<unknown>;
}) {
  return (
    <FormProvider {...form}>
      <form
        id={setup.formId}
        className="space-y-4"
        onSubmit={form.handleSubmit(async (values) => {
          const input = Object.fromEntries(
            Object.entries(values).filter(([, value]) => value !== ""),
          );
          // Reset only this form's credential fields; the associated account-name field keeps its value.
          form.reset(
            Object.fromEntries(
              Object.keys(values).map((key) => [key, form.formState.defaultValues?.[key] ?? ""]),
            ),
          );
          await (onSubmit ? onSubmit(values) : setup.submit(input));
        })}
      >
        {children}
        <div className="flex justify-end">
          <Button
            type="submit"
            className={continueClass}
            disabled={setup.busy || form.formState.isSubmitting}
          >
            {setup.busy ? "Connecting…" : "Continue"}
            <ArrowRightIcon aria-hidden="true" className="size-5" />
          </Button>
        </div>
      </form>
    </FormProvider>
  );
}
export function mount(App: () => ReactNode) {
  createRoot(document.getElementById("root")!).render(<App />);
}

function ProviderPost({
  action,
}: {
  action: { url: string; fields: { name: string; value: string }[] };
}) {
  const element = useRef<HTMLFormElement>(null);
  const form = useConnectionForm(
    Object.fromEntries(action.fields.map((field) => [field.name, field.value])),
  );
  return (
    <form
      ref={element}
      action={action.url}
      method="post"
      rel="noreferrer"
      target={window.parent === window.top ? "_top" : "_blank"}
      onSubmit={form.handleSubmit(() => element.current?.submit())}
    >
      {action.fields.map((field) => (
        <input key={field.name} type="hidden" {...form.register(field.name)} />
      ))}
      <div className="flex justify-end">
        <Button type="submit" className={continueClass}>
          Create app with provider
          <ArrowRightIcon aria-hidden="true" className="size-5" />
        </Button>
      </div>
    </form>
  );
}

function WebhookUrl({ url }: { url: string }) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  async function copy() {
    setError("");
    setCopied(false);
    try {
      // Opaque sandbox origins and HTTP development hosts can deny the Clipboard API.
      if (!navigator.clipboard) throw new Error("Clipboard API unavailable");
      await navigator.clipboard.writeText(url);
      setCopied(true);
    } catch {
      input.current?.focus();
      input.current?.select();
      try {
        if (!document.execCommand("copy")) throw new Error("Copy unavailable");
        setCopied(true);
      } catch {
        setError("Copy unavailable. Select the URL and copy it manually.");
      }
    }
  }
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>Webhook URL</Label>
      <div className="flex">
        <Input
          ref={input}
          id={id}
          value={url}
          readOnly
          className="rounded-r-none font-mono text-sm"
        />
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="rounded-l-none border-l-0"
          aria-label="Copy webhook URL"
          onClick={() => void copy()}
        >
          {copied ? <CheckIcon aria-hidden="true" /> : <CopyIcon aria-hidden="true" />}
        </Button>
      </div>
      {copied && (
        <p role="status" className="sr-only">
          Webhook URL copied
        </p>
      )}
      {error && (
        <p role="status" className="text-sm text-muted-foreground">
          {error}
        </p>
      )}
    </div>
  );
}
