import { createFileRoute } from "@tanstack/react-router";

// The parent page owns tab state.
export const Route = createFileRoute("/_app/registry/sandboxes/$blueprintId/settings")({});
