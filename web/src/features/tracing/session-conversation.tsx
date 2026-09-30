import { useMemo } from "react";
import { Transcript } from "@trytilde/chat-ui";
import type { Observation } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Button } from "@/components/ui/button";
import { sessionTranscript } from "./session-transcript";
import { SkeletonLines } from "@/components/table-skeleton";
export function SessionConversation({
  rows,
  busy,
  hasMore,
  onLoadMore,
  onInspect,
}: {
  rows: Observation[];
  busy: boolean;
  hasMore: boolean;
  onLoadMore: () => void;
  onInspect: (sourceId: string) => void;
}) {
  const entries = useMemo(() => sessionTranscript(rows), [rows]);
  return (
    <section
      className="flex min-h-0 flex-1 flex-col overflow-auto"
      aria-label="Session conversation"
    >
      <div className="flex items-center justify-between gap-3 border-b px-4 py-2 text-[11px] text-muted-foreground">
        <span>Recorded messages, reasoning and tool calls</span>
        {hasMore && (
          <Button variant="ghost" size="xs" disabled={busy} onClick={onLoadMore}>
            {busy ? "Loading…" : "Load more traces"}
          </Button>
        )}
      </div>
      {entries.length ? (
        <Transcript className="flex-1" entries={entries} onInspect={onInspect} />
      ) : (
        <div className="p-4 text-xs text-muted-foreground">
          {busy ? (
            <SkeletonLines rows={5} label="Loading session" />
          ) : (
            <p role="status">No message content was recorded for this session.</p>
          )}
        </div>
      )}
    </section>
  );
}
