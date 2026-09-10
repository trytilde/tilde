import { useForm, FormProvider, type UseFormReturn } from "react-hook-form";
import type { SetupValues } from "@trytilde/sdk/connection-setup";
import { Button, buttonVariants } from "./components/button.js";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { AuthDriver, type Brokering } from "@trytilde/sdk/connection-setup";

import { createConnectionSetupClient } from "@trytilde/sdk/connection-setup";
type Client = ReturnType<typeof createConnectionSetupClient>;
export function useSetup() {
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
    if (state?.action.case !== "working") return;
    const timer = setTimeout(() => void run((client) => client.getSetup()), 2000);
    return () => clearTimeout(timer);
  }, [state]);
  const actionId = () => latest.current?.actionId ?? "";
  return {
    state,
    error,
    busy,
    saveDraft: (draft: Record<string, string>) =>
      run((client) => client.saveDraft(actionId(), draft)),
    startOAuth: (fields: Record<string, string>) =>
      run((client) => client.startOAuth(actionId(), fields)),
    saveCredentials: (fields: Record<string, string>) =>
      run((client) => client.saveCredentials(actionId(), fields)),
    executeProviderAction: (action: string, fields: Record<string, string> = {}) =>
      run((client) => client.executeProviderAction(actionId(), action, fields)),
    submit: (fields: Record<string, string>) =>
      run((client) =>
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
export function Page({
  setup,
  title,
  children,
}: {
  setup: Setup;
  title: string;
  children: ReactNode;
}) {
  const { state, error, busy } = setup;
  const action = state?.action;
  return (
    <main className="mx-auto max-w-lg space-y-5 p-6">
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p>{state?.connectionName}</p>
      {error && (
        <p role="alert" className="text-red-700">
          {error}
        </p>
      )}
      {!state && <p role="status">Loading setup…</p>}
      {action?.case === "form" && children}
      {action?.case === "redirect" && (
        <a className={buttonVariants()} href={action.value.url} target="_top" rel="noreferrer">
          Continue with provider
        </a>
      )}
      {action?.case === "formPost" && <ProviderPost action={action.value} />}
      {action?.case === "working" && <p role="status">Setup is in progress…</p>}
      {state?.webhookUrl && (
        <div className="space-y-2 rounded border p-3 text-sm">
          <p>
            Use this webhook URL in your provider’s settings to receive messages after
            authorization:
          </p>
          <code className="block break-all">{state.webhookUrl}</code>
        </div>
      )}
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
      {state && !["complete", "failed", "cancelled"].includes(action?.case ?? "") && (
        <Button variant="secondary" disabled={busy} onClick={() => void setup.cancel()}>
          Cancel
        </Button>
      )}
    </main>
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
        className="space-y-4"
        onSubmit={form.handleSubmit(async (values) => {
          const input = Object.fromEntries(
            Object.entries(values).filter(([, value]) => value !== ""),
          );
          form.reset();
          await (onSubmit ? onSubmit(values) : setup.submit(input));
        })}
      >
        {children}
        <Button type="submit" disabled={setup.busy || form.formState.isSubmitting}>
          {setup.busy ? "Connecting…" : "Continue"}
        </Button>
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
      target="_top"
      onSubmit={form.handleSubmit(() => element.current?.submit())}
    >
      {action.fields.map((field) => (
        <input key={field.name} type="hidden" {...form.register(field.name)} />
      ))}
      <Button type="submit">Create app with provider</Button>
    </form>
  );
}
