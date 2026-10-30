import { createFileRoute } from "@tanstack/react-router";
import { MemoryPage } from "@/components/memory";
export const Route = createFileRoute("/_app/memory/")({ component: MemoryPage });
