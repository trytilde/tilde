import { createFileRoute } from "@tanstack/react-router";
import { SandboxesPage } from "@/components/sandboxes";
export const Route = createFileRoute("/_app/registry/sandboxes/")({ component: SandboxesPage });
