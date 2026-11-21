import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/registry/sandboxes/$blueprintId/")({
  component: () => {
    const { blueprintId } = Route.useParams();
    return (
      <Navigate to="/registry/sandboxes/$blueprintId/settings" params={{ blueprintId }} replace />
    );
  },
});
