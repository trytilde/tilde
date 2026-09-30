import { createFileRoute } from "@tanstack/react-router";
// A provider's panel: rendered by the catalog page, which reads `providerId`.
export const Route = createFileRoute("/_app/tools/catalog/$providerId")({ component: () => null });
