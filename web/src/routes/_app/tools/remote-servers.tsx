import { createFileRoute } from "@tanstack/react-router";
import { RemoteServersPage } from "@/components/remote-servers";
export const Route = createFileRoute("/_app/tools/remote-servers")({
  component: RemoteServersPage,
});
