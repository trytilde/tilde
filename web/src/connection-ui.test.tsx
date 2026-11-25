import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  cleanup,
  renderHook,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  Form,
  Input,
  Label,
  useConnectionForm,
  Page,
  useForm,
  useSetup,
  type Setup,
} from "@trytilde/connection-ui";
afterEach(cleanup);
it("registers provider inputs with React Hook Form and clears submitted secrets", async () => {
  const submit = vi.fn().mockResolvedValue(undefined);
  const accountNameForm = renderHook(() => useForm<{ name: string }>()).result.current;
  const setup = {
    formId: "provider-form",
    accountNameForm,
    state: undefined,
    error: "",
    busy: false,
    submit,
    cancel: vi.fn(),
    saveDraft: vi.fn(),
    startOAuth: vi.fn(),
    saveCredentials: vi.fn(),
    executeProviderAction: vi.fn(),
  } satisfies Setup;
  function Provider() {
    const form = useConnectionForm();
    return (
      <Form setup={setup} form={form}>
        <Label>
          API key
          <Input {...form.register("api_key", { required: "API key is required" })} />
        </Label>
      </Form>
    );
  }
  render(<Provider />);
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
  expect(submit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "private-key" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() => expect(submit).toHaveBeenCalledWith({ api_key: "private-key" }));
  expect((screen.getByLabelText("API key") as HTMLInputElement).value).toBe("");
});

