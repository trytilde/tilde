import { createRootRoute, Link, Outlet } from "@tanstack/react-router";

export const Route = createRootRoute({
  component: Outlet,
  notFoundComponent: () => (
    <main className="space-y-4 p-8">
      <h1>Page not found</h1>
      <Link to="/">Back to agents</Link>
    </main>
  ),
});
