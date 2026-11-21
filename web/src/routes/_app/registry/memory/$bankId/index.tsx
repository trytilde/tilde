import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/registry/memory/$bankId/")({
  component: () => {
    const { bankId } = Route.useParams();
    return <Navigate to="/registry/memory/$bankId/memories" params={{ bankId }} replace />;
  },
});
