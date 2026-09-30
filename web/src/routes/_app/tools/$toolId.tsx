import { createFileRoute } from "@tanstack/react-router";
import { ToolConnectionDetail } from "@/components/tool-connection-detail";
export const Route = createFileRoute("/_app/tools/$toolId")({
  validateSearch: (search: Record<string, unknown>) => ({
    kind: search.kind === "host" ? ("host" as const) : ("connection" as const),
  }),
  component: () => {
    const { toolId } = Route.useParams();
    const { kind } = Route.useSearch();
    return <ToolConnectionDetail id={toolId} kind={kind} />;
  },
});
