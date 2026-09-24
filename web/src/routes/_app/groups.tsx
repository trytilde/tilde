import { createFileRoute } from "@tanstack/react-router";
import { GroupsPage } from "@/components/groups";
export const Route = createFileRoute("/_app/groups")({ component: GroupsPage });
