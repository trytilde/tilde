/**
 * Connection setup's visual preview: the standard setup page in frames, each handed a theme the
 * way Tilde's setup page hands a host's theme to a provider page (`tilde.setup.port`), with a
 * stand-in broker answering from a fixture. Dev only; served by `task preview:connection-setup`
 * at /connection-setup-preview.html. With `?page` this same entry is the framed page.
 */
import { StrictMode, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { AuthDriver, StandardSetup, mount, type Theme } from "@trytilde/connection-ui";
import { dark, light, withCanvas } from "../chat-ui-preview/themes";

// An OAuth app setup with both URLs to copy and steps that refer to them.
const setup = {
  providerName: "Linear",
  connectionName: "Linear",
  accountNameLabel: "Linear workspace",
  instructions:
    "Connect Linear so agents can read, comment on and move issues, and so routines can start when issues or comments change.",
  setupInstructions: [
    "In Linear, go to Settings > API > OAuth applications and create an application for this connection.",
    "Add the redirect URL below to its callback URLs.",
    "For routines, turn on webhooks, paste the webhook URL below and select Issues and Issue comments.",
    "Save, then copy the client ID, client secret and (with webhooks) signing secret into the fields below.",
    "Continue, and approve the read and write access Linear asks for.",
  ],
  webhookUrl: "https://tilde.example/connections/webhooks/conn_123",
  actionId: "action",
  authDriver: AuthDriver.OAUTH_CODE,
  draft: [],
  inputSchemaJson: JSON.stringify({
    type: "object",
    required: ["client_id", "client_secret"],
    properties: {
      client_id: { type: "string", title: "Client ID", minLength: 1 },
      client_secret: { type: "string", title: "Client secret", writeOnly: true, minLength: 1 },
      webhook_secret: {
        type: "string",
        title: "Webhook signing secret",
        description: "Optional. The signing secret of the Linear webhook.",
        writeOnly: true,
      },
    },
  }),
  action: { case: "form", value: {} },
};

function Frame({ label, theme }: { label: string; theme?: Theme }) {
  const frame = useRef<HTMLIFrameElement>(null);
  useEffect(() => {
    const ready = (event: MessageEvent) => {
      const page = frame.current?.contentWindow;
      if (!page || event.source !== page || event.data?.type !== "tilde.setup.ready") return;
      const { port1, port2 } = new MessageChannel();
      port1.onmessage = ({ data }) =>
        port1.postMessage({
          id: data.id,
          state:
            data.method === "startOAuth"
              ? { ...setup, action: { case: "complete", value: {} } }
              : setup,
        });
      page.postMessage({ type: "tilde.setup.port", theme }, "*", [port2]);
    };
    window.addEventListener("message", ready);
    return () => window.removeEventListener("message", ready);
  }, [theme]);
  return (
    <section style={{ display: "grid", gap: 8 }}>
      <h2 style={{ margin: 0, font: "550 12px system-ui", color: "#888" }}>{label}</h2>
      <iframe
        ref={frame}
        title={label}
        src="/connection-setup-preview.html?page"
        style={{ width: 460, height: 760, border: "1px solid #8884", borderRadius: 16 }}
      />
    </section>
  );
}

if (location.search === "?page") {
  await import("../../connection-ui.css");
  mount(StandardSetup);
} else {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <main style={{ display: "flex", flexWrap: "wrap", gap: 24, padding: 24 }}>
        <Frame label="No theme" />
        <Frame label="Example theme · light" theme={light} />
        <Frame label="Example theme · dark" theme={dark} />
        <Frame label="Example theme · dark, its own canvas" theme={withCanvas(dark, "#14100e")} />
      </main>
    </StrictMode>,
  );
}
