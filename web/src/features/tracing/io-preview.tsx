// ViewModeToggle adapted from Langfuse's MIT component; see ATTRIBUTION.md.
import { useState } from "react";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
export type ViewMode = "pretty" | "json";
export function ViewModeToggle({
  selectedView,
  onViewChange,
}: {
  selectedView: ViewMode;
  onViewChange: (view: ViewMode) => void;
}) {
  return (
    <div className="flex w-full items-center gap-1.5">
      <Tabs value={selectedView} onValueChange={(v) => onViewChange(v as ViewMode)}>
        <TabsList aria-label="Input/output format">
          <TabsTrigger value="pretty">Formatted</TabsTrigger>
          <TabsTrigger value="json">JSON</TabsTrigger>
        </TabsList>
      </Tabs>
    </div>
  );
}
function Tree({ value, depth = 0 }: { value: unknown; depth?: number }) {
  if (value === null || typeof value !== "object")
    return (
      <span className="whitespace-pre-wrap break-words">
        {typeof value === "string" ? value : (JSON.stringify(value) ?? "null")}
      </span>
    );
  if (depth > 8)
    return <pre className="whitespace-pre-wrap break-all">{JSON.stringify(value)}</pre>;
  return (
    <div className="space-y-2">
      {Object.entries(value).map(([key, child]) => (
        <details key={key} open={depth < 2} className="border-l pl-3">
          <summary className="cursor-pointer font-mono text-xs text-muted-foreground">
            {key}
          </summary>
          <div className="pt-1">
            <Tree value={child} depth={depth + 1} />
          </div>
        </details>
      ))}
    </div>
  );
}
export function IOPreview({ title, text, mode }: { title: string; text: string; mode: ViewMode }) {
  const [copied, setCopied] = useState(false);
  let parsed: unknown = text;
  try {
    if (text.length < 200_000) parsed = JSON.parse(text);
  } catch {
    /* Plain text is a valid observation payload. */
  }
  const large = text.length > 200_000;
  return (
    <section className="overflow-hidden rounded-lg border">
      <div className="flex items-center justify-between bg-muted/40 px-3 py-2">
        <h4 className="text-sm font-medium">{title}</h4>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          disabled={!text}
          onClick={() => {
            void navigator.clipboard.writeText(text).then(() => setCopied(true));
          }}
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <div className="max-h-96 overflow-auto p-3 text-sm">
        {!text ? (
          <p className="text-muted-foreground">Not recorded</p>
        ) : large ? (
          <>
            <p className="mb-2 text-xs text-muted-foreground">
              Large payload: showing the first 200,000 characters. Copy includes the full value.
            </p>
            <pre className="whitespace-pre-wrap break-all">{text.slice(0, 200_000)}</pre>
          </>
        ) : mode === "json" ? (
          <pre className="whitespace-pre-wrap break-all text-xs">
            {typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2)}
          </pre>
        ) : (
          <Tree value={parsed} />
        )}
      </div>
    </section>
  );
}
