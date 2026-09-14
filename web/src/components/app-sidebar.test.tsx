import type { ReactNode } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Auth } from "./auth";
import { AppSidebar } from "./app-sidebar";
import { SidebarProvider } from "./ui/sidebar";

vi.mock("./sidebar-material", () => ({ default: () => null }));
vi.mock("@tanstack/react-router", () => ({
  useRouterState: () => "/",
  Link: ({ to, children, ...props }: { to: string; children: ReactNode }) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
}));
beforeEach(() => {
  localStorage.setItem("tilde.access_token", "test-access-token");
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.clear();
});
function Dashboard() {
  return (
    <Auth>
      <SidebarProvider>
        <AppSidebar />
        <p>Private dashboard</p>
      </SidebarProvider>
    </Auth>
  );
}
it("renders Sign out in the sidebar footer and closes the authenticated dashboard on success", async () => {
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(new Response(null, { status: 200 }))
    .mockResolvedValueOnce(new Response(null, { status: 204 }));
  vi.stubGlobal("fetch", fetch);
  render(<Dashboard />);
  const button = await screen.findByRole("button", { name: "Sign out" });
  expect(button.closest('[data-slot="sidebar-footer"]')).toBeTruthy();
  expect(screen.getAllByRole("button", { name: "Sign out" })).toHaveLength(1);
  fireEvent.click(button);
  await screen.findByRole("heading", { name: "Sign in to Tilde" });
  expect(fetch).toHaveBeenCalledWith("/auth/logout", {
    method: "POST",
    headers: { Authorization: "Bearer test-access-token" },
  });
  expect(localStorage.getItem("tilde.access_token")).toBeNull();
  expect(screen.queryByText("Private dashboard")).toBeNull();
});
it("keeps the session and offers retry when logout fails", async () => {
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(new Response(null, { status: 200 }))
    .mockResolvedValueOnce(new Response(null, { status: 500 }));
  vi.stubGlobal("fetch", fetch);
  render(<Dashboard />);
  fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
  await screen.findByText("Unable to sign out. Please try again.");
  expect(localStorage.getItem("tilde.access_token")).toBe("test-access-token");
  expect(screen.getByText("Private dashboard")).toBeTruthy();
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "Sign out" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
});
