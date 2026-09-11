import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { IdentityVerificationHost } from "./-identity-verification";
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
