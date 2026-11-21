import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { BANK_TABS, MemoryBankPage } from "@/components/memory-bank";

export const Route = createFileRoute("/_app/registry/memory/$bankId")({ component: BankRoute });

const tabPaths = {
  memories: "/registry/memory/$bankId/memories",
  settings: "/registry/memory/$bankId/settings",
} as const;
function BankRoute() {
  const { bankId } = Route.useParams();
  const tab = useRouterState({
    select: (state) =>
      BANK_TABS.find((tab) =>
        state.matches.some((match) => match.routeId === `/_app/registry/memory/$bankId/${tab}`),
      ) ?? "memories",
  });
  const navigate = Route.useNavigate();
  return (
    <>
      <MemoryBankPage
        key={bankId}
        bankId={bankId}
        tab={tab}
        onTabChange={(next) => void navigate({ to: tabPaths[next], params: { bankId } })}
      />
      <Outlet />
    </>
  );
}
