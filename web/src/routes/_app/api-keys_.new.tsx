import { createFileRoute } from "@tanstack/react-router";
import { ApiKeyCreate } from "@/components/api-key-create";

export const Route = createFileRoute("/_app/api-keys_/new")({ component: NewApiKeyPage });

function NewApiKeyPage() {
  const navigate = Route.useNavigate();
  return <ApiKeyCreate onDone={() => void navigate({ to: "/api-keys" })} />;
}
