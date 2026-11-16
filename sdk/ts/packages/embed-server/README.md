# @trytilde/embed-server

The server half of embedding Tilde in your application: one handler that opens dashboards,
proxies agent chats and starts connection setup for **your** signed-in users. Pair it with
`@trytilde/embed` in the browser.

```ts
import { createEmbed } from "@trytilde/embed-server";
export const embed = createEmbed({
  apiKey: process.env.TILDE_EMBED_KEY!, // tilde_embed_..., from the project's Embed tab
  agents: ["<agent id>"], // agents whose chats the browser may open
  resolveIdentity: async (request) => {
    const user = await yourSession(request);
    return user ? { id: user.id, groups: [user.orgSlug], attributes: { customer: user.orgId } } : null;
  },
});
// Route /api/tilde/* to embed.handle(request).
```

Next.js (App Router), `app/api/tilde/[...path]/route.ts`:

```ts
import { createEmbedRouteHandlers } from "@trytilde/embed-server/next";
export const { GET, POST, DELETE } = createEmbedRouteHandlers(embed);
```

`resolveIdentity` derives the user from your verified server session, never from browser
input; returning null refuses the request. The user's `id` is stable: it is `ext:<id>` in the
project's ontology and the same conversation identity in agent chats. `groups` become
`ext:<group>` groups and `attributes` become Cedar tags (`principal.getTag("customer")`) and
`ctx.principal.attrs`, so the project's policies decide what each user sees.

Routes below `mountPath` (default `/api/tilde`): `POST dashboards/session`, `GET connections`,
`POST connections/setup`, `POST connections/<id>/reconnect`, `DELETE connections/<id>`
(`?agent=` when several agents are configured), and the chat proxy under `chat/`
(`<Chat proxy="/api/tilde/chat">`). Cross-origin requests are refused, and the embed key never
leaves your server.

Server-side calls: `openDashboard(user, environment, dashboard)`, `ontologyToken(user,
environment)` (to call the ontology's API yourself), `createConnectionSetup({ user, agent,
provider })`, `listConnections`, `reconnectConnection` and `deleteConnection`.

Dashboards and embed keys are Tilde Cloud features. With open-source Tilde, use
`@trytilde/chat-proxy` with each agent's chat key for chats.
