import { createFileRoute } from "@tanstack/react-router";
import { SecretsPage } from "@/components/secrets-page";

// `?add=KEY` opens a new row for KEY with its value focused (tilde deploy links here).
export const Route = createFileRoute("/_app/connections/secrets")({
  validateSearch: (search: Record<string, unknown>): { add?: string; team?: string } => ({
    add: typeof search.add === "string" ? search.add : undefined,
    team: typeof search.team === "string" ? search.team : undefined,
  }),
  component: Page,
});
function Page() {
  const { add, team } = Route.useSearch();
  const navigate = Route.useNavigate();
  let notice = "";
  return (
    <SecretsPage
      notice={notice}
      add={notice ? undefined : add}
      onAdded={() => void navigate({ search: team ? { team } : {}, replace: true })}
    />
  );
}
