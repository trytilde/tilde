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
  onComplete: () => refresh(),
});
```

The space runs in a sandboxed frame of Tilde's spaces host and sizes itself to its
content (`autoResize`, default on); its short-lived tokens come from your server and are renewed
before they expire. Add your site's origin to the project's allowed origins in Tilde Cloud.

`mountConnectionSetup(element | "popup", …)` shows Tilde's setup page, which hosts the
provider's form and OAuth redirects; credentials never pass through your app.

React (`@trytilde/embed/react`): `<Space>`, `<ConnectionSetup>`, and `Chat` and
`ListSessions` from `@trytilde/chat-ui` (`<Chat proxy={chatProxy()} agentId="…" sessionId={session} identity={user.id} />`).
Import `@trytilde/embed/style.css` for the chat styles.
