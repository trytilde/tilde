import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AuthDriver, BrokeringSchema } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";
import { Page, StandardSetup, useSetup } from "@trytilde/connection-ui";

const getSetup = vi.hoisted(() => vi.fn());
const setConnectionName = vi.hoisted(() => vi.fn());
const saveCredentials = vi.hoisted(() => vi.fn());
vi.mock("../../sdk/ts/packages/sdk/dist/connection-setup.js", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../sdk/ts/packages/sdk/dist/connection-setup.js")>()),
  createConnectionSetupClient: () => ({
    getSetup,
    setConnectionName,
    saveCredentials,
    dispose: vi.fn(),
  }),
}));
function Provider() {
  const setup = useSetup();
  return (
    <Page setup={setup}>
      <p>Configuration form</p>
    </Page>
  );
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.resetAllMocks();
  vi.useRealTimers();
});

it("opens embedded OAuth outside the agent page and follows callback state through the bridge", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("top", {});
  getSetup
    .mockResolvedValueOnce(
      create(BrokeringSchema, {
        providerName: "Provider",
        action: { case: "redirect", value: { url: "https://provider.example.com/authorize" } },
      }),
    )
    .mockResolvedValue(
      create(BrokeringSchema, {
        providerName: "Provider",
        action: { case: "form", value: {} },
      }),
    );
  await act(async () => {
    render(<Provider />);
  });
  expect(screen.getByRole("link", { name: "Continue with provider" }).getAttribute("target")).toBe(
    "_blank",
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2000);
  });
  expect(screen.getByText("Configuration form")).toBeTruthy();
  expect(getSetup).toHaveBeenCalledTimes(2);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(4000);
  });
  expect(getSetup).toHaveBeenCalledTimes(2);
});

it("opens embedded provider manifest posts in a separate window", async () => {
  vi.stubGlobal("top", {});
  getSetup.mockResolvedValue(
    create(BrokeringSchema, {
      providerName: "Provider",
      action: {
        case: "formPost",
        value: {
          url: "https://provider.example.com/apps",
          fields: [{ name: "manifest", value: "test-manifest" }],
        },
      },
    }),
  );
  await act(async () => {
    render(<Provider />);
  });
  const form = screen.getByRole("button", { name: "Create app with provider" }).closest("form")!;
  expect(form.target).toBe("_blank");
  expect(form.method).toBe("post");
  expect(form.querySelector('input[name="manifest"]')?.getAttribute("value")).toBe("test-manifest");
});

it("renders the catalog icon beside the setup title and instructions beneath it", async () => {
  getSetup.mockResolvedValue(
    create(BrokeringSchema, {
      providerName: "Provider",
      iconUrl: "https://cdn.example.com/provider.svg",
      instructions: "Enter your provider credentials to continue.",
      action: { case: "form", value: {} },
    }),
  );
  await act(async () => {
    render(<Provider />);
  });
  const title = screen.getByRole("heading", { name: "Connect Provider to Tilde" });
  const header = title.closest("header")!;
  expect(header.querySelector("img")?.getAttribute("src")).toBe(
    "https://cdn.example.com/provider.svg",
  );
  expect(screen.queryByLabelText("Webhook URL")).toBeNull();
  expect(title.nextElementSibling?.textContent).toBe(
    "Enter your provider credentials to continue.",
  );
});

