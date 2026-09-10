import { Button } from "./components/ui/button";
import { useEffect, useRef, useState } from "react";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ConnectionSetupService, type Brokering } from "./gen/tilde/setup/v1/connections_pb.js";

const client = createClient(
  ConnectionSetupService,
  createConnectTransport({ baseUrl: window.location.origin }),
);
// The frame receives only setup data and an allowlisted MessagePort, never a management credential.
export function BrokeringPage({ setupId }: { setupId: string }) {
  const [connectionSetupToken] = useState(() => {
    const url = new URL(window.location.href);
    const supplied = url.searchParams.get("connection_setup_token");
    const key = `tilde:connection_setup_token:${setupId}`;
    if (supplied) {
      sessionStorage.setItem(key, supplied);
      url.searchParams.delete("connection_setup_token");
      history.replaceState(null, "", url.pathname + url.search);
    }
    return supplied ?? sessionStorage.getItem(key) ?? "";
  });
  const [state, setState] = useState<Brokering>();
  const [frameReady, setFrameReady] = useState(false);
  const [frameError, setFrameError] = useState(false);
  useEffect(() => {
    if (!state?.uiPath || frameReady) {
      setFrameError(false);
      return;
    }
    const timer = setTimeout(() => setFrameError(true), 10000);
    return () => clearTimeout(timer);
  }, [state?.uiPath, frameReady]);
  const [error, setError] = useState("");
  const frame = useRef<HTMLIFrameElement>(null);
  useEffect(() => {
    if (!connectionSetupToken) {
      setError("Missing connection setup token");
      return;
    }
    let active = true;
    void client
      .getSetup({ setupId, connectionSetupToken })
      .then((result) => {
        if (active) setState(result.state);
      })
      .catch((error) => {
        if (active) setError(String(error));
      });
    return () => {
      active = false;
    };
  }, [setupId, connectionSetupToken]);
  useEffect(() => {
    let port: MessagePort | undefined;
    const connect = (event: MessageEvent) => {
      if (
        event.source !== frame.current?.contentWindow ||
        event.origin !== "null" ||
        event.data?.type !== "tilde.setup.ready"
      )
        return;
      setFrameReady(true);
      port?.close();
      const channel = new MessageChannel();
      port = channel.port1;
      port.onmessage = async ({ data }) => {
        if (!data || !Number.isSafeInteger(data.id)) return;
        try {
          let response;
          if (data.method === "read")
            response = await client.getSetup({ setupId, connectionSetupToken });
          else if (data.method === "cancel")
            response = await client.cancelSetup({ setupId, connectionSetupToken });
          else if (
            ["saveDraft", "saveCredentials", "startOAuth", "executeProviderAction"].includes(
              data.method,
            )
          ) {
            if (
              typeof data.actionId !== "string" ||
              !data.fields ||
              typeof data.fields !== "object" ||
              Array.isArray(data.fields)
            )
              throw new Error("Invalid setup submission");
            const entries = Object.entries(data.fields);
            if (
              entries.length > 100 ||
              entries.some(([k, v]) => k.length > 160 || typeof v !== "string" || v.length > 65536)
            )
              throw new Error("Invalid setup values");
            const request = {
              setupId,
              connectionSetupToken,
              actionId: data.actionId,
              fields: entries.map(([key, value]) => ({ key, value: value as string })),
            };
            if (data.method === "saveDraft")
              response = await client.saveDraft({
                setupId,
                connectionSetupToken,
                actionId: data.actionId,
                draft: request.fields,
              });
            else if (data.method === "saveCredentials")
              response = await client.saveCredentials(request);
            else if (data.method === "startOAuth") response = await client.startOAuth(request);
            else {
              if (typeof data.action !== "string" || data.action.length > 160)
                throw new Error("Invalid provider action");
              response = await client.executeProviderAction({ ...request, action: data.action });
            }
          } else throw new Error("Unsupported setup operation");
          setState(response.state);
          port?.postMessage({ id: data.id, state: response.state });
        } catch (error) {
          port?.postMessage({
            id: data.id,
            error: error instanceof Error ? error.message : "Setup failed",
          });
        }
      };
      // Opaque sandbox origins require '*'; transfer is restricted to this exact iframe window.
      frame.current!.contentWindow!.postMessage({ type: "tilde.setup.port" }, "*", [channel.port2]);
    };
    window.addEventListener("message", connect);
    return () => {
      window.removeEventListener("message", connect);
      port?.close();
    };
  }, [setupId, connectionSetupToken]);
  useEffect(() => {
    if (state?.action.case === "complete")
      window.opener?.postMessage(
        { type: "tilde.connection.complete", connectionId: state.connectionId },
        "*",
      );
  }, [state]);
  if (error && !state) return <p role="alert">{error}</p>;
  if (!state) return <p role="status">Loading connection setup…</p>;
  // A catalog adapter selects only an API-local compiled page. Runtime definitions cannot inject a URL.
  if (
    !/^(?:\/catalog\/[a-z0-9_-]+\/ui|\/connections\/ui\/remote\/[A-Za-z0-9_-]+\/index\.html)$/.test(
      state.uiPath,
    )
  )
    return <p role="alert">Invalid provider UI path</p>;
  if (state.action.case === "cancelled") return <p role="status">Connection setup cancelled.</p>;
  return (
    <>
      {(frameError || error) && (
        <p role="alert">
          {error || "The provider UI did not respond. You can reload this page or cancel setup."}
        </p>
      )}
      {!["complete", "failed"].includes(state.action.case ?? "") && (
        <div style={{ padding: 8 }}>
          <Button
            onClick={() => {
              void client
                .cancelSetup({ setupId, connectionSetupToken })
                .then((response) => setState(response.state))
                .catch((error) => setError(String(error)));
            }}
          >
            Cancel setup
          </Button>
        </div>
      )}
      <iframe
        ref={frame}
        title="Connection setup"
        src={state.uiPath}
        referrerPolicy="no-referrer"
        sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation-by-user-activation"
        style={{ width: "100%", height: "calc(100dvh - 48px)", border: 0, display: "block" }}
      />
    </>
  );
}
