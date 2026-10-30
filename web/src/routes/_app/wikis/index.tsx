import { createFileRoute } from "@tanstack/react-router";
import { WikisPage } from "@/components/wikis";
export const Route = createFileRoute("/_app/wikis/")({ component: WikisPage });
