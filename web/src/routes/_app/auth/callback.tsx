import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/auth/callback")({
  component: () => <Navigate to="/" replace />,
});
