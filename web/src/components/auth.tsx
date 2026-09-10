import { useEffect, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { authHeaders, clearAccessToken, finishLogin, login } from "@/auth";
export function Auth({ children }: { children: ReactNode }) {
  const [state, setState] = useState<"loading" | "signed-out" | "signed-in" | "error">("loading");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const signedOut = () => setState("signed-out");
    window.addEventListener("tilde.signed-out", signedOut);
    (async () => {
      if (location.pathname === "/auth/callback") await finishLogin();
      const response = await fetch("/auth/session", { headers: authHeaders() });
      if (active) {
        if (response.status === 401) clearAccessToken();
        setState(response.ok ? "signed-in" : response.status === 401 ? "signed-out" : "error");
      }
    })().catch((e) => {
      if (active) {
        setState("error");
        setError(e instanceof Error ? e.message : "Login failed");
      }
    });
    return () => {
      active = false;
      window.removeEventListener("tilde.signed-out", signedOut);
    };
  }, []);
  if (state === "loading")
    return (
      <main className="p-8" role="status">
        Loading…
      </main>
    );
  if (state !== "signed-in")
    return (
      <main className="flex min-h-screen items-center justify-center">
        <div className="grid gap-4 text-center">
          <h1 className="text-2xl font-semibold">Sign in to Tilde</h1>
          <p>Use your identity provider to access this workspace.</p>
          {error && <p role="alert">{error}</p>}
          <Button onClick={() => login().catch((e) => setError(e.message))}>Sign in</Button>
        </div>
      </main>
    );
  return (
    <>
      <div className="fixed right-4 top-3 z-50">
        <Button
          variant="outline"
          size="sm"
          onClick={async () => {
            const response = await fetch("/auth/logout", {
              method: "POST",
              headers: authHeaders(),
            });
            if (response.ok || response.status === 401) {
              clearAccessToken();
              setState("signed-out");
            }
          }}
        >
          Sign out
        </Button>
      </div>
      {children}
    </>
  );
}
