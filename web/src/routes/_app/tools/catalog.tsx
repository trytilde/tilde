import { createFileRoute } from "@tanstack/react-router";
import { ToolsPage } from "@/components/tool-connections";
// The catalog stays mounted while a provider's panel opens at its child path.
export const Route = createFileRoute("/_app/tools/catalog")({ component: ToolsPage });
