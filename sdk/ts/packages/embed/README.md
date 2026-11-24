# @trytilde/embed

Tilde in your application's pages: spaces, agent chats and connection setup for your
signed-in users. Your server mounts `@trytilde/embed-server`; this package talks only to it.

```ts
import { mountSpace, mountConnectionSetup, connections } from "@trytilde/embed";

const space = mountSpace(document.getElementById("pipeline")!, {
  environment: "<environment id>",
  space: "pipeline",
  theme: { tokens: { accent: "#6d28d9", radius: "12px" }, scheme: "light" },
  onNavigate: (to) => router.push(to), // a page's navigate(to)
  onEvent: (name, detail) => console.log(name, detail), // a page's emit(name, detail)
});

const setup = await connections({ agent: "<agent id>" }).setup("linear");
mountConnectionSetup(document.getElementById("connect")!, {
  ...setup,
  url: setup.setupUrl,
  theme: { tokens: { accent: "#6d28d9", radius: "12px" }, scheme: "light" },
  onComplete: () => refresh(),
});
```

The space runs in a sandboxed frame of Tilde's spaces host and sizes itself to its
content (`autoResize`, default on); its short-lived tokens come from your server and are renewed
before they expire. Add your site's origin to the project's allowed origins in Tilde Cloud.

`mountConnectionSetup(element | "popup", …)` shows Tilde's setup page, which hosts the
provider's form and OAuth redirects; credentials never pass through your app.

## Theming

One `Theme` styles all three surfaces. Each derives its own colours from the tokens you give and
keeps its defaults for the rest; `scheme` picks the light or dark defaults whatever the page's own.

```tsx
import { Chat, ConnectionSetup, Space, chatProxy, type Theme } from "@trytilde/embed/react";
import "@trytilde/embed/style.css";

const theme: Theme = {
  scheme: "dark",
  tokens: { canvas: "#211714", surface: "#30211d", ink: "#faf9f5", accent: "#e08a5b", radius: "10px" },
};

<Space environment="<environment id>" space="pipeline" theme={theme} />
<Chat proxy={chatProxy()} agentId="<agent id>" sessionId={session} identity={user.id} theme={theme} />
<ConnectionSetup {...setup} url={setup.setupUrl} theme={theme} />
```

Without React, `mountSpace` and `mountConnectionSetup` take `theme` and return `setTheme` for later
changes. To change one surface, pass it a copy: `{ ...theme, tokens: { ...theme.tokens, canvas: "#14100e" } }`.

A space uses every token: they are the spaces preset's own, chart series colours included. The
other two use what they have a place for and ignore the rest:

| Token | Chat | Connection setup |
| --- | --- | --- |
| `canvas` | page, session list and composer backdrop | page background |
| `surface` | agent bubbles, fields, menus | cards, secondary buttons |
| `ink` | text; borders and hover washes are tints of it | text; input borders are a tint of it |
| `muted` | secondary text (a tint of `ink` without it) | secondary text |
| `line` | hairlines | borders |
| `accent` | your bubbles (their text turns black or white to read on it), focus, links | primary button (same text rule), focus ring |
| `good`, `critical` | success and error colours | success and error colours |
| `font` | text | body and headings |
| `radius` | ignored: the chat keeps its bubble and composer shapes | corner radius |
| `series-1` … `series-5` | ignored | ignored |

`<Chat header={false}>` shows only the transcript and the message bar. In this repository,
`task preview:chat-ui` and `task preview:connection-setup` render the chat and the setup page with
one example theme in both schemes.

React (`@trytilde/embed/react`): `<Space>`, `<ConnectionSetup>`, and `Chat` and
`ListSessions` from `@trytilde/chat-ui` (`<Chat proxy={chatProxy()} agentId="…" sessionId={session} identity={user.id} />`).
Import `@trytilde/embed/style.css` for the chat styles.
