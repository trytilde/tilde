import { render } from "@testing-library/react";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { WikiPage, type WikiTab } from "./wiki";

/** A wiki's page under a router whose `?page=` holds the open page, as the app's route does. */
export function renderWiki(wikiId: string, tab: WikiTab, url = `/wikis/${wikiId}/pages`) {
  const root = createRootRoute();
  const pages = createRoute({
    getParentRoute: () => root,
    path: "/wikis/$wikiId/pages",
    validateSearch: (search: Record<string, unknown>): { page?: string } => ({
      page: typeof search.page === "string" ? search.page : undefined,
    }),
    component: function Pages() {
      const { page } = pages.useSearch();
      const navigate = pages.useNavigate();
      return (
        <WikiPage
          wikiId={wikiId}
          tab={tab}
          onTabChange={() => {}}
          page={page}
          onPageChange={(path) => void navigate({ search: { page: path } })}
        />
      );
    },
  });
  const router = createRouter({
    routeTree: root.addChildren([pages]),
    history: createMemoryHistory({ initialEntries: [url] }),
  });
  return render(<RouterProvider router={router} />);
}
