import { createFileRoute } from "@tanstack/react-router";
import { SkillDetail } from "@/components/skill-detail";
export const Route = createFileRoute("/_app/skills_/$skillId")({ component: Page });
function Page() {
  const { skillId } = Route.useParams();
  return <SkillDetail id={skillId} />;
}
