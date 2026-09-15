import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { IdentityVerificationHost, IdentityVerificationForm } from "./-identity-verification";
const rpc = vi.hoisted(() => ({
  getIdentityVerification: vi.fn(),
  approveIdentityVerification: vi.fn(),
}));
vi.mock("@connectrpc/connect", () => ({ createClient: () => rpc }));
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sessionStorage.clear();
});
it("keeps the secret in the host and requires an explicit approval command from its own iframe", async () => {
  history.replaceState(
    null,
    "",
    "/identity/verify/request?identity_verification_token=private-proof",
  );
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
  rpc.getIdentityVerification.mockResolvedValue({
    verification: { id: "request", status: "delivered", value: "+12025550101" },
  });
  rpc.approveIdentityVerification.mockResolvedValue({
    verification: { id: "request", status: "approved", value: "+12025550101" },
  });
  render(<IdentityVerificationHost />);
  const frame = screen.getByTitle("Approve identity access") as HTMLIFrameElement;
  expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
  expect(frame.src).not.toContain("private-proof");
  expect(location.search).toBe("");
  const transfer = vi.spyOn(frame.contentWindow!, "postMessage");
  await act(() =>
    window.dispatchEvent(
      new MessageEvent("message", {
        source: window,
        origin: "null",
        data: { type: "tilde.identity.ready" },
      }),
    ),
  );
  expect(transfer).not.toHaveBeenCalled();
  await act(() =>
    window.dispatchEvent(
      new MessageEvent("message", {
        source: frame.contentWindow,
        origin: "null",
        data: { type: "tilde.identity.ready" },
      }),
    ),
  );
  expect(transfer).toHaveBeenCalled();
  expect(JSON.stringify(transfer.mock.calls)).not.toContain("private-proof");
  await act(async () => {
    await port.onmessage!({ data: { id: 1, method: "read", requestId: "different" } });
  });
  expect(rpc.getIdentityVerification).toHaveBeenCalledWith(
    { id: "request", identityVerificationToken: "private-proof" },
    expect.anything(),
  );
  expect(rpc.approveIdentityVerification).not.toHaveBeenCalled();
  await act(async () => {
    await port.onmessage!({ data: { id: 2, method: "approve", requestId: "different" } });
  });
  await waitFor(() =>
    expect(rpc.approveIdentityVerification).toHaveBeenCalledWith(
      { id: "request", identityVerificationToken: "private-proof" },
      expect.anything(),
    ),
  );
});

it("uses the shared branding and explains both identities before explicit approval", async () => {
  const port = {
    onmessage: null as null | ((event: { data: unknown }) => void),
    postMessage: vi.fn(),
    close: vi.fn(),
  };
  vi.spyOn(window.parent, "postMessage").mockImplementation(() => {});
  render(<IdentityVerificationForm />);
  await act(() =>
    window.dispatchEvent(
      new MessageEvent("message", {
        source: window.parent,
        data: { type: "tilde.identity.port" },
        ports: [port as unknown as MessagePort],
      }),
    ),
  );
  expect(port.postMessage).toHaveBeenCalledWith({ id: 1, method: "read" });
  const verification = {
    id: "request",
    status: "delivered",
    value: "user@example.com",
    agentName: "Daniel Agent",
    accountName: "Editable label",
    providerName: "AgentMail",
    iconUrl: "https://example.com/mail.svg",
    agentIdentity: { agentId: "agent", connectionId: "connection", value: "heyash@agentmail.to" },
  };
  act(() => port.onmessage!({ data: { id: 1, verification } }));
  expect(screen.getByLabelText("Tilde")).toBeTruthy();
  expect(screen.getByAltText("AgentMail")).toBeTruthy();
  expect(
    screen.getByRole("heading", { name: "Link user@example.com to Daniel Agent" }),
  ).toBeTruthy();
  const body = screen.getByText(/Clicking approve/).textContent;
  expect(body).toContain("contact Daniel Agent via heyash@agentmail.to on AgentMail");
  expect(body).toContain(
    "Daniel Agent will also be allowed to send messages to you via user@example.com",
  );
  expect(screen.queryByText("Editable label")).toBeNull();
  expect(port.postMessage).not.toHaveBeenCalledWith(expect.objectContaining({ method: "approve" }));
  fireEvent.click(screen.getByRole("button", { name: "Approve" }));
  expect(port.postMessage).toHaveBeenLastCalledWith({ id: 2, method: "approve" });
  act(() =>
    port.onmessage!({ data: { id: 2, verification: { ...verification, status: "approved" } } }),
  );
  expect(screen.getByText(/^Approved\. You can now contact/).textContent).toContain(
    "heyash@agentmail.to",
  );
});
