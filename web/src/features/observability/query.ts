export type SearchSuggestion = { value: string; label: string; description: string };
/** Track token boundaries for both committed queries and unfinished autocomplete input. */
export function tokens(query: string) {
  const result: { text: string; start: number; end: number }[] = [];
  let start = -1,
    quoted = false,
    escaped = false;
  for (let i = 0; i < query.length; i++) {
    const ch = query[i];
    if (start === -1) {
      if (/\s/.test(ch)) continue;
      start = i;
    }
    if (escaped) {
      escaped = false;
      continue;
    }
    if (quoted && ch === "\\") {
      escaped = true;
      continue;
    }
    if (ch === '"') quoted = !quoted;
    if (!quoted && /\s/.test(ch)) {
      result.push({ text: query.slice(start, i), start, end: i });
      start = -1;
    }
  }
  if (start !== -1) result.push({ text: query.slice(start), start, end: query.length });
  return { items: result, unfinished: quoted || escaped };
}
export function unquote(value: string) {
  if (!value.startsWith('"')) {
    if (value.includes('"')) throw new Error("Put quotes around the entire filter value.");
    return value;
  }
  try {
    const parsed: unknown = JSON.parse(value);
    if (typeof parsed === "string") return parsed;
  } catch {
    /* Present a query-specific error, not a JSON parser diagnostic. */
  }
  throw new Error('Close the quoted value and escape embedded quotes with \\".');
}
export const quote = (value: string) => (/[\s:"\\]/.test(value) ? JSON.stringify(value) : value);
