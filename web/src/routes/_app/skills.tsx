import { createFileRoute } from "@tanstack/react-router";
import { SkillsPage } from "@/components/skills-page";
export const Route = createFileRoute("/_app/skills")({ component: SkillsPage });
