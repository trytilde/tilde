import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/memory/$bankId/")({
  component: () => {
    const { bankId } = Route.useParams();
    return <Navigate to="/memory/$bankId/memories" params={{ bankId }} replace />;
  },
});
