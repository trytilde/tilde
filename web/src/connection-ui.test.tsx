import { fireEvent, render, screen, waitFor, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  Form,
  Input,
  Label,
  useConnectionForm,
  useForm,
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
