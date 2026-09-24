# @trytilde/chat-next

Next.js App Router adapter for the native Tilde chat proxy. Uses Web Request/Response route handlers and asynchronous catch-all parameters, preserving Connect streaming and cancellation.

```ts
// app/api/chat/[...path]/route.ts (server-only)
import { createChatProxy, createChatRouteHandlers } from '@trytilde/chat-next';
import { auth } from '@/auth'; // Your application's server authentication.

const proxy = createChatProxy({
  apiKey: process.env.TILDE_CHAT_API_KEY!,
  agentId: process.env.TILDE_AGENT_ID!,
  // baseUrl: 'http://127.0.0.1:8080', // self-hosted gateway
  resolveIdentity: async () => {
    const session = await auth();
    return session?.user?.id ?? null;
  },
});
export const { GET, POST } = createChatRouteHandlers(proxy);
export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';
```

Mount `Chat` and `ListSessions` from `@trytilde/chat-ui` with `proxy="/api/chat"`. Keep the API key in this server route, never in a `NEXT_PUBLIC_` variable or component props. No Next runtime imports are needed in the framework-independent proxy or UI package. Multi-agent configuration and complete upstream URL overrides use the same proxy options.

Your hosting platform must allow streaming responses for the desired duration. The UI reconnects using the last opaque cursor after a connection ends. The adapter itself does not buffer responses or impose an artificial duration limit.
