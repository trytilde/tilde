import { afterEach, expect, it, vi } from "vitest";
import { login } from "./auth";
import { randomUUID } from "./lib/browser-crypto";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

it("starts PKCE login and generates record IDs on remote HTTP without secure-context crypto APIs", async () => {
  vi.stubGlobal("crypto", { getRandomValues: (bytes: Uint8Array) => bytes.fill(0) });
  const assign = vi.fn();
  vi.stubGlobal("location", { assign });
  const fetch = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({
      state: "login-state",
      authorization_url: "http://100.64.12.34:5556/auth",
    }),
  });
  vi.stubGlobal("fetch", fetch);
  await login();
  const saved = JSON.parse(localStorage.getItem("tilde.oidc_login")!);
  const expected = "DwBzhbb51LfusnSGBa_hqYSgo7-j8BTQnip4TOnlzRo";
  expect(saved.verifier).toBe("A".repeat(43));
  expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({ challenge: expected });
  expect(assign).toHaveBeenCalledWith("http://100.64.12.34:5556/auth");
  expect(randomUUID()).toMatch(
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
  );
});
