/// <reference types="node" />
import { readFileSync } from "node:fs";
import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeAll, expect, it } from "vitest";
import { Transcript, type TranscriptEntry } from "../../sdk/ts/packages/chat-ui/src/index";

// The real stylesheet: the defaults a theme has to beat, and the rules that read the variables.
const css = readFileSync("../sdk/ts/packages/chat-ui/src/style.css", "utf8");
beforeAll(() => {
  const style = document.createElement("style");
  style.textContent = css;
  document.head.appendChild(style);
});
afterEach(cleanup);

const source = { sourceId: "s", timestamp: "2026-11-24T10:00:00Z", timestampLabel: "10:00" };
const entries: TranscriptEntry[] = [
  { ...source, id: "1", kind: "message", role: "user", text: "Where is my order?" },
  { ...source, id: "2", kind: "message", role: "assistant", text: "On its way." },
];
/** The variables the page and the two bubbles are painted with, as each element computes them. */
function painted(container: HTMLElement) {
  const value = (selector: string, name: string) =>
    getComputedStyle(container.querySelector(selector)!).getPropertyValue(name);
  return {
    scheme: getComputedStyle(container.querySelector(".tilde-chat")!).colorScheme,
    page: value(".tilde-chat", "--page"),
    ink: value(".tilde-chat", "--ink"),
    hover: value(".tilde-chat", "--hover"),
    user: value(".message.user", "--bubble-user"),
    userInk: value(".message.user", "--bubble-user-ink"),
    agent: value(".message:not(.user)", "--bubble-agent"),
  };
}

it("paints the page and bubbles from a theme's tokens, and keeps its own palette without one", () => {
  // The rules under test: if the stylesheet stops reading these, the assertions mean nothing.
  expect(css).toMatch(/background: var\(--bubble-user\)/);
  expect(css).toMatch(/background: var\(--bubble-agent\)/);
  expect(css).toMatch(/background: var\(--page\)/);
  // Every hover wash reads the palette: the attach button, the attach menu's rows, and no rule
  // paints the default grey itself.
  expect(css).toMatch(
    /\.attach-button:hover \{\s+color: var\(--ink\);\s+background: var\(--hover\)/,
  );
  expect(css).toMatch(/\.composer-attachment-menu > button:hover \{\s+background: var\(--hover\)/);
  expect(css).not.toMatch(/background: #77777717/);

  const plain = painted(render(<Transcript entries={entries} />).container);
  expect(plain).toMatchObject({ scheme: "light", user: "#070707", agent: "#eeeeee" });
  cleanup();

  // A dark scheme needs no `.dark` ancestor; tokens it does not name keep the dark defaults.
  const dark = painted(
    render(
      <Transcript entries={entries} theme={{ scheme: "dark", tokens: { accent: "#89c0ff" } }} />,
    ).container,
  );
  expect(dark).toMatchObject({
    scheme: "dark",
    page: "#070707",
    agent: "#262626",
    user: "#89c0ff",
  });
  expect(dark.userInk).toContain("#89c0ff");
  // Without an ink, the hover wash stays the dark default.
  expect(dark.hover).toBe("#7777773d");
  cleanup();

  // A light theme inside a dark page stays light, with every colour it names.
  document.documentElement.classList.add("dark");
  try {
    const themed = painted(
      render(
        <Transcript
          entries={entries}
          theme={{
            scheme: "light",
            tokens: { canvas: "#fffaf0", surface: "#f1e9da", ink: "#3e2723", accent: "#0038ae" },
          }}
        />,
      ).container,
    );
    expect(themed).toMatchObject({
      scheme: "light",
      page: "#fffaf0",
      ink: "#3e2723",
      agent: "#f1e9da",
      user: "#0038ae",
    });
    expect(themed.hover).toBe("color-mix(in srgb, #3e2723 8%, transparent)");
    cleanup();
    expect(painted(render(<Transcript entries={entries} />).container)).toMatchObject({
      scheme: "dark",
      page: "#070707",
      agent: "#262626",
      user: "#5a5a5a",
    });
  } finally {
    document.documentElement.classList.remove("dark");
  }
});
