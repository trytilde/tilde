import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/registry/wikis/$wikiId/")({
  component: () => {
    const { wikiId } = Route.useParams();
    return <Navigate to="/registry/wikis/$wikiId/pages" params={{ wikiId }} replace />;
  },
});
