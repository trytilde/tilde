import { LoadingReveal } from "@trytilde/connection-ui";
import { useEffect, useRef, useState } from "react";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { IdentityVerificationService } from "@trytilde/contracts/tilde/setup/v1/identity_verification_pb.js";
import type { IdentityVerification } from "@trytilde/contracts/tilde/types/v1/access_pb.js";
import { Button } from "@/components/ui/button";
import { ProviderIcon } from "@/components/provider-icon";

const client = createClient(
  IdentityVerificationService,
  createConnectTransport({ baseUrl: window.location.origin }),
);
/** A public token-authenticated host. No management credentials cross into the approval iframe. */
export function IdentityVerificationHost() {
  const [credentials] = useState(() => {
    const url = new URL(window.location.href);
    const id = url.pathname.split("/").at(-1)!;
    const key = `tilde:identity_verification:${id}`;
    const supplied = url.searchParams.get("identity_verification_token");
    if (supplied) {
      sessionStorage.setItem(key, supplied);
      url.searchParams.delete("identity_verification_token");
      history.replaceState(null, "", url.pathname);
    }
    return { id, identityVerificationToken: supplied ?? sessionStorage.getItem(key) ?? "" };
  });
  const frame = useRef<HTMLIFrameElement>(null);
  useEffect(() => {
    let port: MessagePort | undefined;
    const ready = (event: MessageEvent) => {
      if (
        event.source !== frame.current?.contentWindow ||
        event.origin !== "null" ||
        event.data?.type !== "tilde.identity.ready"
      )
        return;
      port?.close();
      const channel = new MessageChannel();
      port = channel.port1;
      port.onmessage = async ({ data }) => {
        if (!data || !Number.isSafeInteger(data.id)) return;
        try {
          const response =
            data.method === "read"
              ? await client.getIdentityVerification(credentials, { timeoutMs: 10000 })
              : data.method === "approve"
                ? await client.approveIdentityVerification(credentials, { timeoutMs: 10000 })
                : undefined;
          if (!response) throw new Error("Unsupported verification action");
          port?.postMessage({ id: data.id, verification: response.verification });
        } catch (error) {
          port?.postMessage({
            id: data.id,
            error: error instanceof Error ? error.message : "Verification failed.",
          });
        }
      };
      frame.current!.contentWindow!.postMessage({ type: "tilde.identity.port" }, "*", [
        channel.port2,
      ]);
    };
    window.addEventListener("message", ready);
    return () => {
      port?.close();
      window.removeEventListener("message", ready);
    };
  }, [credentials]);
  return (
    <iframe
      ref={frame}
      title="Approve identity access"
      src="/catalog/_identity/ui"
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      className="h-dvh w-full border-0"
    />
  );
}

/** The same recipient approval form is used for every provider. GET never changes access. */
export function IdentityVerificationForm() {
  const [verification, setVerification] = useState<IdentityVerification>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const port = useRef<MessagePort | null>(null);
  const pending = useRef(
    new Map<number, (response: { verification?: IdentityVerification; error?: string }) => void>(),
  );
  const sequence = useRef(0);
  function run(method: "read" | "approve") {
    if (!port.current) return;
    setBusy(true);
    setError("");
    const id = ++sequence.current;
    pending.current.set(id, (response) => {
      setBusy(false);
      if (response.error) setError(response.error);
      else setVerification(response.verification);
    });
    port.current.postMessage({ id, method });
  }
  useEffect(() => {
    const connect = (event: MessageEvent) => {
      if (
        event.source !== window.parent ||
        event.data?.type !== "tilde.identity.port" ||
        !event.ports[0]
      )
        return;
      port.current?.close();
      port.current = event.ports[0];
      port.current.onmessage = ({ data }) => {
        pending.current.get(data.id)?.(data);
        pending.current.delete(data.id);
      };
      run("read");
    };
    window.addEventListener("message", connect);
    window.parent.postMessage({ type: "tilde.identity.ready" }, "*");
    return () => {
      window.removeEventListener("message", connect);
      port.current?.close();
      pending.current.clear();
    };
  }, []);
  useEffect(() => {
    if (verification?.status !== "pending") return;
    const timer = setTimeout(() => run("read"), 2000);
    return () => clearTimeout(timer);
  }, [verification]);
  return (
    <LoadingReveal loading={!verification && !error} label="Loading identity verification">
      <main className="mx-auto max-w-lg space-y-5 p-6">
        <header className="flex items-center gap-3">
          <ProviderIcon iconUrl={verification?.iconUrl} />
          <h1 className="text-xl font-semibold">Approve identity access</h1>
        </header>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        {verification &&
          (verification.status === "approved" ? (
            <p role="status">
              Approved. You can now contact {verification.agentName} through{" "}
              {verification.accountName}.
            </p>
          ) : (
            <>
              <p>
                Allow <strong>{verification.value}</strong> to contact{" "}
                <strong>{verification.agentName}</strong> through {verification.accountName}?
              </p>
              <p className="text-sm text-muted-foreground">
                {verification.providerName} · No Tilde login required. Only approve if you requested
                this access.
              </p>
              <Button
                disabled={busy || verification.status !== "delivered"}
                onClick={() => run("approve")}
              >
                {busy ? "Approving…" : "Approve"}
              </Button>
            </>
          ))}
        {error && (
          <Button variant="outline" onClick={() => run("read")}>
            Retry
          </Button>
        )}
      </main>
    </LoadingReveal>
  );
}
