import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

// Provider pages mount themselves on import; capture the component instead.
const pages = vi.hoisted(() => ({ app: undefined as undefined | (() => ReactNode) }));
vi.mock("@trytilde/connection-ui", async (original) => ({
  ...(await original<typeof import("@trytilde/connection-ui")>()),
  mount: (app: () => ReactNode) => {
    pages.app = app;
  },
}));

type Call = { method: string; action?: string; fields?: Record<string, string> };
let calls: Call[] = [];
let state: Record<string, unknown> = {};

/** Stands in for BrokeringPage: hands the page a port and walks GitHub's setup steps. */
function host() {
  const port = {
    onmessage: null as null | ((event: { data: unknown }) => void),
    postMessage(data: Call & { id: number }) {
      calls.push({ method: data.method, action: data.action, fields: data.fields });
      if (data.method === "executeProviderAction") {
        const fields = data.fields ?? {};
        if (state.step === "fields") state = { ...state, step: `github_${fields.setup_path}` };
        else if (state.step === "github_create")
          state = {
            ...state,
            step: "github_manifest",
            action: {
              case: "formPost",
              value: {
                url: `https://github.com/organizations/${fields.account}/settings/apps/new`,
                fields: [{ name: "manifest", value: "{}" }],
              },
            },
          };
        else state = { ...state, action: { case: "complete", value: {} } };
      }
      queueMicrotask(() => port.onmessage?.({ data: { id: data.id, state } }));
    },
    close() {},
  };
  window.addEventListener("message", function ready(event) {
    if (event.data?.type !== "tilde.setup.ready") return;
    window.removeEventListener("message", ready);
    const message = new MessageEvent("message", {
      source: window,
      data: { type: "tilde.setup.port" },
    });
    Object.defineProperty(message, "ports", { value: [port] });
    window.dispatchEvent(message);
  });
}

async function open(provider: string) {
  vi.resetModules();
  await import(`../../../../crates/tilde/src/connections/catalog/${provider}/ui.tsx`);
  const App = pages.app!;
  render(<App />);
  fireEvent.change(await screen.findByLabelText("GitHub account name"), {
    target: { value: "faroai" },
  });
}

beforeEach(() => {
  calls = [];
  state = {
    providerName: "GitHub",
    connectionName: "GitHub",
    accountNameLabel: "GitHub account name",
    webhookUrl: "https://trytilde.ai/connections/webhooks/conn",
    setupInstructions: [],
    actionId: "action",
    authDriver: 5,
    step: "fields",
    action: { case: "form", value: {} },
  };
  host();
});
afterEach(cleanup);

it("continues a new GitHub App to the manifest post", async () => {
  await open("github");
  fireEvent.click(screen.getByRole("button", { name: "Create an app" }));
  fireEvent.change(await screen.findByLabelText("App name"), { target: { value: "faroai" } });
  fireEvent.change(screen.getByLabelText(/App owner/), { target: { value: "organization" } });
  const organization = await screen.findByRole("textbox", { name: "Organization" });
  fireEvent.change(organization, { target: { value: "faro engineering" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  // An invalid value is reported in the page, not left to a browser bubble.
  expect((await screen.findByRole("alert")).textContent).toContain("Organization");
  expect(calls.filter((call) => call.method === "executeProviderAction")).toHaveLength(1);

  fireEvent.change(organization, { target: { value: "faro-engineering" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  const post = await screen.findByRole("button", { name: "Create app with provider" });
  expect(post.closest("form")?.action).toBe(
    "https://github.com/organizations/faro-engineering/settings/apps/new",
  );
  expect(calls.at(-1)).toEqual({
    method: "executeProviderAction",
    action: "continue",
    fields: { app_name: "faroai", owner_type: "organization", account: "faro-engineering" },
  });
});

it("submits an existing GitHub App", async () => {
  await open("github");
  fireEvent.click(screen.getByRole("button", { name: "Use existing app" }));
  fireEvent.change(await screen.findByLabelText("App ID"), { target: { value: "123" } });
  fireEvent.change(screen.getByLabelText("Private key (PEM)"), { target: { value: "pem" } });
  fireEvent.change(screen.getByLabelText("Installation ID"), { target: { value: "456" } });
  fireEvent.change(screen.getByLabelText("Webhook secret"), { target: { value: "secret" } });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  });
  await waitFor(() =>
    expect(calls.at(-1)?.fields).toEqual({
      app_id: "123",
      private_key: "pem",
      installation_id: "456",
      webhook_secret: "secret",
    }),
  );
});
