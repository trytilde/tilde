vi.mock("@/components/agent-iam", () => ({ AgentIam: () => <input aria-label="IAM draft" /> }));
import type { ReactNode } from "react";
import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { Code, ConnectError } from "@connectrpc/connect";
import { AgentSchema } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { createAppRouter } from "./router";

const rpc = vi.hoisted(() => ({ listAgents: vi.fn(), getAgent: vi.fn(), updateAgent: vi.fn() }));
const auth = vi.hoisted(() => vi.fn());
vi.mock("@/client", () => ({ agents: rpc }));
vi.mock("@/components/auth", () => ({
  Auth: ({ children }: { children: ReactNode }) => {
    auth();
    return children;
  },
}));
vi.mock("@/components/app-sidebar", () => ({ AppSidebar: () => null }));
vi.mock("@/components/ui/sidebar", () => ({
  SidebarTrigger: () => null,
  SidebarProvider: ({ children }: { children: ReactNode }) => children,
  SidebarInset: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("@/components/agent-connections", () => ({
  AgentConnections: () => <p>Assigned connections</p>,
}));
vi.mock("@/routes/connections/-connection-setup-host", () => ({
  BrokeringPage: ({ setupId }: { setupId: string }) => <p>Public setup {setupId}</p>,
}));
const ada = create(AgentSchema, {
  id: "agent-ada",
  name: "Ada",
  endpointUrl: "https://ada.example.com",
});
beforeEach(() => {
  vi.stubGlobal("scrollTo", vi.fn());
  rpc.listAgents.mockResolvedValue({ agents: [ada], nextPageToken: "" });
  rpc.getAgent.mockResolvedValue({ agent: ada });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});
async function open(path: string) {
  const history = createMemoryHistory({ initialEntries: [path] });
  const router = createAppRouter(history);
  await act(async () => {
    await router.load();
  });
  render(<RouterProvider router={router} />);
  return { router, history };
}

it("opens agent rows at capabilities and routes tabs with working Back and Forward", async () => {
  const { router, history } = await open("/");
  fireEvent.click(await screen.findByRole("row", { name: "Edit Ada" }));
  await screen.findByRole("heading", { name: "Ada" });
  expect(router.state.location.pathname).toBe("/agent/agent-ada/capabilities");
  const header = screen.getByRole("navigation", { name: "Breadcrumb" }).closest("header")!;
  expect(within(header).getByRole("tablist", { name: "Agent settings" })).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "Edit agent" })).queryByRole("tablist", {
      name: "Agent settings",
    }),
  ).toBeNull();

  expect(rpc.getAgent).toHaveBeenCalledWith(
    { id: ada.id },
    expect.objectContaining({ signal: expect.any(AbortSignal) }),
  );
  expect(
    screen.queryByText(
      "All actions deny by default. Thread and work access stays within the invocation.",
    ),
  ).toBeNull();
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Agent settings" })).getByRole("tab", {
      name: "Chat providers",
    }),
  );
  await waitFor(() =>
    expect(router.state.location.pathname).toBe("/agent/agent-ada/chat-providers"),
  );
  expect(screen.getByRole("tabpanel").textContent).toContain("Assigned connections");
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Agent settings" })).getByRole("tab", {
      name: "IAM",
    }),
  );
  await waitFor(() => expect(router.state.location.pathname).toBe("/agent/agent-ada/iam"));
  fireEvent.change(screen.getByLabelText("IAM draft"), { target: { value: "retained" } });
  await act(async () => {
    history.back();
  });
  await waitFor(() =>
    expect(router.state.location.pathname).toBe("/agent/agent-ada/chat-providers"),
  );
  await act(async () => {
    history.forward();
  });
  await waitFor(() => expect(router.state.location.pathname).toBe("/agent/agent-ada/iam"));
  expect((screen.getByLabelText("IAM draft") as HTMLInputElement).value).toBe("retained");
  expect(rpc.getAgent).toHaveBeenCalledTimes(1);
});