it("renders JSON Schema secret, boolean, enum and numeric fields without provider UI code", async () => {
  const { create } = await import("@bufbuild/protobuf");
  const { BrokeringSchema, AuthDriver } =
    await import("@trytilde/contracts/tilde/setup/v1/connections_pb.js");
  const { CredentialForm } = await import("@trytilde/connection-ui");
  const submit = vi.fn().mockResolvedValue(undefined);
  const accountNameForm = renderHook(() => useForm<{ name: string }>()).result.current;
  const setup = {
    formId: "provider-form",
    accountNameForm,
    state: undefined,
    error: "",
    busy: false,
    submit,
    cancel: vi.fn(),
    saveDraft: vi.fn(),
    startOAuth: vi.fn(),
    saveCredentials: vi.fn(),
    executeProviderAction: vi.fn(),
  } satisfies Setup;
  const state = create(BrokeringSchema, {
    authDriver: AuthDriver.STATIC,
    draft: [],
    action: { case: "form", value: {} },
    inputSchemaJson: JSON.stringify({
      type: "object",
      additionalProperties: false,
      properties: {
        "api.key": { type: "string", title: "API key", writeOnly: true, minLength: 4 },
        enabled: { type: "boolean", title: "Enabled" },
        region: { type: "string", title: "Region", enum: ["eu", "us"] },
        limit: { type: "integer", title: "Limit", minimum: 1, maximum: 5 },
      },
      required: ["api.key", "enabled", "region", "limit"],
    }),
  });
  render(<CredentialForm setup={setup} state={state} />);
  expect((screen.getByLabelText("API key") as HTMLInputElement).type).toBe("password");
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "private" } });
  fireEvent.change(screen.getByLabelText("Enabled"), { target: { value: "false" } });
  fireEvent.change(screen.getByLabelText("Region"), { target: { value: '"eu"' } });
  fireEvent.change(screen.getByLabelText("Limit"), { target: { value: "99" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText("Maximum: 5");
  expect(submit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Limit"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() =>
    expect(submit).toHaveBeenCalledWith({
      "api.key": "private",
      enabled: "false",
      region: "eu",
      limit: "3",
    }),
  );
});

it("takes the host's theme through the setup handshake, and a later change", async () => {
  // Stands in for BrokeringPage: answers the page's `ready` with a port and the theme.
  const channel = new MessageChannel();
  channel.port1.onmessage = ({ data }) =>
    channel.port1.postMessage({
      id: data.id,
      state: { providerName: "Linear", setupInstructions: [], action: { case: "form", value: {} } },
    });
  window.addEventListener("message", function ready(event) {
    if (event.data?.type !== "tilde.setup.ready") return;
    window.removeEventListener("message", ready);
    const message = new MessageEvent("message", {
      source: window,
      data: {
        type: "tilde.setup.port",
        theme: { scheme: "dark", tokens: { accent: "#89c0ff", surface: "#30211d", radius: "2px" } },
      },
    });
    Object.defineProperty(message, "ports", { value: [channel.port2] });
    window.dispatchEvent(message);
  });
  // A provider's own page: it only uses `useSetup` and `Page`.
  function Provider() {
    return <Page setup={useSetup()}>fields</Page>;
  }
  const root = document.documentElement;
  const variable = (name: string) => getComputedStyle(root).getPropertyValue(name);
  try {
    render(<Provider />);
    await screen.findByText("Connect Linear to Tilde");
    expect(root.classList.contains("dark")).toBe(true);
    expect(variable("--primary")).toBe("#89c0ff");
    expect(variable("--primary-foreground")).toContain("#89c0ff");
    expect(variable("--card")).toBe("#30211d");
    expect(variable("--radius")).toBe("2px");

    // A change replaces the theme: variables of tokens it no longer names go back to the page's.
    await act(async () => {
      channel.port1.postMessage({
        theme: { scheme: "light", tokens: { canvas: "#fffaf0", font: "Inter" } },
      });
      await waitFor(() => expect(variable("--background")).toBe("#fffaf0"));
    });
    expect(root.classList.contains("dark")).toBe(false);
    expect(variable("--font-body")).toBe("Inter");
    expect(variable("--primary")).toBe("");
    expect(variable("--card")).toBe("");
  } finally {
    channel.port1.close();
    root.removeAttribute("style");
    root.classList.remove("dark");
  }
});

it("prefills schema fields from the page's URL, never secrets, with saved drafts winning", async () => {
  const { create } = await import("@bufbuild/protobuf");
  const { BrokeringSchema, AuthDriver } =
    await import("@trytilde/contracts/tilde/setup/v1/connections_pb.js");
  const { CredentialForm } = await import("@trytilde/connection-ui");
  const submit = vi.fn().mockResolvedValue(undefined);
  const accountNameForm = renderHook(() => useForm<{ name: string }>()).result.current;
  const setup = {
    formId: "provider-form",
    accountNameForm,
    state: undefined,
    error: "",
    busy: false,
    submit,
    cancel: vi.fn(),
    saveDraft: vi.fn(),
    startOAuth: vi.fn(),
    saveCredentials: vi.fn(),
    executeProviderAction: vi.fn(),
  } satisfies Setup;
  const state = create(BrokeringSchema, {
    actionId: "act_1",
    authDriver: AuthDriver.STATIC,
    draft: [{ key: "site", value: "drafted.example" }],
    action: { case: "form", value: {} },
    inputSchemaJson: JSON.stringify({
      type: "object",
      properties: {
        region: { type: "string", title: "Region", enum: ["eu", "us"] },
        tier: { type: "string", title: "Tier", enum: ["free", "pro"] },
        enabled: { type: "boolean", title: "Enabled" },
        site: { type: "string", title: "Site" },
        account: { type: "string", title: "Account" },
        passphrase: { type: "string", title: "Passphrase", writeOnly: true },
        api_key: { type: "string", title: "API key" },
      },
    }),
  });
  const previous = window.location.href;
  window.history.replaceState(
    null,
    "",
    "/catalog/_standard/ui?field.region=eu&field.tier=enterprise&field.enabled=true" +
      "&field.site=from-url.example&field.account=acme&field.passphrase=p&field.api_key=k",
  );
  try {
    render(<CredentialForm setup={setup} state={state} />);
    const value = (label: string) => (screen.getByLabelText(label) as HTMLInputElement).value;
    expect(value("Region")).toBe('"eu"');
    // Not one of the field's choices: ignored.
    expect(value("Tier")).toBe("");
    expect(value("Enabled")).toBe("true");
    await waitFor(() => expect(value("Site")).toBe("drafted.example"));
    expect(value("Account")).toBe("acme");
    expect(value("Passphrase")).toBe("");
    expect(value("API key")).toBe("");
    // Prefilled values stay editable.
    fireEvent.change(screen.getByLabelText("Account"), { target: { value: "acme-2" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        region: "eu",
        enabled: "true",
        site: "drafted.example",
        account: "acme-2",
      }),
    );
  } finally {
    window.history.replaceState(null, "", previous);
  }
});