it("submits the provider-labelled account name with the credential form before saving credentials", async () => {
  const initial = create(BrokeringSchema, {
    providerName: "AgentMail",
    connectionName: "AgentMail",
    accountNameLabel: "AgentMail email address",
    instructions: "Connect your AgentMail inbox.",
    setupInstructions: ["Create an inbox.", "Generate an inbox-scoped API key."],
    webhookUrl: "https://fixture.ngrok.app/connections/webhooks/mail",
    actionId: "initial",
    authDriver: AuthDriver.STATIC,
    action: { case: "form", value: {} },
    inputSchemaJson: JSON.stringify({
      type: "object",
      properties: { api_key: { type: "string", title: "API key" } },
      required: ["api_key"],
    }),
  });
  getSetup
    .mockResolvedValueOnce(initial)
    .mockResolvedValue({ ...initial, actionId: "named", connectionName: "assistant@example.com" });
  setConnectionName.mockResolvedValue({
    ...initial,
    actionId: "named",
    connectionName: "assistant@example.com",
  });
  saveCredentials.mockRejectedValue(new Error("Retry credentials"));
  render(<StandardSetup />);
  const account = (await screen.findByLabelText("AgentMail email address")) as HTMLInputElement;
  const key = screen.getByLabelText("API key") as HTMLInputElement;
  const webhook = screen.getByLabelText("Webhook URL") as HTMLInputElement;
  expect(webhook.readOnly).toBe(true);
  expect(webhook.value).toBe(initial.webhookUrl);
  expect(account.parentElement?.nextElementSibling?.contains(webhook)).toBe(true);
  expect(webhook.form).toBeNull();
  expect(screen.queryByLabelText("Inbox ID")).toBeNull();
  expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(
    initial.setupInstructions,
  );
  const writeText = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", Object.create(navigator, { clipboard: { value: { writeText } } }));
  fireEvent.click(screen.getByRole("button", { name: "Copy webhook URL" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith(initial.webhookUrl));
  await screen.findByText("Webhook URL copied");
  expect(setConnectionName).not.toHaveBeenCalled();
  expect(account.value).toBe("");
  expect(account.form).toBe(key.form);
  expect(account.form).not.toBeNull();
  expect(screen.queryByRole("button", { name: "Save draft" })).toBeNull();
  fireEvent.change(account, { target: { value: "assistant@example.com" } });
  fireEvent.change(key, { target: { value: "private-key" } });
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  await waitFor(() =>
    expect(setConnectionName).toHaveBeenCalledWith("initial", "assistant@example.com"),
  );
  await waitFor(() =>
    expect(saveCredentials).toHaveBeenCalledWith("named", { api_key: "private-key" }),
  );
  await screen.findByText("Retry credentials");
  expect(account.value).toBe("assistant@example.com");
  expect(key.value).toBe("");
  expect(screen.getByRole("heading", { name: "Connect AgentMail to Tilde" })).toBeTruthy();
});

it.each([true, false])(
  "handles denied iframe clipboard access (fallback succeeds: %s)",
  async (copies) => {
    getSetup.mockResolvedValue(
      create(BrokeringSchema, {
        providerName: "Provider",
        webhookUrl: "http://localhost/connections/webhooks/example",
        action: { case: "form", value: {} },
      }),
    );
    const writeText = vi.fn().mockRejectedValue(new Error("Clipboard permission denied"));
    vi.stubGlobal("navigator", Object.create(navigator, { clipboard: { value: { writeText } } }));
    const original = Object.getOwnPropertyDescriptor(document, "execCommand");
    const execCommand = vi.fn().mockReturnValue(copies);
    Object.defineProperty(document, "execCommand", { configurable: true, value: execCommand });
    try {
      render(<Provider />);
      const field = (await screen.findByLabelText("Webhook URL")) as HTMLInputElement;
      fireEvent.click(screen.getByRole("button", { name: "Copy webhook URL" }));
      await waitFor(() => expect(execCommand).toHaveBeenCalledWith("copy"));
      expect(document.activeElement).toBe(field);
      expect(field.selectionStart).toBe(0);
      expect(field.selectionEnd).toBe(field.value.length);
      await screen.findByText(
        copies ? "Webhook URL copied" : "Copy unavailable. Select the URL and copy it manually.",
      );
    } finally {
      if (original) Object.defineProperty(document, "execCommand", original);
      else Reflect.deleteProperty(document, "execCommand");
    }
  },
);
