# @trytilde/chat-ui

Embeddable native Tilde conversations, extracted from Dispatch's composer and markdown presentation. Includes paginated history, live updates with reconnect/replay, replies, copying, markdown/code, attachments, typing, stop/resume, queue remove/reorder/steer, session creation, search, rename and per-identity read/unread state. Desktop, routines and workspace administration remain outside this package.

```tsx
'use client';
import { useState } from 'react';
import { Chat, ListSessions } from '@trytilde/chat-ui';
import '@trytilde/chat-ui/style.css';

export function Conversations({ userId }: { userId: string }) {
  const [selected, select] = useState<{ sessionId: string; agentId: string }>();
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '280px 1fr', height: '80vh' }}>
      <ListSessions identity={userId} proxy="/api/chat"
        selectedSessionId={selected?.sessionId} selectedAgentId={selected?.agentId}
        onSelectSession={(sessionId, agentId) => select({ sessionId, agentId })} />
      {selected && <Chat {...selected} identity={userId} proxy="/api/chat" />}
    </div>
  );
}
```

The proxy may be on another origin; requests carry cookies (`credentials: "include"`), so a cross-origin proxy must allow credentialed CORS from your app. `Chat.fetch` and `ListSessions.fetch` replace that fetch, e.g. to send a bearer token instead (a Tilde Cloud space's `<AgentChat>`), or to answer from fixtures in tests and Storybook. Keep it stable (define it once or memoise it): a new function reconnects. A refused request shows the proxy's own Connect error message. A viewer reading a thread they are not in yet (a space's chat) posts naming no participant, and the server binds the post to them or refuses it.

`identity` is a stable UI/cache identity, not authorization. The server proxy independently resolves the authenticated user. Change this prop when signing in as another user; unmount the components on sign-out. `Chat.agentId` is optional for a single-agent proxy; supplying it avoids discovery requests for a multi-agent sidebar.

`showHeader={false}` hides the header (agent, thread title, search) for a host with a title of its own; a waiting run's Resume button still shows. `showSearch={false}` keeps the header without its search. `className` styles the root and `classNames` (`header`, `conversation`, `composer`) its parts.

Colours: the chat's background is transparent by default, so it sits on the host's own surface, and light or dark follows a `.dark` class or `data-theme="dark"` on an ancestor. A host on its own tokens sets these on the chat (or an ancestor); unset ones keep chat-ui's palette:

```css
.my-chat {
  --tilde-chat-background: var(--surface); /* default: transparent */
  --tilde-chat-ink: var(--text);
  --tilde-chat-muted: var(--text-muted);
  --tilde-chat-line: var(--border);
  --tilde-chat-bubble-agent: var(--surface-raised);
  --tilde-chat-bubble-user: var(--text);
  --tilde-chat-bubble-user-ink: var(--surface);
  --tilde-chat-composer-background: var(--surface-raised);
}
```

Icons are lucide's throughout.

The components own requests and clean up subscriptions and attachment object URLs on unmount. Mutations invalidate mounted sidebars sharing the proxy and identity; background sidebar refresh also observes other tabs. CSS is scoped to `.tilde-chat` and does not reset the host page. Dispatch's palette variables (`--page`, `--ink`, `--line`, `--accent`, `--bubble-agent`, `--bubble-user`, …) may be overridden on `.tilde-chat`. Give Chat a bounded height for history scrolling. React 19 is a peer dependency. No router, API key or Next.js dependency is required in the browser.

The look is Dispatch's (openbot's) chat: grey agent bubbles on the left, black user bubbles on the right, grouped consecutive messages, centred day separators, a pill composer with `+` and a round send button, and a session list with agent avatars, unread dots and a ⌘K search. Reply and Copy appear on hover. Light and dark follow a `.dark` class on an ancestor. Run `task preview:chat-ui` for a fixture-backed preview at `/chat-ui-preview.html`.

Use `@trytilde/chat-next` for Next.js routes or `@trytilde/chat-proxy` with any Web Request/Response server. Builds run via `task build:chat`. Dispatch-derived files retain their MIT attribution in `LICENSE.dispatch`.

## Recorded transcripts

Read-only transcript presentation using the original Dispatch components:
`ConversationMessage`, `ToolChipsBlock`, `TraceBlock`, and its Markdown/code renderer.
Sources: `trytilde/dispatch` commits `2ecb27d` and `a655a4f` (the live chat surface: `openbot-ui.css`, `chat-components.tsx`, `chat-composer.tsx`, `transcript-components.tsx`, `activity-queue.tsx`, `workspace-icons.tsx`, `sidebar-components.tsx`), `packages/ui/src/chat-components.tsx`,
`beautiful-ui/blocks/{tool-chips-block,trace-block}.tsx`, `markdown-components.tsx`,
and the matching chat/palette rules from `openbot-ui.css` and `beautiful-ui/upstream/globals.css`.
See LICENSE.dispatch and LICENSE.beautiful-ui.

The tool-chip and reasoning primitives retain their original component implementations.
Message rendering adds optional trace-source navigation and a formatted timestamp label.
Chat CSS is scoped to `.tilde-chat`; the existing Dispatch palette, message geometry,
markdown rules and Tailwind utility tokens are preserved within that surface.
`Transcript` adapts recorded entries into these components, groups contiguous execution
activity, and includes full captured tool input/output instead of a shortened preview.
It never opens live chat subscriptions, sends messages, or fetches markdown images.

Import `Transcript` and `@trytilde/chat-ui/style.css`. The stylesheet uses Tailwind 4
and includes a source directive for the component classes. Build with `task build:chat-ui`.
Chat and ListSessions are the live clients in this same package.
