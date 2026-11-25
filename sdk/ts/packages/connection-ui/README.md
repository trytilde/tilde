# @trytilde/connection-ui

React building blocks for Tilde connection setup pages: `useSetup` (the scoped iframe client and
setup state), `Page` and `Form` (presentation and validation), `StandardSetup` and `CredentialForm`
(the standard page, rendered from a provider's JSON Schema), the Tailwind theme (`style.css`) and a
small shadcn-style control set. Providers still author their own TSX; these helpers are optional.

## Prefill

A setup page's URL may say what a new connection starts with:

| Parameter | Fills |
|---|---|
| `type=<connection type id>` | The connection type (`github_app`, `oauth`, `oidc`...). A page with a sign-in method chooser opens that type directly; a type the provider does not offer is ignored and the chooser shows. |
| `name=<account name>` | The account name, while the connection still has the provider's placeholder name. |
| `field.<key>=<value>` | A form field by its credential key (`field.region=eu-west-1`), or a provider-owned page's own field name. |

Everything prefilled stays editable. Unknown parameters, fields the form does not have, values
that are not among a field's choices (enums and booleans), repeated parameters after the first and
values over 1000 characters are ignored. **Secrets never come from a URL**: keys that look secret
(containing `secret`, `token`, `password` or `key`) are dropped, and `CredentialForm` skips fields
its schema marks `writeOnly`. A saved draft takes precedence over the URL.

The brokering page (`/connections/broker/<setup>?connection_setup_token=…&name=…&field.…=…`)
forwards `name` and `field.*` to the provider page in its iframe. Provider-owned pages read the
same contract with `readPrefill(window.location.search)`; `withPrefill(url, prefill)` builds a link
(leaving secret-looking fields out) and `secretKey(key)` is the secret test. The standard page has
no type chooser, so it does not read `type`.
