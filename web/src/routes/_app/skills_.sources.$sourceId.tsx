import { createFileRoute, redirect } from "@tanstack/react-router";

// Groups are managed from their rows on the Skills page; old links land there.
export const Route = createFileRoute("/_app/skills_/sources/$sourceId")({
  beforeLoad: () => {
    throw redirect({ to: "/skills" });
  },
});
