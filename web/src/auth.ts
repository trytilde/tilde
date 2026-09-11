import { pkceChallenge } from "@/lib/browser-crypto";
import { Code, ConnectError, type Interceptor } from "@connectrpc/connect";
const TOKEN = "tilde.access_token";
const LOGIN = "tilde.oidc_login";
export const getAccessToken = () => localStorage.getItem(TOKEN);
export const clearAccessToken = () => localStorage.removeItem(TOKEN);
export const authHeaders = () => {
  const token = getAccessToken();
  return token ? { Authorization: `Bearer ${token}` } : undefined;
};
export const authInterceptor: Interceptor = (next) => async (request) => {
  const token = getAccessToken();
  if (token) request.header.set("Authorization", `Bearer ${token}`);
  try {
    return await next(request);
  } catch (error) {
    if (error instanceof ConnectError && error.code === Code.Unauthenticated) {
      clearAccessToken();
      window.dispatchEvent(new Event("tilde.signed-out"));
    }
    throw error;
  }
};
function base64url(bytes: Uint8Array) {
  return btoa(String.fromCharCode(...bytes))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "");
}
export async function login() {
  const verifier = base64url(crypto.getRandomValues(new Uint8Array(32)));
  const challenge = pkceChallenge(verifier);
  const response = await fetch("/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ challenge }),
  });
  if (!response.ok) throw new Error("Unable to start login");
  const result = await response.json();
  localStorage.setItem(LOGIN, JSON.stringify({ state: result.state, verifier }));
  location.assign(result.authorization_url);
}
let exchange: Promise<void> | undefined;
export function finishLogin(): Promise<void> {
  // React strict mode may run the effect twice; redeem the one-time code once.
  return (exchange ??= (async () => {
    const params = new URLSearchParams(location.search);
    const saved = JSON.parse(localStorage.getItem(LOGIN) ?? "null");
    if (!saved || !params.get("code") || saved.state !== params.get("state"))
      throw new Error("Login response did not match this browser");
    const response = await fetch("/auth/exchange", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        code: params.get("code"),
        state: saved.state,
        verifier: saved.verifier,
      }),
    });
    localStorage.removeItem(LOGIN);
    if (!response.ok) throw new Error("Login could not be completed");
    const result = await response.json();
    localStorage.setItem(TOKEN, result.access_token);
    history.replaceState(null, "", "/");
  })());
}

/** Revoke the server session before clearing local credentials and notifying the auth boundary. */
export async function logout() {
  const response = await fetch("/auth/logout", { method: "POST", headers: authHeaders() });
  if (!response.ok && response.status !== 401)
    throw new Error("Unable to sign out. Please try again.");
  clearAccessToken();
  window.dispatchEvent(new Event("tilde.signed-out"));
}
