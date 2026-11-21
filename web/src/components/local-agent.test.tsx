import { create } from "@bufbuild/protobuf";
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AgentSchema, Harness } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { AgentEditor } from "./agent-editor";

vi.mock("@/client", () => ({ agents: {}, tildeChat: {} }));
afterEach(cleanup);

function tabs() {
  return within(screen.getByRole("tablist", { name: "Agent settings" }))
    .getAllByRole("tab")
    .map((tab) => tab.textContent);
}
const setup = () => screen.getByRole("region", { name: "Local agent setup" }).textContent;

it("a local agent shows its setup and only what its harness supports", () => {
  const coding = create(AgentSchema, {
    id: "agent-cc",
    name: "Claude Code",
    local: {
      harness: Harness.CLAUDE_CODE,
      features: { sessions: true, traces: true, tools: true, skills: true },
    },
  });
  render(<AgentEditor agent={coding} onClose={vi.fn()} onSaved={vi.fn()} />);
  // No capabilities, inference, chat providers, prompts, routines, IAM, logs or deployment.
  expect(tabs()).toEqual(["Setup", "Skills", "Tools", "Sessions", "Tracing"]);
  expect(setup()).toContain("tilde local install claude-code --url ");
  expect(setup()).toContain("--agent-id agent-cc");
  cleanup();

  // A desktop app keeps its conversations in the vendor's account: it only takes tools, and
  // ChatGPT only as a remote MCP server.
  const desktop = create(AgentSchema, {
    id: "agent-gpt",
    name: "ChatGPT",
    local: { harness: Harness.CHATGPT_DESKTOP, features: { tools: true } },
  });
  render(<AgentEditor agent={desktop} onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(tabs()).toEqual(["Setup", "Tools"]);
  expect(setup()).toContain("/agents/agent-gpt/mcp");
  expect(setup()).not.toContain("tilde local install");
  cleanup();

  // A link to a tab the harness does not have opens the setup, and mounts nothing of that tab.
  const onTabChange = vi.fn();
  render(
    <AgentEditor
      agent={desktop}
      tab="prompts"
      onTabChange={onTabChange}
      onClose={vi.fn()}
      onSaved={vi.fn()}
    />,
  );
  expect(onTabChange).toHaveBeenCalledWith("capabilities");
  expect(screen.queryByRole("tabpanel")).toBeNull();
});
