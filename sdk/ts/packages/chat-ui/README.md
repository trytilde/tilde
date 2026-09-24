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

`identity` is a stable UI/cache identity, not authorization. The server proxy independently resolves the authenticated user. Change this prop when signing in as another user; unmount the components on sign-out. `Chat.agentId` is optional for a single-agent proxy; supplying it avoids discovery requests for a multi-agent sidebar.

The components own requests and clean up subscriptions and attachment object URLs on unmount. Mutations invalidate mounted sidebars sharing the proxy and identity; background sidebar refresh also observes other tabs. CSS is scoped to `.tilde-chat` and does not reset the host page. Its `--tc-bg`, `--tc-ink`, `--tc-muted`, `--tc-line`, and `--tc-accent` variables may be overridden. Give Chat a bounded height for history scrolling. React 19 is a peer dependency. No router, API key or Next.js dependency is required in the browser.

Use `@trytilde/chat-next` for Next.js routes or `@trytilde/chat-proxy` with any Web Request/Response server. Builds run via `task build:chat`. Dispatch-derived files retain their MIT attribution in `LICENSE.dispatch`.

## Recorded transcripts

Read-only transcript presentation using the original Dispatch components:
`ConversationMessage`, `ToolChipsBlock`, `TraceBlock`, and its Markdown/code renderer.
Sources: `trytilde/dispatch` commit `2ecb27d`, `packages/ui/src/chat-components.tsx`,
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
