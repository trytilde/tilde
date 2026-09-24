import { createFileRoute } from "@tanstack/react-router";
import { BrokeringPage } from "@/routes/connections/-connection-setup-host";

export const Route = createFileRoute("/connections/broker/$setupId")({ component: BrokerPage });

function BrokerPage() {
  return <BrokeringPage setupId={Route.useParams().setupId} />;
}
