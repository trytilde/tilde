import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { BLUEPRINT_TABS, SandboxBlueprintPage } from "@/components/sandbox-blueprint";

export const Route = createFileRoute("/_app/registry/sandboxes/$blueprintId")({
  component: BlueprintRoute,
});

const tabPaths = {
  settings: "/registry/sandboxes/$blueprintId/settings",
  environment: "/registry/sandboxes/$blueprintId/environment",
  egress: "/registry/sandboxes/$blueprintId/egress",
  tools: "/registry/sandboxes/$blueprintId/tools",
  sandboxes: "/registry/sandboxes/$blueprintId/sandboxes",
} as const;
function BlueprintRoute() {
  const { blueprintId } = Route.useParams();
  const tab = useRouterState({
    select: (state) =>
      BLUEPRINT_TABS.find((tab) =>
        state.matches.some(
          (match) => match.routeId === `/_app/registry/sandboxes/$blueprintId/${tab}`,
        ),
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
