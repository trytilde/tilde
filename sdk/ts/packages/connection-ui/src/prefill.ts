/**
 * Connection setup prefill: what a new connection's setup opens with, passed as query parameters
 * on the setup's URL. Tilde Cloud's console reads the same contract (keep the two identical).
 *
 *   type=<connection type id>  The provider's connection type (`github_app`, `oauth`, `oidc`...).
 *                              A sign-in method chooser is skipped and that type's setup opens
 *                              directly. A type the provider does not offer there is ignored and
 *                              the chooser shows as usual.
 *   name=<account name>        The account name field.
 *   field.<key>=<value>        A form field of the type, by its credential key (`region`) or, on
 *                              a provider-owned page, the page's own field name (`app_name`).
 *
 * Everything prefilled stays editable. Unknown parameters, fields the form does not have, values
 * that are not among a field's choices, repeated parameters after the first and values longer
 * than 1000 characters are ignored. Secrets never travel in a URL (it lands in browser history,
 * logs and screenshots): a field whose key reads as a secret is dropped when the URL is read or
 * built, and forms skip any field their schema marks write-only as well.
 *
 * Where it is read: the brokering page (`/connections/broker/<setup>?connection_setup_token=…
 * &name=…&field.…=…`) forwards `name` and `field.*` to the provider page in its iframe (the
 * setup's type is fixed by then), and provider pages read their own URL (`useSetup` fills the
 * account name, `CredentialForm` the schema's fields).
 */
export type ConnectionPrefill = {
  type?: string;
  name?: string;
  fields?: Record<string, string>;
};

const MAX = 1000;
const FIELD = "field.";

/** Keys that read as credentials: never prefilled, whatever a schema says. */
export const secretKey = (key: string) => /secret|token|password|key/i.test(key);

/** The prefill a setup URL's query carries. */
export function readPrefill(search: string | URLSearchParams): ConnectionPrefill {
  const params = typeof search === "string" ? new URLSearchParams(search) : search;
  const prefill: ConnectionPrefill = {};
  const fields: Record<string, string> = {};
  for (const [param, value] of params) {
    if (!value || value.length > MAX) continue;
    if (param === "type" && prefill.type === undefined && /^[A-Za-z0-9_-]+$/.test(value))
      prefill.type = value;
    else if (param === "name" && prefill.name === undefined) prefill.name = value;
    else if (param.startsWith(FIELD)) {
      const key = param.slice(FIELD.length);
      if (/^[A-Za-z0-9_]{1,160}$/.test(key) && !secretKey(key) && !(key in fields))
        fields[key] = value;
    }
  }
  if (Object.keys(fields).length) prefill.fields = fields;
  return prefill;
}

/** `url` with the prefill's parameters added (secret fields left out). */
export function withPrefill(url: string, prefill: ConnectionPrefill = {}): string {
  const target = new URL(url, "http://relative.invalid");
  if (prefill.type) target.searchParams.set("type", prefill.type);
  if (prefill.name) target.searchParams.set("name", prefill.name);
  for (const [key, value] of Object.entries(prefill.fields ?? {}))
    if (!secretKey(key) && value) target.searchParams.set(`${FIELD}${key}`, value);
  return target.origin === "http://relative.invalid"
    ? `${target.pathname}${target.search}${target.hash}`
    : target.href;
}
