export type LogSearch = {
  range?: string;
  from?: string;
  to?: string;
  severity?: string;
  text?: string;
  invocation?: string;
  trace?: string;
  service?: string;
};
export function parseLogSearch(raw: Record<string, unknown>): LogSearch {
  return Object.fromEntries(
    ["range", "from", "to", "severity", "text", "invocation", "trace", "service"].flatMap((key) =>
      typeof raw[key] === "string" && raw[key] ? [[key, raw[key]]] : [],
    ),
  );
}
