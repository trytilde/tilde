import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { WIKI_TABS, WikiPage } from "@/components/wiki";

export const Route = createFileRoute("/_app/wikis/$wikiId")({ component: WikiRoute });

const tabPaths = {
  pages: "/wikis/$wikiId/pages",
  settings: "/wikis/$wikiId/settings",
} as const;
function WikiRoute() {
  const { wikiId } = Route.useParams();
  const tab = useRouterState({
    select: (state) =>
      WIKI_TABS.find((tab) =>
        state.matches.some((match) => match.routeId === `/_app/wikis/$wikiId/${tab}`),
      ) ?? "pages",
  });
  const navigate = Route.useNavigate();
  const page = useRouterState({
    select: (state) => (state.location.search as { page?: string }).page,
  });
  return (
    <>
      <WikiPage
        key={wikiId}
        wikiId={wikiId}
        tab={tab}
        onTabChange={(next) => void navigate({ to: tabPaths[next], params: { wikiId } })}
        page={page}
        onPageChange={(path) =>
          void navigate({ to: "/wikis/$wikiId/pages", params: { wikiId }, search: { page: path } })
        }
      />
      <Outlet />
    </>
  );
}
