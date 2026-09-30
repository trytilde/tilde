import { useEffect, useState, type ReactNode } from "react";
import { traces } from "@/client";
import { Button } from "@/components/ui/button";

/** `@@@tildeMedia:type=<mime>|id=<sha256>|source=<origin>@@@`, as the gateway writes it. */
const token = /@@@tildeMedia:type=([^|@]+)\|id=([0-9a-f]{64})\|source=[^@]*@@@/g;
export const hasMedia = (text: string) => /@@@tildeMedia:/.test(text);

/** Text with media tokens replaced by the media itself, fetched through a signed URL. */
export function WithMedia({ agentId, text }: { agentId: string; text: string }) {
  const parts: ReactNode[] = [];
  let last = 0;
  for (const match of text.matchAll(token)) {
    if (match.index > last) parts.push(text.slice(last, match.index));
    parts.push(<Media key={`${match.index}`} agentId={agentId} type={match[1]} id={match[2]} />);
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
}
function Media({ agentId, type, id }: { agentId: string; type: string; id: string }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    void traces
      .getTraceObjectUrl({ agentId, key: `media/${agentId}/${id}` }, { signal: abort.signal })
      .then((response) => setUrl(response.url))
      .catch((cause) => setError(cause instanceof Error ? cause.message : "Unavailable"));
    return () => abort.abort();
  }, [agentId, id]);
  if (error)
    return (
      <span className="text-muted-foreground" title={error}>
        [{type} unavailable]
      </span>
    );
  if (!url) return <span className="text-muted-foreground">[{type}…]</span>;
  if (type.startsWith("image/"))
    return <img src={url} alt={type} className="my-1 max-h-80 max-w-full rounded border" />;
  if (type.startsWith("audio/")) return <audio src={url} controls className="my-1 max-w-full" />;
  if (type.startsWith("video/"))
    return <video src={url} controls className="my-1 max-h-80 max-w-full rounded border" />;
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="underline underline-offset-2"
    >
      [{type}]
    </a>
  );
}
/**
 * A payload the gateway truncated on the span, with the full copy in the trace bucket under
 * `reference`. Loads the whole text on demand.
 */
export function FullPayload({
  agentId,
  reference,
  onLoaded,
}: {
  agentId: string;
  reference: string;
  onLoaded: (text: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <p className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
      Showing the first 64 KiB of this payload.
      <Button
        type="button"
        variant="outline"
        size="xs"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError("");
          void traces
            .getTraceObjectUrl({ agentId, key: reference })
            .then((response) => fetch(response.url))
            .then((response) => {
              if (!response.ok) throw new Error("Unable to load the payload.");
              return response.text();
            })
            .then(onLoaded)
            .catch((cause) => setError(cause instanceof Error ? cause.message : "Unavailable"))
            .finally(() => setBusy(false));
        }}
      >
        {busy ? "Loading…" : "Load full payload"}
      </Button>
      {error && <span className="text-destructive">{error}</span>}
    </p>
  );
}
