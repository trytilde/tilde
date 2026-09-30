import { createFileRoute } from "@tanstack/react-router";
import { SkillCatalogPage } from "@/components/skill-catalog";
// The catalog stays mounted while a group's panel opens at its child path.
export const Route = createFileRoute("/_app/skills_/catalog")({ component: SkillCatalogPage });
