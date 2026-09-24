import { useState } from "react";
import ReactMarkdown from "react-markdown";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function Text({ text }: { text: string }) {
  return (
    <div className="trace-prose text-xs leading-relaxed">
      <ReactMarkdown
        components={{
          img: ({ alt }) => (
            <span className="text-muted-foreground">[Image{alt ? `: ${alt}` : ""}]</span>
          ),
          a: ({ children, href }) => (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="underline underline-offset-2"
            >
              {children}
            </a>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
/** Recognize conversation/tool payloads; retain all other fields as compact,
 * collapsible key/value records. Raw JSON stays available for exact inspection.
 */
function Value({ value, depth = 0 }: { value: unknown; depth?: number }) {
  if (typeof value === "string") return <Text text={value} />;
  if (value === null || typeof value !== "object")
    return <span className="font-mono text-[11px]">{JSON.stringify(value) ?? "—"}</span>;
  if (depth >= 8)
    return (
      <pre className="whitespace-pre-wrap break-words text-[11px]">{JSON.stringify(value)}</pre>
    );
  if (Array.isArray(value))
    return (
      <div className="divide-y">
        {value.map((item, index) => (
          <div key={index} className="py-2 first:pt-0 last:pb-0">
            <Value value={item} depth={depth + 1} />
          </div>
        ))}
      </div>
    );
  if (record(value) && typeof value.role === "string" && "content" in value) {
    const extra = Object.fromEntries(
      Object.entries(value).filter(([key]) => !["role", "content"].includes(key)),
    );
    return (
      <div className="grid gap-1.5">
        <span className="w-fit rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] uppercase text-muted-foreground">
          {value.role}
        </span>
        <Value value={value.content} depth={depth + 1} />
        {Object.keys(extra).length > 0 && <Value value={extra} depth={depth + 1} />}
      </div>
    );
  }
  if (
    record(value) &&
    value.type === "text" &&
    typeof value.text === "string" &&
    Object.keys(value).every((key) => ["type", "text"].includes(key))
  )
    return <Text text={value.text} />;
  if (record(value) && Array.isArray(value.messages)) {
    const extra = Object.fromEntries(Object.entries(value).filter(([key]) => key !== "messages"));
    return (
      <div className="grid gap-2">
        <Value value={value.messages} depth={depth + 1} />
        {Object.keys(extra).length > 0 && <Value value={extra} depth={depth + 1} />}
      </div>
    );
  }
  return (
    <dl className="divide-y border-y">
      {Object.entries(value).map(([key, child]) => (
        <div
          key={key}
          className="grid grid-cols-[minmax(80px,24%)_minmax(0,1fr)] gap-3 py-1.5 text-xs"
        >
          <dt className="break-words font-mono text-[11px] text-muted-foreground">{key}</dt>
          <dd className="min-w-0 break-words">
            {typeof child === "object" && child !== null ? (
              <details open={depth < 2}>
                <summary className="cursor-pointer text-[11px] text-muted-foreground">
                  {Array.isArray(child)
                    ? `${child.length} items`
                    : `${Object.keys(child).length} fields`}
                </summary>
                <div className="pt-1">
                  <Value value={child} depth={depth + 1} />
                </div>
              </details>
            ) : (
              <Value value={child} depth={depth + 1} />
            )}
          </dd>
        </div>
      ))}
    </dl>
  );
}
export function TracePayload({ text }: { text: string }) {
  const [mode, setMode] = useState("formatted");
  const large = text.length > 200_000;
  let parsed: unknown = text;
  if (!large) {
    try {
      parsed = JSON.parse(text);
    } catch {
      /* Plain text is a first-class payload. */
    }
  }
  return (
    <div className="min-w-0">
      <Tabs
        value={mode}
        onValueChange={(mode) => setMode(String(mode))}
        className="mb-3 items-end gap-0"
      >
        <TabsList aria-label="Payload format" className="h-6 gap-0 rounded p-0.5">
          <TabsTrigger value="formatted" className="h-5 rounded px-1.5 text-[10px]">
            Formatted
          </TabsTrigger>
          <TabsTrigger value="json" className="h-5 rounded px-1.5 text-[10px]">
            JSON
          </TabsTrigger>
        </TabsList>
      </Tabs>
      {!text || parsed === null ? (
        <p className="text-xs text-muted-foreground">Not recorded</p>
      ) : large ? (
        <>
          <p className="mb-2 text-[11px] text-muted-foreground">
            Large payload: previewing the first 200,000 characters.
          </p>
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px]">
            {text.slice(0, 200_000)}
          </pre>
        </>
      ) : mode === "json" ? (
        <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed">
          {typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2)}
        </pre>
      ) : (
        <Value value={parsed} />
      )}
    </div>
  );
}
