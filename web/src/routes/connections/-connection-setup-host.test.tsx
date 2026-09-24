import { cleanup, render, screen, waitFor, act } from "@testing-library/react";
import { afterEach, it, expect, vi } from "vitest";
import { BrokeringPage } from "./-connection-setup-host";
const rpc = vi.hoisted(() => ({
  getSetup: vi.fn(),
  setConnectionName: vi.fn(),
  executeProviderAction: vi.fn(),
  saveCredentials: vi.fn(),
  saveDraft: vi.fn(),
  startOAuth: vi.fn(),
  cancelSetup: vi.fn(),
}));
vi.mock("@connectrpc/connect", () => ({ createClient: () => rpc }));
const state = {
  setupId: "setup",
  connectionId: "connection",
  actionId: "action",
  step: "fields",
  uiPath: "/catalog/slack/ui",
  action: { case: "form", value: {} },
};
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sessionStorage.clear();
});
it("isolates provider code and scopes iframe operations to the host setup", async () => {
  history.replaceState(null, "", "/connections/broker/setup?connection_setup_token=private-token");
  rpc.getSetup.mockResolvedValue({ state });
  rpc.saveCredentials.mockResolvedValue({ state });
  const port = {
    onmessage: null as null | ((event: { data: unknown }) => Promise<void>),
    postMessage: vi.fn(),
    close: vi.fn(),
  };
  vi.stubGlobal(
    "MessageChannel",
    class {
      port1 = port;
      port2 = {};
    },
  );
  render(<BrokeringPage setupId="setup" />);
  const frame = (await screen.findByTitle("Connection setup")) as HTMLIFrameElement;
  expect(screen.getByRole("status", { name: "Loading connection setup" })).toBeTruthy();
  expect(frame.parentElement?.getAttribute("aria-hidden")).toBe("true");
  expect(frame.getAttribute("sandbox")).toContain("allow-scripts");
  expect(frame.getAttribute("sandbox")).not.toContain("allow-same-origin");
  expect(frame.src).not.toContain("private-token");
  expect(location.search).toBe("");
  const transfer = vi.spyOn(frame.contentWindow!, "postMessage");
  act(() => {
    window.dispatchEvent(
      new MessageEvent("message", {
        source: window,
        origin: "null",
        data: { type: "tilde.setup.ready" },
      }),
    );
  });
  expect(transfer).not.toHaveBeenCalled();
  act(() => {
    window.dispatchEvent(
      new MessageEvent("message", {
        source: frame.contentWindow,
        origin: "null",
        data: { type: "tilde.setup.ready" },
      }),
    );
  });
  expect(transfer).toHaveBeenCalled();
  // Port negotiation alone is not enough: wait for the provider to read its setup.
  expect(frame.parentElement?.getAttribute("aria-hidden")).toBe("true");
  await act(async () => {
    await port.onmessage!({ data: { id: 0, method: "read" } });
  });
  expect(frame.parentElement?.getAttribute("aria-hidden")).toBe("false");
  expect(JSON.stringify(transfer.mock.calls)).not.toContain("private-token");
  await act(async () => {
    await port.onmessage!({
      data: {
        id: 1,
        method: "saveCredentials",
        setupId: "other",
        actionId: "action",
        fields: { client_id: "client" },
      },
    });
  });
  expect(rpc.saveCredentials).toHaveBeenCalledWith({
    setupId: "setup",
    connectionSetupToken: "private-token",
    actionId: "action",
    fields: [{ key: "client_id", value: "client" }],
  });
  rpc.setConnectionName.mockResolvedValue({ state });
  await act(async () => {
    await port.onmessage!({
      data: {
        id: 3,
        method: "setConnectionName",
        setupId: "other",
        actionId: "action",
        connectionName: "Personal account",
      },
    });
  });
  expect(rpc.setConnectionName).toHaveBeenCalledWith({
    setupId: "setup",
    connectionSetupToken: "private-token",
    actionId: "action",
    name: "Personal account",
  });
  await act(async () => {
    await port.onmessage!({ data: { id: 2, method: "Disconnect" } });
  });
  await waitFor(() =>
    expect(port.postMessage).toHaveBeenCalledWith({ id: 2, error: "Unsupported setup operation" }),
  );
});

it("notifies an embedding dialog when brokering completes without disclosing the token", async () => {
  history.replaceState(null, "", "/connections/broker/setup?connection_setup_token=private-token");
  const postMessage = vi.fn();
  vi.stubGlobal("parent", { postMessage });
  rpc.getSetup.mockResolvedValue({ state: { ...state, action: { case: "complete", value: {} } } });
  render(<BrokeringPage setupId="setup" />);
  await waitFor(() =>
    expect(postMessage).toHaveBeenCalledWith(
      { type: "tilde.connection.complete", connectionId: "connection" },
      "*",
    ),
  );
  expect(JSON.stringify(postMessage.mock.calls)).not.toContain("private-token");
});

it("shows a setup fetch failure instead of leaving the loading overlay active", async () => {
  history.replaceState(null, "", "/connections/broker/setup?connection_setup_token=private-token");
  rpc.getSetup.mockRejectedValueOnce(new Error("Setup unavailable"));
  render(<BrokeringPage setupId="setup" />);
  expect(screen.getByRole("status", { name: "Loading connection setup" })).toBeTruthy();
  expect((await screen.findByRole("alert")).textContent).toContain("Setup unavailable");
  expect(screen.queryByRole("status", { name: "Loading connection setup" })).toBeNull();
});
