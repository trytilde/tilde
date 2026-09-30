import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { CatalogGroupSchema } from "@trytilde/contracts/tilde/types/v1/skill_pb.js";
import { SkillCatalogPage } from "./skill-catalog";

const rpc = vi.hoisted(() => ({
  skills: { listCatalog: vi.fn(), getCatalogGroup: vi.fn(), enableCatalogGroup: vi.fn() },
}));
vi.mock("@/client", () => rpc);

const runtime = create(CatalogGroupSchema, {
  id: "tilde-runtime",
  name: "Tilde runtime",
  description: "How agents use Tilde.",
  category: "tilde",
  skills: [{ name: "tilde-channels", description: "Reply on the channel." }],
  sourceId: "src-runtime",
});
const cloudflare = create(CatalogGroupSchema, {
  id: "cloudflare",
  name: "Cloudflare",
  description: "Workers, D1 and R2.",
  category: "cloud",
  iconUrl: "/skill-provider-icons/cloudflare.svg",
  repositoryUrl: "https://github.com/cloudflare/skills",
  branch: "main",
});
const stripe = create(CatalogGroupSchema, {
  id: "stripe",
  name: "Stripe",
  description: "Payments.",
  category: "payments",
  repositoryUrl: "https://github.com/stripe/ai",
});

function renderPage() {
  const root = createRootRoute({ component: Outlet });
  const catalog = createRoute({
    getParentRoute: () => root,
    path: "/skills/catalog",
    component: SkillCatalogPage,
  });
  const panel = createRoute({
    getParentRoute: () => catalog,
    path: "$groupId",
    component: () => null,
  });
  const source = createRoute({
    getParentRoute: () => root,
    path: "/skills",
    component: () => <p>Skills page</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([catalog.addChildren([panel]), source]),
    history: createMemoryHistory({ initialEntries: ["/skills/catalog"] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
beforeEach(() => {
  rpc.skills.listCatalog.mockResolvedValue({ groups: [stripe, runtime, cloudflare] });
  rpc.skills.getCatalogGroup.mockImplementation(({ id }: { id: string }) =>
    Promise.resolve({ group: [stripe, runtime, cloudflare].find((group) => group.id === id) }),
  );
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("sections groups by category and filters them by category and search", async () => {
  renderPage();
  const list = await screen.findByRole("region", { name: "Skill groups" });
  await within(list).findByText("Cloudflare");
  const headings = within(list)
    .getAllByRole("heading", { level: 3 })
    .filter((heading) => heading.id);
  expect(headings.map((heading) => heading.textContent)).toEqual(["Cloud", "Payments", "Tilde"]);
  // Enabled groups are marked; the provider's icon is its own.
  const enabled = within(list).getByRole("button", { name: /^Tilde runtime/ });
  expect(within(enabled).getByText("Enabled")).toBeTruthy();
  expect(
    within(list)
      .getByRole("button", { name: /^Cloudflare/ })
      .querySelector("img")
      ?.getAttribute("src"),
  ).toBe("/skill-provider-icons/cloudflare.svg");
  expect(
    within(within(list).getByRole("button", { name: /^Stripe/ })).queryByText("Enabled"),
  ).toBeNull();

  // Search covers a group's skills as well as its own text.
  fireEvent.change(screen.getByPlaceholderText("Search catalog"), {
    target: { value: "channels" },
  });
  expect(within(list).getByText("Tilde runtime")).toBeTruthy();
  expect(within(list).queryByText("Cloudflare")).toBeNull();

  fireEvent.change(screen.getByPlaceholderText("Search catalog"), { target: { value: "" } });
  fireEvent.click(screen.getByRole("button", { name: "Category" }));
  fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Payments" }));
  await waitFor(() => expect(within(list).queryByText("Cloudflare")).toBeNull());
  expect(within(list).getByText("Stripe")).toBeTruthy();

  fireEvent.change(screen.getByPlaceholderText("Search catalog"), { target: { value: "nothing" } });
  expect(await within(list).findByText("No skill groups match these filters.")).toBeTruthy();
});

it("previews a provider's skills at its own path, enables it and links to the new group", async () => {
  const router = renderPage();
  // Before enabling, the provider's skills are previewed from its repository.
  rpc.skills.getCatalogGroup.mockResolvedValueOnce({
    group: {
      ...cloudflare,
      skills: [
        { name: "workers", description: "Build Workers.", path: "skills/workers" },
        { name: "d1", description: "", path: "skills/d1" },
      ],
    },
  });
  fireEvent.click(await screen.findByRole("button", { name: /^Cloudflare/ }));
  const panel = await screen.findByRole("dialog", { name: "Cloudflare" });
  expect(router.state.location.pathname).toBe("/skills/catalog/cloudflare");
  expect(
    within(panel)
      .getByRole("link", { name: /github\.com\/cloudflare\/skills/ })
      .getAttribute("href"),
  ).toBe("https://github.com/cloudflare/skills");
  const preview = await within(panel).findByRole("region", { name: "Skills" });
  const rows = within(preview).getAllByRole("row").slice(1);
  expect(rows.map((row) => row.textContent)).toEqual([
    "workersBuild Workers.skills/workers",
    "d1—skills/d1",
  ]);
  expect(
    within(panel).getByText(/Preview from github\.com\/cloudflare\/skills @ main/),
  ).toBeTruthy();
  expect(rpc.skills.getCatalogGroup).toHaveBeenCalledWith({ id: "cloudflare" }, expect.anything());

  rpc.skills.enableCatalogGroup.mockResolvedValue({ source: { id: "src-cf" } });
  rpc.skills.listCatalog.mockResolvedValue({
    groups: [stripe, runtime, { ...cloudflare, sourceId: "src-cf" }],
  });
  fireEvent.click(within(panel).getByRole("button", { name: "Enable" }));
  await waitFor(() =>
    expect(rpc.skills.enableCatalogGroup).toHaveBeenCalledWith({ group: "cloudflare" }),
  );
  // Enabling lists the group's own skills; its first sync is still running, so the panel
  // says so and links to the Skills page, where the group now is.
  await within(panel).findByText(/Syncing: the repository's skills appear here/);
  expect(rpc.skills.getCatalogGroup).toHaveBeenCalledTimes(2);
  fireEvent.click(within(panel).getByRole("link", { name: "Open skills" }));
  await screen.findByText("Skills page");
});

it("lists a built-in group's skills up front and closes back to the catalog", async () => {
  const router = renderPage();
  fireEvent.click(await screen.findByRole("button", { name: /^Tilde runtime/ }));
  const panel = await screen.findByRole("dialog", { name: "Tilde runtime" });
  const listed = await within(panel).findByRole("region", { name: "Skills" });
  expect(within(listed).getByText("tilde-channels")).toBeTruthy();
  expect(within(panel).queryByRole("button", { name: "Enable" })).toBeNull();
  expect(within(panel).getByRole("link", { name: "Open skills" })).toBeTruthy();
  fireEvent.click(within(panel).getByRole("button", { name: "Close" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/skills/catalog"));
});

it("shows why a provider's preview failed", async () => {
  rpc.skills.getCatalogGroup.mockRejectedValue(
    new Error("GitHub rate limited the sync; configure ENGINE_GITHUB_TOKEN"),
  );
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: /^Stripe/ }));
  const panel = await screen.findByRole("dialog", { name: "Stripe" });
  expect((await within(panel).findByRole("alert")).textContent).toMatch(/rate limited/);
  expect(within(panel).queryByRole("region", { name: "Skills" })).toBeNull();
});
