import { createFileRoute, redirect } from "@tanstack/react-router";
// Tools has no page of its own: it opens on the connections in use.
export const Route = createFileRoute("/_app/tools/")({
  beforeLoad: () => {
    throw redirect({ to: "/tools/connections" });
  },
});
