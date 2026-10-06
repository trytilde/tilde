import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { BLUEPRINT_TABS, SandboxBlueprintPage } from "@/components/sandbox-blueprint";

export const Route = createFileRoute("/_app/sandboxes/$blueprintId")({ component: BlueprintRoute });

const tabPaths = {
  settings: "/sandboxes/$blueprintId/settings",
  environment: "/sandboxes/$blueprintId/environment",
  tools: "/sandboxes/$blueprintId/tools",
  sandboxes: "/sandboxes/$blueprintId/sandboxes",
} as const;
function BlueprintRoute() {
  const { blueprintId } = Route.useParams();
  const tab = useRouterState({
    select: (state) =>
      BLUEPRINT_TABS.find((tab) =>
        state.matches.some((match) => match.routeId === `/_app/sandboxes/$blueprintId/${tab}`),
      ) ?? "settings",
  });
  const navigate = Route.useNavigate();
  return (
    <>
      <SandboxBlueprintPage
        key={blueprintId}
        blueprintId={blueprintId}
        tab={tab}
        onTabChange={(next) => void navigate({ to: tabPaths[next], params: { blueprintId } })}
      />
      <Outlet />
    </>
  );
}
