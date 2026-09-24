import { createFileRoute } from "@tanstack/react-router";
import { ApiKeysPage } from "@/components/api-keys";
export const Route = createFileRoute("/_app/api-keys")({ component: Page });
function Page() {
  const navigate = Route.useNavigate();
  return <ApiKeysPage onCreate={() => void navigate({ to: "/api-keys/new" })} />;
}
