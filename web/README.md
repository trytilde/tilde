# Tilde UI

React/Vite with shadcn/ui and Tailwind CSS. The layout starts from
`npx shadcn@latest add dashboard-01`, reduced to the Agent Registry sidebar item,
header, and data table. Agent creation and editing have dedicated routes.

TanStack Router uses file-based routing in `src/routes`. Add routes with
`createFileRoute`; Vite generates `src/routeTree.gen.ts` during development and builds.
Commit the generated tree, but do not edit it by hand. `_app/route.tsx` owns the
authenticated layout; public connection brokering sits outside it. The shared
`_app/agent/$agentId/route.tsx` editor stays mounted as its child tab routes change.
Agent fetching stays inside the authenticated component tree so it runs after
the session check. Agent routes are grouped under `_app/agent/`, with tab routes
in `$agentId/`. The standalone Connections and Chat pages are not exposed.

`use-cursor-page.ts` sends the server's opaque `nextPageToken` as the next request's
`pageToken`. Previous reuses visited page-start cursors. Page-size changes restart
from an empty cursor. Failed requests preserve the displayed page; retry repeats
the failed request. Superseded responses are ignored. The UI shows the current
page number without claiming a total; only Previous and Next navigation exist.

The square Tilde mark and favicon were copied from the supplied tilde-marketing
checkout's API and documentation favicon SVG assets, matching its canonical
square-and-horizontal-stroke brand guidance. Fonts and assets are bundled locally.

Run `pnpm --dir web test` for cursor and table interaction tests, and
`pnpm --dir web build` for type checking and the production bundle.

The management API provides OIDC login. The UI stores its user bearer token in
local storage and adds Authorization headers to generated clients. Sign out
revokes the server session and clears local storage. The callback route is
handled by the UI and exchanges a one-time code with a browser-held verifier.
No authentication cookies are used. The agent editor supports default-deny
capabilities with Any or explicit target lists where supported.

Public connection setup and identity verification hosts live under
`src/routes/connections/`. Their `-` prefixed modules are excluded from route
generation and are also imported by the catalog iframe entry points. Setup loading
uses shared bouncing dots and Motion fades, with reduced motion support. The app
dependency scan starts at `index.html`; `connection-ui.html` is a provider template.

Connection setup shows the provider overview followed by ordered instructions
from the selected connection-type adapter. When the setup has a webhook URL, a
read-only field with a copy button appears below the account name and before
credentials. Connection-type adapters can bind the account name to a credential
field through `account_name_field`; the shared broker supplies and validates it,
so the form never repeats the field. AgentMail maps to the inbox ID; Linq and
Telnyx map to the sending phone number.
