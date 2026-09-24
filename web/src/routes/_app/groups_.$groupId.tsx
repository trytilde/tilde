import { createFileRoute } from "@tanstack/react-router";
import { GroupDetail } from "@/components/group-detail";

export const Route = createFileRoute("/_app/groups_/$groupId")({ component: GroupPage });

function GroupPage() {
  const { groupId } = Route.useParams();
  const navigate = Route.useNavigate();
  return <GroupDetail groupId={groupId} onDeleted={() => void navigate({ to: "/groups" })} />;
}
