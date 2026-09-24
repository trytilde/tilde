# @trytilde/chat-proxy

Server-side Web Request/Response reverse proxy for the Tilde ConnectRPC chat provider.

```ts
import { createChatProxy } from '@trytilde/chat-proxy';
const proxy = createChatProxy({
  apiKey: process.env.TILDE_CHAT_API_KEY!,
  agentId: process.env.TILDE_AGENT_ID!,
  baseUrl: 'https://api.trytilde.ai',
  resolveIdentity: async (request, agentId) => {
    const user = await yourSessionAuthentication(request);
    return user && await yourAgentAccessCheck(user, agentId) ? user.id : null;
  },
});
// Route /api/chat/* to proxy.handle(request).
```

Read the API key in the agent's Chat Providers → Tilde card. Keys authenticate your server application. `resolveIdentity` must derive a stable identity from verified server authentication, never from a browser identity field. Returning null denies that agent. Tilde scopes each identity to the agent and checks active session membership before search/pagination, history, attachment reads, subscriptions, and mutations. A newly created session contains the authenticated user and the configured agent. Existing participants can explicitly add/remove participants; an outsider cannot add themselves to a session.

`baseUrl` defaults to the hosted API. `baseUrlOverride` replaces the complete agent ingress base URL, including any `/agents/{id}` prefix; it is server configuration, never request input. `mountPath` defaults to `/api/chat`; adapters can instead supply catch-all path segments to `handle(request, path)`.

For a sidebar across agents, replace `apiKey`/`agentId` with `agents: [{agentId,apiKey,baseUrl?}, …]`. Each entry is independently authorized by `resolveIdentity`. `GET /api/chat/agents` returns only authorized agent names/IDs. Connect calls use `/api/chat/{agentId}/tilde.provider.tilde.v1.ChatService/{method}`. A single-agent proxy also accepts the service path directly.

Only declared provider RPC methods are forwarded. Browser credentials and identity headers are replaced; cookies stay at the host. Cross-origin browser requests are rejected. Streaming request/response bodies and cancellation pass through without buffering, responses disable caching, and redirects are rejected so credentials cannot follow an upstream redirect. Application authentication and upstream failures do not expose keys or internal error details.

The native wire identity header `x-tilde-identity` is base64url-encoded UTF-8; use this package or the SDK rather than setting it in browser code. API key rotation invalidates new requests immediately. Existing subscriptions use a signed, identity-scoped internal credential with a maximum 15-minute lifetime and recheck membership before delivering events; reconnects use the current server-side key.
