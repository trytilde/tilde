import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/sandboxes/$blueprintId/")({
  component: () => {
    const { blueprintId } = Route.useParams();
    return <Navigate to="/sandboxes/$blueprintId/settings" params={{ blueprintId }} replace />;
  },
});
