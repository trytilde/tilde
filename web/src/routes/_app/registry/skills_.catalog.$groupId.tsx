import { createFileRoute } from "@tanstack/react-router";
// A catalog group's panel: rendered by the catalog page, which reads `groupId`.
export const Route = createFileRoute("/_app/registry/skills_/catalog/$groupId")({
  component: () => null,
});
