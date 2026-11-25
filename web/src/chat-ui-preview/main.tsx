/**
 * chat-ui's visual preview: the live components against in-browser fixtures, in light, dark, a
 * host's theme (both schemes, a per-surface override, the chat without its header) and at phone
 * width, for side-by-side comparison with Dispatch's Chat stories. Dev only; served by
 * `task preview:chat-ui` at /chat-ui-preview.html.
 */
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Chat, ListSessions, type Theme } from "@trytilde/chat-ui";
import "../style.css";
import { PROXY, installMockProxy } from "./mock";
import { dark, light, withCanvas } from "./themes";

installMockProxy();

function Frame({
  label,
  initial,
  width,
  theme,
  chatTheme = theme,
}: {
  label: string;
  initial: [string, string];
  width?: number;
  theme?: Theme;
  /** The chat's own theme, where it differs from the session list's. */
  chatTheme?: Theme;
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
          theme={chatTheme}
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
      <Frame label="Example theme · light" initial={["sev2", "triage"]} theme={light} />
      <Frame
        label="Example theme · dark (on a light page)"
        initial={["sev2", "triage"]}
        theme={dark}
      />
      <Frame
        label="Example theme · light, the chat on its own canvas"
        initial={["sev2", "triage"]}
        theme={light}
        chatTheme={withCanvas(light, "#ffffff")}
      />
      <section style={{ display: "grid", gap: 8 }}>
        <h2 style={{ margin: 0, font: "550 12px system-ui", color: "#888" }}>
          Example theme · dark, transcript and message bar only (header=false)
        </h2>
        <div style={{ width: 480, height: 560, borderRadius: 16, overflow: "hidden" }}>
          <Chat
            sessionId="sev2"
            agentId="triage"
            identity="maya"
            proxy={PROXY}
            theme={dark}
            header={false}
          />
        </div>
      </section>
      <div style={{ display: "flex", gap: 24, flexWrap: "wrap" }}>
        <Frame label="390px · working with a queue" initial={["launch", "writer"]} width={390} />
        <Frame label="390px · group chat" initial={["standup", "triage"]} width={390} />
      </div>
    </main>
  </StrictMode>,
);
