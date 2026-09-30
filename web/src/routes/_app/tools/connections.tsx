import { createFileRoute } from "@tanstack/react-router";
import { ConnectionsPage } from "@/components/connections-page";
export const Route = createFileRoute("/_app/tools/connections")({ component: ConnectionsPage });
