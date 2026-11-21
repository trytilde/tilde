import { createRootRoute, Link, Outlet, redirect } from "@tanstack/react-router";

/** The registry's pages before they moved below `/registry`; old links redirect there. */
const movedToRegistry = /^\/(agent|skills|tools|sandboxes|memory|wikis)(\/|$)/;

export const Route = createRootRoute({
  beforeLoad: ({ location }) => {
    if (!movedToRegistry.test(location.pathname)) return;
    throw redirect({
      to: `/registry${location.pathname}` as "/registry",
      search: location.search as never,
      hash: location.hash,
      replace: true,
    });
  },
  component: Outlet,
  notFoundComponent: () => (
    <main className="space-y-4 p-8">
      <h1>Page not found</h1>
      <Link to="/registry">Back to agents</Link>
    </main>
  ),
});
