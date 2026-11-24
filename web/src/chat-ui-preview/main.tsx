/**
 * chat-ui's visual preview: the live components against in-browser fixtures, in light, dark, a
 * host's theme and at phone width, for side-by-side comparison with Dispatch's Chat stories. Dev only; served by
 * `task preview:chat-ui` at /chat-ui-preview.html.
 */
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Chat, ListSessions, type Theme } from "@trytilde/chat-ui";
import "../style.css";
import { PROXY, installMockProxy } from "./mock";

installMockProxy();

function Frame({
  label,
  initial,
  width,
  theme,
}: {
  label: string;
  initial: [string, string];
  width?: number;
  theme?: Theme;
}) {
  const [[session, agent], select] = useState(initial);
  return (
    <section style={{ display: "grid", gap: 8 }}>
      <h2 style={{ margin: 0, font: "550 12px system-ui", color: "#888" }}>{label}</h2>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: width ? "1fr" : "280px 1fr",
          gridTemplateRows: width ? "260px 560px" : "640px",
          width: width ?? "100%",
          border: "1px solid #8884",
          borderRadius: 16,
          overflow: "hidden",
        }}
      >
        <ListSessions
          identity="maya"
          proxy={PROXY}
          selectedSessionId={session}
          selectedAgentId={agent}
          onSelectSession={(s, a) => select([s, a])}
          theme={theme}
        />
        <Chat
          key={session}
          sessionId={session}
          agentId={agent}
          identity="maya"
          proxy={PROXY}
          theme={theme}
        />
      </div>
    </section>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <main style={{ display: "grid", gap: 32, padding: 24, background: "#fff" }}>
      <Frame label="Light" initial={["sev2", "triage"]} />
      <div className="dark" style={{ background: "#070707", padding: 16, borderRadius: 20 }}>
        <Frame label="Dark" initial={["sev2", "triage"]} />
      </div>
      <Frame
        label="A host's theme (dark scheme on a light page)"
        initial={["sev2", "triage"]}
        theme={{
          scheme: "dark",
          tokens: {
            canvas: "#211714",
            surface: "#30211d",
            ink: "#faf9f5",
            muted: "#b9b1ab",
            line: "color-mix(in srgb, #faf9f5 18%, transparent)",
            accent: "#89c0ff",
            font: '"Geist Variable", sans-serif',
          },
        }}
      />
      <div style={{ display: "flex", gap: 24, flexWrap: "wrap" }}>
        <Frame label="390px · working with a queue" initial={["launch", "writer"]} width={390} />
        <Frame label="390px · group chat" initial={["standup", "triage"]} width={390} />
      </div>
    </main>
  </StrictMode>,
);
