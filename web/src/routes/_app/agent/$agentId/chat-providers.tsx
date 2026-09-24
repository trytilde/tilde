import { createFileRoute } from "@tanstack/react-router";

// The parent editor owns tab state so drafts survive navigation.
export const Route = createFileRoute("/_app/agent/$agentId/chat-providers")({});
