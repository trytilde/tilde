import type { ReactNode } from "react";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { createAppRouter } from "../router";

const rpc = vi.hoisted(() => ({
  listAgents: vi.fn().mockResolvedValue({ agents: [], nextPageToken: "" }),
  listGroups: vi.fn().mockResolvedValue({ groups: [] }),
  listApiKeys: vi.fn().mockResolvedValue({ apiKeys: [], nextPageToken: "" }),
}));
vi.mock("@/client", () => ({ agents: rpc, iam: rpc, apiKeys: rpc }));
vi.mock("@/hooks/use-caller", () => ({
  CallerProvider: ({ children }: { children: ReactNode }) => children,
  useCaller: () => ({ userId: "user-1", admin: true, groupIds: [], creatable: [1] }),
}));
vi.mock("@/components/auth", () => ({
  Auth: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("./sidebar-material", () => ({ default: () => null }));
afterEach(cleanup);

it("groups API keys and groups under an IAM entry that is never itself active", async () => {
  vi.stubGlobal("scrollTo", vi.fn());
  // The sidebar provider reads a media query for its mobile breakpoint.
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }),
  );
  const router = createAppRouter(createMemoryHistory({ initialEntries: ["/groups"] }));
  await act(async () => {
    await router.load();
  });
  render(<RouterProvider router={router} />);
  const iam = await screen.findByRole("button", { name: "IAM" });
  expect(iam.getAttribute("data-active")).toBeNull();
  expect(iam.querySelector("svg.lucide-chevron-right")).toBeTruthy();
  const sub = document.querySelector('[data-sidebar="menu-sub"]')!;
  expect(sub.className).toContain("border-sidebar-foreground");
  expect(within(sub as HTMLElement).getByRole("link", { name: "API keys" })).toBeTruthy();
  const groups = within(sub as HTMLElement).getByRole("link", { name: "Groups" });
  // Base UI marks state by attribute presence, so an active link carries an empty value.
  expect(groups.getAttribute("data-active")).not.toBeNull();
  expect(screen.queryByRole("link", { name: "Access" })).toBeNull();
  vi.unstubAllGlobals();
});
