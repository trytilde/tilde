import { createFileRoute } from "@tanstack/react-router";
import { SandboxesPage } from "@/components/sandboxes";
export const Route = createFileRoute("/_app/sandboxes/")({ component: SandboxesPage });
