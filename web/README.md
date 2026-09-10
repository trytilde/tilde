# Tilde UI

React/Vite with shadcn/ui and Tailwind CSS. The layout starts from
`npx shadcn@latest add dashboard-01`, reduced to the Agent Registry sidebar item,
header, and data table. There are no sample charts, tabs, filters, settings,
search, help, or account menus. Create/edit/delete use shadcn dialogs.

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
