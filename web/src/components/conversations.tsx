import { useEffect, useState } from "react";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ChatService } from "@/gen/tilde/management/v1/chat_pb.js";
import { type Thread, type Activity, type Attachment } from "@/gen/tilde/types/v1/chat_pb.js";
import { authInterceptor } from "@/auth";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

const chat = createClient(
  ChatService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
/** Ordered snapshots expose every event rather than reconstructing an audit from final messages. */
export function ConversationsPage() {
  const [threads, setThreads] = useState<Thread[]>([]);
  const [next, setNext] = useState("");
  const [selected, setSelected] = useState(
    new URLSearchParams(window.location.search).get("thread") ?? "",
  );
  const [error, setError] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    void chat
      .listThreads({ pageSize: 30 }, { signal: controller.signal })
      .then((p) => {
        setThreads(p.threads);
        setNext(p.nextPageToken);
      })
      .catch((e) => {
        if (!controller.signal.aborted) setError(String(e));
      });
    return () => controller.abort();
  }, []);
  return (
    <section className="mx-auto w-full max-w-5xl space-y-5 p-6">
      <div className="flex gap-3">
        <Select
          value={selected}
          onValueChange={(v) => setSelected(v ?? "")}
          items={threads.map((t) => ({ value: t.id, label: t.title }))}
        >
          <SelectTrigger className="w-full">
            <SelectValue placeholder="Select a conversation" />
          </SelectTrigger>
          <SelectContent>
            {threads.map((t) => (
              <SelectItem key={t.id} value={t.id}>
                {t.title}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {next && (
          <Button
            variant="outline"
            onClick={() =>
              void chat
                .listThreads({ pageToken: next, pageSize: 30 })
                .then((p) => {
                  setThreads([...threads, ...p.threads]);
                  setNext(p.nextPageToken);
                })
                .catch((e) => setError(String(e)))
            }
          >
            More conversations
          </Button>
        )}
      </div>
      {error && <p role="alert">{error}</p>}
      {selected ? (
        <Conversation key={selected} threadId={selected} />
      ) : (
        <p className="text-sm text-muted-foreground">
          Choose a conversation to view its messages and audit trail.
        </p>
      )}
    </section>
  );
}
export function Conversation({ threadId }: { threadId: string }) {
  const [events, setEvents] = useState<Activity[]>([]);
  const [error, setError] = useState("");
  const [audit, setAudit] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    void (async () => {
      let cursor = 0n;
      let initial: Activity[] = [];
      do {
        const page = await chat.listActivity(
          { threadId, afterSequence: cursor, limit: 100 },
          { signal: controller.signal },
        );
        initial = initial.concat(page.events);
        cursor = page.nextSequence;
        if (!page.hasMore) break;
      } while (!controller.signal.aborted);
      setEvents(initial);
      for await (const response of chat.watchThread(
        { threadId, afterSequence: cursor },
        { signal: controller.signal },
      )) {
        const event = response.activity;
        if (!event) continue;
        setEvents((previous) =>
          previous.some((e) => e.sequence === event.sequence) ? previous : [...previous, event],
        );
      }
    })().catch((e) => {
      if (!controller.signal.aborted)
        setError(e instanceof Error ? e.message : "Unable to load conversation");
    });
    return () => controller.abort();
  }, [threadId]);
  const names = new Map<string, string>();
  for (const e of events) {
    if (e.detail.case === "participant") names.set(e.detail.value.id, e.detail.value.name);
  }
  const rendered = conversationEvents(events);
  const visible = audit ? events : rendered;
  return (
    <div className="space-y-4">
      <div className="flex justify-between">
        <h2 className="text-xl font-medium">Conversation</h2>
        <Button variant="outline" onClick={() => setAudit(!audit)}>
          {audit ? "Conversation view" : "All events and chunks"}
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      <ol className="space-y-3" aria-label="Conversation audit">
        {visible.map((event) => (
          <li key={String(event.sequence)}>
            <AuditEvent event={event} names={names} />
          </li>
        ))}
      </ol>
    </div>
  );
}
export function AuditEvent({
  event,
  names = new Map<string, string>(),
}: {
  event: Activity;
  names?: Map<string, string>;
}) {
  const detail = event.detail;
  const time = event.createdAt
    ? new Date(Number(event.createdAt.seconds) * 1000).toLocaleString()
    : "";
  return (
    <article className="rounded-lg border p-4">
      <div className="mb-2 flex justify-between text-xs text-muted-foreground">
        <span>{event.kind}</span>
        <time>{time}</time>
      </div>
      {detail.case === "message" ? (
        <>
          <div className="mb-1 text-xs">
            {names.get(detail.value.participantId) ?? detail.value.participantId} ·{" "}
            {detail.value.status}
          </div>
          {detail.value.subject && <h3 className="mb-2 font-medium">{detail.value.subject}</h3>}
          {detail.value.delivery && (
            <p className="mb-2 text-xs text-muted-foreground">
              {detail.value.delivery.destination} · {detail.value.delivery.status}
            </p>
          )}
          {detail.value.format === "html" ? (
            <iframe
              title="Email content"
              sandbox=""
              className="min-h-48 w-full rounded border"
              srcDoc={`<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">${detail.value.text}`}
            />
          ) : (
            <p className="whitespace-pre-wrap break-words">{detail.value.text}</p>
          )}
          {detail.value.attachments.map((a) => (
            <AttachmentDownload key={a.id} attachment={a} />
          ))}
          {event.kind === "message.delta" && (
            <pre className="mt-2 whitespace-pre-wrap text-sm">{event.textDelta}</pre>
          )}
        </>
      ) : detail.case === "messageChunk" ? (
        <pre className="whitespace-pre-wrap text-sm">{detail.value.textDelta}</pre>
      ) : detail.case === "participant" ? (
        <p>
          {detail.value.name} {detail.value.active ? "joined" : "left"}
        </p>
      ) : detail.case === "typing" ? (
        <p className="text-sm">
          {names.get(detail.value.participantId) ?? detail.value.participantId}{" "}
          {detail.value.typing ? "started typing" : "stopped typing"}
        </p>
      ) : detail.case === "toolCall" ? (
        <details open={detail.value.status === "failed"}>
          <summary className="cursor-pointer text-sm font-medium">
            {detail.value.name} · {detail.value.status}
          </summary>
          {detail.value.inputJson && (
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs">
              {detail.value.inputJson}
            </pre>
          )}
          {detail.value.inputDelta && (
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs">
              {detail.value.inputDelta}
            </pre>
          )}
          {detail.value.outputJson && (
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap text-xs">
              {detail.value.outputJson}
            </pre>
          )}
          {detail.value.error && (
            <p role="alert" className="text-sm text-destructive">
              {detail.value.error}
            </p>
          )}
        </details>
      ) : (
        <p className="whitespace-pre-wrap break-words text-sm">
          {event.textDelta || event.entityId}
        </p>
      )}
    </article>
  );
}
function AttachmentDownload({ attachment }: { attachment: Attachment }) {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function download() {
    setBusy(true);
    setError("");
    try {
      const result = await chat.downloadAttachment({
        threadId: attachment.threadId,
        attachmentId: attachment.id,
      });
      const url = URL.createObjectURL(
        new Blob([new Uint8Array(result.content)], { type: "application/octet-stream" }),
      );
      const a = document.createElement("a");
      a.href = url;
      a.download = attachment.filename;
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="mt-2">
      <Button variant="outline" disabled={busy} onClick={() => void download()}>
        {attachment.filename} · {String(attachment.sizeBytes)} bytes
      </Button>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

/** Fold streamed chunks and final snapshots into one message while keeping its original position. */
export function conversationEvents(events: Activity[]): Activity[] {
  const messages = new Map<string, Activity>();
  const positions = new Map<string, bigint>();
  for (const event of events) {
    if (event.detail.case === "message") {
      const id = event.detail.value.id;
      positions.set(id, positions.get(id) ?? event.sequence);
      messages.set(id, event);
    } else if (event.detail.case === "messageChunk") {
      const prior = messages.get(event.detail.value.messageId);
      if (prior?.detail.case === "message")
        messages.set(event.detail.value.messageId, {
          ...prior,
          detail: {
            case: "message",
            value: {
              ...prior.detail.value,
              text: prior.detail.value.text + event.detail.value.textDelta,
            },
          },
        });
    }
  }
  return events.flatMap((event) => {
    if (event.detail.case === "messageChunk") return [];
    if (event.detail.case !== "message") return [event];
    const id = event.detail.value.id;
    return positions.get(id) === event.sequence ? [messages.get(id)!] : [];
  });
}
