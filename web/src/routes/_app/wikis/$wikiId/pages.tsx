import { createFileRoute } from "@tanstack/react-router";

// The parent page owns tab state; `page` is the open page's path, so pages have URLs.
export const Route = createFileRoute("/_app/wikis/$wikiId/pages")({
  validateSearch: (search: Record<string, unknown>): { page?: string } => ({
    page: typeof search.page === "string" && search.page ? search.page : undefined,
  }),
});