it("loads a deep-linked agent tab without first listing agents", async () => {
  const { router } = await open("/agent/agent-ada/iam");
  await screen.findByRole("heading", { name: "Ada" });
  expect(
    within(screen.getByRole("tablist", { name: "Agent settings" }))
      .getByRole("tab", { name: "IAM" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  expect(rpc.listAgents).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Back to agents" })).toBeNull();
  const breadcrumb = screen.getByRole("navigation", { name: "Breadcrumb" });
  expect(within(breadcrumb).getByText("Ada").getAttribute("aria-current")).toBe("page");
  fireEvent.click(within(breadcrumb).getByRole("link", { name: "Agent Registry" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/"));
  expect(screen.queryByRole("tablist", { name: "Agent settings" })).toBeNull();
});

it("redirects the bare agent URL and shows a retryable missing-agent result", async () => {
  rpc.getAgent
    .mockRejectedValueOnce(new ConnectError("missing", Code.NotFound))
    .mockResolvedValue({ agent: ada });
  const { router } = await open("/agent/agent-ada");
  await screen.findByText("Agent not found.");
  expect(router.state.location.pathname).toBe("/agent/agent-ada/capabilities");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await screen.findByRole("heading", { name: "Ada" });
});

it("keeps connection brokering outside the authenticated app routes", async () => {
  await open("/connections/broker/example-setup");
  await screen.findByText("Public setup example-setup");
  expect(auth).not.toHaveBeenCalled();
});

it("matches agent creation before the dynamic agent route", async () => {
  await open("/agent/new");
  await screen.findByRole("heading", { name: "Create agent" });
  expect(auth).toHaveBeenCalled();
  expect(rpc.getAgent).not.toHaveBeenCalled();
});

it.each(["/connections", "/chat"])("no longer serves %s", async (path) => {
  await open(path);
  await screen.findByRole("heading", { name: "Page not found" });
});

it("returns from the auth callback to the registry", async () => {
  const { router } = await open("/auth/callback");
  await screen.findByRole("row", { name: "Edit Ada" });
  expect(router.state.location.pathname).toBe("/");
  expect(auth).toHaveBeenCalled();
});

it("offers navigation back to the registry for unknown URLs", async () => {
  const { router } = await open("/missing-page");
  await screen.findByRole("heading", { name: "Page not found" });
  fireEvent.click(screen.getByRole("link", { name: "Back to agents" }));
  await screen.findByRole("row", { name: "Edit Ada" });
  expect(router.state.location.pathname).toBe("/");
});

it("updates the agent breadcrumb after a successful rename without refetching", async () => {
  rpc.updateAgent.mockResolvedValue({ agent: { ...ada, name: "Ada Lovelace" } });
  await open("/agent/agent-ada/capabilities");
  await screen.findByRole("heading", { name: "Ada" });
  fireEvent.click(screen.getByRole("button", { name: "Edit agent name" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada Lovelace" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await within(screen.getByRole("navigation", { name: "Breadcrumb" })).findByText("Ada Lovelace");
  expect(rpc.getAgent).toHaveBeenCalledTimes(1);
});

it("keeps the header tabs linked to their panels and supports keyboard navigation", async () => {
  const { router } = await open("/agent/agent-ada/capabilities");
  await screen.findByRole("heading", { name: "Ada" });
  const tabs = within(screen.getByRole("tablist", { name: "Agent settings" }));
  const capabilities = tabs.getByRole("tab", { name: "Capabilities" });
  expect(
    document.getElementById(capabilities.getAttribute("aria-controls")!)?.getAttribute("role"),
  ).toBe("tabpanel");
  capabilities.focus();
  fireEvent.keyDown(capabilities, { key: "ArrowRight" });
  const chatTab = tabs.getByRole("tab", { name: "Chat providers" });
  await waitFor(() => expect(document.activeElement).toBe(chatTab));
  expect(router.state.location.pathname).toBe("/agent/agent-ada/capabilities");
  // Base UI uses manual activation. JSDOM fireEvent does not synthesize Enter's native click.
  fireEvent.keyDown(chatTab, { key: "Enter" });
  fireEvent.click(chatTab);
  await waitFor(() =>
    expect(router.state.location.pathname).toBe("/agent/agent-ada/chat-providers"),
  );
  const selected = tabs.getByRole("tab", { name: "Chat providers" });
  expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(selected.id);
});
