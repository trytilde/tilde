/** One line of a diff: kept, removed from `before` or added in `after`. */
type Line = { kind: " " | "-" | "+"; text: string };
/** Past this many lines on a side the diff is too costly to compute in the browser. */
const MAX_LINES = 4000;

/** A line diff by longest common subsequence, or `null` when the texts are too long. */
export function diffLines(before: string, after: string): Line[] | null {
  const a = before.split("\n");
  const b = after.split("\n");
  if (a.length > MAX_LINES || b.length > MAX_LINES) return null;
  // lcs[i][j]: the longest common subsequence of a[i..] and b[j..].
  const lcs = Array.from({ length: a.length + 1 }, () => new Uint32Array(b.length + 1));
  for (let i = a.length - 1; i >= 0; i--)
    for (let j = b.length - 1; j >= 0; j--)
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
  const out: Line[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ kind: " ", text: a[i++] });
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) out.push({ kind: "-", text: a[i++] });
    else out.push({ kind: "+", text: b[j++] });
  }
  while (i < a.length) out.push({ kind: "-", text: a[i++] });
  while (j < b.length) out.push({ kind: "+", text: b[j++] });
  return out;
}

/** Unchanged lines kept around each change; longer unchanged runs fold. */
const CONTEXT = 3;
type Row = Line | { kind: "fold"; count: number };
/** The diff with each unchanged run longer than the context on both sides folded. */
function folded(lines: Line[]): Row[] {
  const out: Row[] = [];
  for (let i = 0; i < lines.length;) {
    if (lines[i].kind !== " ") {
      out.push(lines[i++]);
      continue;
    }
    let end = i;
    while (end < lines.length && lines[end].kind === " ") end++;
    const keepBefore = i === 0 ? 0 : CONTEXT;
    const keepAfter = end === lines.length ? 0 : CONTEXT;
    if (end - i > keepBefore + keepAfter + 1) {
      out.push(...lines.slice(i, i + keepBefore));
      out.push({ kind: "fold", count: end - i - keepBefore - keepAfter });
      out.push(...lines.slice(end - keepAfter, end));
    } else out.push(...lines.slice(i, end));
    i = end;
  }
  return out;
}

/** Two versions of a text, line by line, with what was removed and added marked. */
export function LineDiff({ before, after }: { before: string; after: string }) {
  const diff = diffLines(before, after);
  if (!diff)
    return <p className="text-sm text-muted-foreground">These versions are too long to compare.</p>;
  const lines = folded(diff);
  return (
    <pre
      aria-label="Changes"
      className="max-h-[60vh] overflow-auto rounded-lg border bg-muted/20 py-2 font-mono text-xs leading-relaxed"
    >
      {lines.map((line, index) =>
        line.kind === "fold" ? (
          <div key={index} className="px-3 text-muted-foreground italic">
            ⋯ {line.count} unchanged {line.count === 1 ? "line" : "lines"}
          </div>
        ) : (
          <div
            key={index}
            className={
              line.kind === "+"
                ? "bg-emerald-500/15 px-3"
                : line.kind === "-"
                  ? "bg-destructive/15 px-3"
                  : "px-3 text-muted-foreground"
            }
          >
            <span aria-hidden className="mr-3 select-none">
              {line.kind}
            </span>
            <span className="sr-only">
              {line.kind === "+" ? "Added: " : line.kind === "-" ? "Removed: " : ""}
            </span>
            {line.text || " "}
          </div>
        ),
      )}
    </pre>
  );
}
