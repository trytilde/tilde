import { quote, tokens, unquote, type SearchSuggestion } from "@/features/observability/query";

/** The Skills page query: `group:<name>` terms (any of them) and free words matched against names and descriptions. */
export type SkillFilter = { groups: string[]; words: string[] };

export function parseSkillQuery(query: string): SkillFilter {
  const scanned = tokens(query);
  if (scanned.unfinished)
    throw new Error('Close the quoted value and escape embedded quotes with \\".');
  const filter: SkillFilter = { groups: [], words: [] };
  for (const { text } of scanned.items) {
    const colon = text.indexOf(":");
    if (colon === -1 || text.startsWith('"')) {
      filter.words.push(unquote(text).toLowerCase());
      continue;
    }
    const key = text.slice(0, colon);
    if (key !== "group") throw new Error(`Unknown filter ${key}:. Use group: or plain words.`);
    const value = unquote(text.slice(colon + 1));
    if (!value) throw new Error("Name a group after group:.");
    filter.groups.push(value);
  }
  return filter;
}

export function serializeSkillQuery(filter: SkillFilter) {
  return [
    ...filter.groups.map((group) => `group:${quote(group)}`),
    ...filter.words.map(quote),
  ].join(" ");
}

export function skillSuggestions(
  query: string,
  caret: number,
  groups: string[],
): SearchSuggestion[] {
  const token = tokens(query).items.find((t) => caret >= t.start && caret <= t.end) ?? {
    start: caret,
    end: caret,
    text: "",
  };
  const prefix = query.slice(token.start, caret);
  const replace = (text: string) => query.slice(0, token.start) + text + query.slice(token.end);
  if (!prefix.includes(":")) {
    return "group:".startsWith(prefix.toLowerCase())
      ? [{ value: replace("group:"), label: "group:", description: "Skills in a group" }]
      : [];
  }
  if (!prefix.startsWith("group:")) return [];
  const typed = prefix.slice("group:".length).replace(/^"/, "").toLowerCase();
  return groups
    .filter((name) => name.toLowerCase().includes(typed))
    .slice(0, 12)
    .map((name) => ({ value: replace(`group:${quote(name)}`), label: name, description: "Group" }));
}
