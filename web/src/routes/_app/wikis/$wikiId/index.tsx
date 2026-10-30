import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/wikis/$wikiId/")({
  component: () => {
    const { wikiId } = Route.useParams();
    return <Navigate to="/wikis/$wikiId/pages" params={{ wikiId }} replace />;
  },
});
