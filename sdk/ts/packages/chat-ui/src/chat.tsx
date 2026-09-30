"use client";
import { useEffect, useMemo, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { Button, Input } from "@trytilde/connection-ui";
import {
  ToolDisplay,
  type Message,
  type QueuedInput,
  type ToolCall,
  type User,
} from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import type { Session } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import { ChatComposer, type ComposerAttachment } from "./composer.js";
import { MarkdownText } from "./markdown.js";
import { AttachmentView } from "./attachments.js";
import {
  randomUUID,
  chatClient,
  changed,
  errorText,
  listAgents,
  type ChatAgent,
} from "./client.js";
export interface ChatProps {
  sessionId: string;
  identity: string;
  proxy: string;
  agentId?: string;
  className?: string;
}
export function Chat(props: ChatProps) {
  const [attempt, setAttempt] = useState(0);
  return (
    <ChatWindow
      key={`${props.proxy}:${props.identity}:${props.agentId ?? ""}:${props.sessionId}:${attempt}`}
      {...props}
      onRetry={() => setAttempt((value) => value + 1)}
    />
  );
}
type Pending = ComposerAttachment & { file: File; uploadId: string };
function ChatWindow({
  sessionId,
  identity,
  proxy,
  agentId,
  className = "",
  onRetry,
}: ChatProps & { onRetry: () => void }) {
  const [agent, setAgent] = useState<ChatAgent>();
  const [session, setSession] = useState<Session>();
  const [user, setUser] = useState<User>();
  const [messages, setMessages] = useState<Message[]>([]);
  const [queue, setQueue] = useState<QueuedInput[]>([]);
  const [tools, setTools] = useState<Record<string, ToolCall>>({});
  const [reasoning, setReasoning] = useState("");
  const [typing, setTyping] = useState("");
  const lastTyping = useRef(0);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [reconnecting, setReconnecting] = useState(false);
  const [before, setBefore] = useState("");
  const [paging, setPaging] = useState(false);
  const [files, setFiles] = useState<Pending[]>([]);
  const [reply, setReply] = useState<Message>();
  const [submitting, setSubmitting] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [latest, setLatest] = useState(false);
  const [search, setSearch] = useState("");
  const [matches, setMatches] = useState<Message[]>([]);
  const [matchCursor, setMatchCursor] = useState("");
  const [searching, setSearching] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const lifetime = useRef(new AbortController());
  const blobs = useRef(new Set<string>());
  const sequence = useRef(0n);
  const pendingSend = useRef<
    { id: string; text: string; attachmentIds: string[]; reply?: string } | undefined
  >(undefined);
  const form = useForm({ defaultValues: { draft: "" } });
  const draft = form.watch("draft");
  const client = useMemo(() => (agent ? chatClient(proxy, agent.id) : undefined), [proxy, agent]);
  useEffect(() => {
    const abort = new AbortController();
    lifetime.current = abort;
    return () => {
      abort.abort();
      for (const blob of blobs.current) URL.revokeObjectURL(blob);
    };
  }, []);
  useEffect(() => {
    const abort = new AbortController();
    void (async () => {
      const agents = await listAgents(proxy, abort.signal);
      const candidates = agentId ? agents.filter((a) => a.id === agentId) : agents;
      if (candidates.length === 1) {
        if (!abort.signal.aborted) setAgent(candidates[0]);
        return;
      }
      for (const candidate of candidates) {
        try {
          await chatClient(proxy, candidate.id).getSession(
            { threadId: sessionId },
            { signal: abort.signal },
          );
          if (!abort.signal.aborted) setAgent(candidate);
          return;
        } catch (error) {
          if (abort.signal.aborted) return;
          if (candidates.length === 1) throw error;
        }
      }
      throw new Error("Conversation is unavailable");
    })().catch((error) => {
      if (!abort.signal.aborted) {
        setError(errorText(error));
        setLoading(false);
      }
    });
    return () => abort.abort();
  }, [proxy, identity, agentId, sessionId]);
  useEffect(() => {
    if (!client || !search.trim()) {
      setSearching(false);
      setMatches([]);
      setMatchCursor("");
      return;
    }
    const abort = new AbortController();
    setSearching(true);
    const timer = setTimeout(() => {
      void client
        .searchMessages(
          { threadId: sessionId, query: search, pageSize: 30 },
          { signal: abort.signal },
        )
        .then((page) => {
          if (!abort.signal.aborted) {
            setMatches(page.messages);
            setMatchCursor(page.nextPageToken);
          }
        })
        .catch((error) => {
          if (!abort.signal.aborted) setError(errorText(error));
        })
        .finally(() => {
          if (!abort.signal.aborted) setSearching(false);
        });
    }, 200);
    return () => {
      abort.abort();
      clearTimeout(timer);
    };
  }, [client, sessionId, search]);
  const refresh = async (signal = lifetime.current.signal) => {
    if (!client) return;
    const [state, pending] = await Promise.all([
      client.getSession({ threadId: sessionId }, { signal }),
      client.listQueuedMessages({ threadId: sessionId }, { signal }),
    ]);
    if (!signal.aborted) {
      setSession(state.session);
      setQueue(pending.messages);
    }
  };
  useEffect(() => {
    if (!client) return;
    const abort = new AbortController();
    let cursor = "";
    let timer: ReturnType<typeof setTimeout> | undefined;
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;
    const scheduleRefresh = () => {
      if (refreshTimer) return;
      refreshTimer = setTimeout(() => {
        refreshTimer = undefined;
        void refresh(abort.signal).catch((error) => {
          if (!abort.signal.aborted) setError(errorText(error));
        });
        changed(proxy, identity);
      }, 100);
    };
    void (async () => {
      const [initial, who] = await Promise.all([
        client.getSession({ threadId: sessionId, includeHistory: true }, { signal: abort.signal }),
        client.getIdentity({}, { signal: abort.signal }),
      ]);
      cursor = initial.cursor;
      sequence.current = initial.sequence;
      if (abort.signal.aborted) return;
      setReasoning(
        (initial.recentActivity ?? [])
          .filter((event) => event.kind === "reasoning.delta")
          .map((event) => event.textDelta)
          .join(""),
      );
      setTools(
        Object.fromEntries(
          (initial.recentActivity ?? []).flatMap((event) =>
            event.detail.case === "toolCall" ? [[event.detail.value.id, event.detail.value]] : [],
          ),
        ),
      );
      setUser(who.user);
      setSession(initial.session);
      setMessages(sortMessages(initial.messages));
      setBefore(initial.nextMessageToken);
      setLoading(false);
      await refresh(abort.signal);
      let delay = 500;
      while (!abort.signal.aborted) {
        try {
          for await (const event of client.watchThread(
            { threadId: sessionId, afterCursor: cursor },
            { signal: abort.signal },
          )) {
            if (abort.signal.aborted) return;
            setReconnecting(false);
            delay = 500;
            cursor = event.cursor;
            const activity = event.activity;
            if (!activity) continue;
            sequence.current =
              activity.sequence > sequence.current ? activity.sequence : sequence.current;
            if (activity.detail.case === "message") {
              const message = activity.detail.value;
              setMessages((old) =>
                sortMessages([...old.filter((m) => m.id !== message.id), message]),
              );
            }
            if (
              activity.kind === "message.delta" &&
              activity.detail.case !== "messageChunk" &&
              activity.detail.case !== "message"
            ) {
              setMessages((old) =>
                old.map((m) =>
                  m.id === activity.entityId ? { ...m, text: m.text + activity.textDelta } : m,
                ),
              );
            }
            if (activity.detail.case === "messageChunk") {
              const chunk = activity.detail.value;
              setMessages((old) =>
                old.map((m) =>
                  m.id === chunk.messageId ? { ...m, text: m.text + chunk.textDelta } : m,
                ),
              );
            }
            if (activity.kind === "reasoning.delta")
              setReasoning((old) => old + activity.textDelta);
            if (activity.detail.case === "toolCall") {
              const tool = activity.detail.value;
              setTools((old) => ({ ...old, [tool.id]: tool }));
            }
            if (activity.detail.case === "typing") {
              const value = activity.detail.value;
              const participant = initial.session?.thread?.participants.find(
                (p) => p.id === value.participantId,
              );
              if (participant?.userId !== who.user?.id)
                setTyping(value.typing ? participant?.name || "Someone" : "");
            }
            scheduleRefresh();
          }
        } catch (error) {
          if (abort.signal.aborted) return;
          setError(errorText(error));
        }
        setReconnecting(true);
        await new Promise<void>((resolve) => {
          timer = setTimeout(resolve, delay);
          abort.signal.addEventListener(
            "abort",
            () => {
              clearTimeout(timer);
              resolve();
            },
            { once: true },
          );
        });
        delay = Math.min(delay * 2, 10000);
      }
    })().catch((error) => {
      if (!abort.signal.aborted) {
        setError(errorText(error));
        setLoading(false);
      }
    });
    return () => {
      abort.abort();
      clearTimeout(timer);
      clearTimeout(refreshTimer);
    };
  }, [client, sessionId, proxy, identity]);
  useEffect(() => {
    if (stick.current) scroll.current?.scrollTo({ top: scroll.current.scrollHeight });
  }, [messages, reasoning]);
  const markRead = () => {
    if (client && document.visibilityState === "visible" && document.hasFocus() && stick.current)
      void client
        .setReadState(
          { threadId: sessionId, throughSequence: sequence.current, unread: false },
          { signal: lifetime.current.signal },
        )
        .then(() => changed(proxy, identity))
        .catch(() => {});
  };
  useEffect(() => {
    const timer = setTimeout(markRead, 400);
    window.addEventListener("focus", markRead);
    document.addEventListener("visibilitychange", markRead);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("focus", markRead);
      document.removeEventListener("visibilitychange", markRead);
    };
  }, [client, messages, sessionId]);
  const act = async (action: () => Promise<unknown>) => {
    setError("");
    try {
      await action();
      await refresh();
      changed(proxy, identity);
    } catch (error) {
      if (!lifetime.current.signal.aborted) setError(errorText(error));
    }
  };
  function addFiles(list: FileList) {
    const added: Array<Pending> = [];
    for (const file of Array.from(list)) {
      if (file.size > 128 * 1024 * 1024) {
        setError("Each attachment must be 128 MB or smaller");
        continue;
      }
      const previewUrl = file.type.startsWith("image/") ? URL.createObjectURL(file) : undefined;
      if (previewUrl) blobs.current.add(previewUrl);
      added.push({
        id: randomUUID(),
        uploadId: randomUUID(),
        file,
        name: file.name,
        size: file.size,
        progress: 0,
        status: "ready",
        previewUrl,
      });
    }
    setFiles((old) => [...old, ...added]);
  }
  function removeFile(id: string) {
    setFiles((old) =>
      old.filter((file) => {
        if (file.id !== id) return true;
        if (file.previewUrl) {
          URL.revokeObjectURL(file.previewUrl);
          blobs.current.delete(file.previewUrl);
        }
        return false;
      }),
    );
  }
  function sendTyping(typing: boolean) {
    const participant = session?.thread?.participants.find(
      (p) => p.userId === user?.id && p.active,
    );
    if (client && participant)
      void client
        .setTyping(
          { threadId: sessionId, participantId: participant.id, typing },
          { signal: lifetime.current.signal },
        )
        .catch(() => {});
    lastTyping.current = typing ? Date.now() : 0;
  }
  async function send() {
    if (!client || !user || submitting || (!draft.trim() && !files.length)) return;
    const participant = session?.thread?.participants.find((p) => p.userId === user.id && p.active);
    if (!participant) return;
    setSubmitting(true);
    setError("");
    try {
      const attachmentIds: string[] = [];
      for (const file of files) {
        if (file.status !== "uploaded") {
          setFiles((old) => old.map((f) => (f.id === file.id ? { ...f, status: "uploading" } : f)));
          try {
            await client.uploadAttachment(
              {
                id: file.uploadId,
                threadId: sessionId,
                filename: file.name,
                mediaType: file.file.type || "application/octet-stream",
                content: new Uint8Array(await file.file.arrayBuffer()),
              },
              { signal: lifetime.current.signal },
            );
          } catch (error) {
            setFiles((old) =>
              old.map((f) =>
                f.id === file.id ? { ...f, status: "error", error: errorText(error) } : f,
              ),
            );
            throw error;
          }
          setFiles((old) =>
            old.map((f) => (f.id === file.id ? { ...f, status: "uploaded", progress: 1 } : f)),
          );
        }
        attachmentIds.push(file.uploadId);
      }
      const previous = pendingSend.current;
      const attempt =
        previous &&
        previous.text === draft &&
        previous.reply === reply?.id &&
        previous.attachmentIds.join() === attachmentIds.join()
          ? previous
          : { id: randomUUID(), text: draft, attachmentIds, reply: reply?.id };
      pendingSend.current = attempt;
      const result = await client.postMessage(
        {
          id: attempt.id,
          threadId: sessionId,
          participantId: participant.id,
          text: attempt.text,
          attachmentIds,
          inReplyToMessageId: attempt.reply,
        },
        { signal: lifetime.current.signal },
      );
      if (lifetime.current.signal.aborted) return;
      pendingSend.current = undefined;
      if (result.message)
        setMessages((old) =>
          sortMessages([...old.filter((m) => m.id !== result.message!.id), result.message!]),
        );
      sendTyping(false);
      form.reset();
      setReply(undefined);
      files.forEach((file) => removeFile(file.id));
      stick.current = true;
      await refresh();
      changed(proxy, identity);
    } catch (error) {
      if (!lifetime.current.signal.aborted) setError(errorText(error));
    } finally {
      if (!lifetime.current.signal.aborted) setSubmitting(false);
    }
  }
  const busy = ["pending", "running"].includes(session?.run?.invocationStatus ?? "");
  async function older() {
    if (!client || !before || paging) return;
    setPaging(true);
    const height = scroll.current?.scrollHeight ?? 0;
    try {
      const page = await client.listMessages(
        { threadId: sessionId, limit: 50, beforeMessageId: before },
        { signal: lifetime.current.signal },
      );
      setMessages((old) =>
        sortMessages([
          ...page.messages.filter((m) => !old.some((existing) => existing.id === m.id)),
          ...old,
        ]),
      );
      setBefore(page.nextPageToken);
      requestAnimationFrame(() => {
        if (scroll.current) scroll.current.scrollTop += scroll.current.scrollHeight - height;
      });
    } catch (error) {
      setError(errorText(error));
    } finally {
      setPaging(false);
    }
  }
  return (
    <section className={`tilde-chat tc-live tc-window ${className}`} aria-label="Chat">
      <header className="tc-header">
        <div>
          <strong>{session?.thread?.title || agent?.name || "Chat"}</strong>
          <small>{reconnecting ? "Reconnecting…" : busy ? "Working…" : agent?.name}</small>
        </div>
        <Input
          aria-label="Search messages"
          placeholder="Find in conversation"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        {session?.run && ["waiting", "failed"].includes(session.run.status) && !busy && (
          <Button onClick={() => void act(() => client!.resumeRun({ id: session.run!.id }))}>
            Resume
          </Button>
        )}
      </header>
      {loading && <p role="status">Loading conversation…</p>}
      {!loading && !user && error && <Button onClick={onRetry}>Retry connection</Button>}
      <div
        className="tc-transcript"
        ref={scroll}
        onScroll={() => {
          const el = scroll.current!;
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
          setLatest(!stick.current);
          if (stick.current) markRead();
        }}
      >
        {!search && before && (
          <Button disabled={paging} onClick={() => void older()}>
            {paging ? "Loading…" : "Load earlier messages"}
          </Button>
        )}
        {!loading && !messages.length && (
          <p className="tc-empty">Start a conversation with {agent?.name}.</p>
        )}
        {searching && <p role="status">Searching…</p>}
        {search && !searching && !matches.length && <p>No matching messages</p>}
        {search && matchCursor && (
          <Button
            onClick={() =>
              void act(async () => {
                const page = await client!.searchMessages({
                  threadId: sessionId,
                  query: search,
                  pageToken: matchCursor,
                  pageSize: 30,
                });
                setMatches((old) => [...old, ...page.messages]);
                setMatchCursor(page.nextPageToken);
              })
            }
          >
            More matches
          </Button>
        )}
        {(search ? matches : messages).map((message) => {
          const own =
            message.participantId ===
            session?.thread?.participants.find((p) => p.userId === user?.id)?.id;
          const name =
            session?.thread?.participants.find((p) => p.id === message.participantId)?.name ||
            (own ? "You" : agent?.name);
          return (
            <article
              key={message.id}
              className={`tc-message ${own ? "tc-own" : ""}`}
              aria-label={`${name} message`}
            >
              <small>{name}</small>
              {message.inReplyToMessageId && (
                <blockquote>
                  {messages.find((m) => m.id === message.inReplyToMessageId)?.text ||
                    "Reply to earlier message"}
                </blockquote>
              )}
              <MarkdownText text={message.text} />
              {client &&
                message.attachments.map((attachment) => (
                  <AttachmentView key={attachment.id} client={client} attachment={attachment} />
                ))}
              <footer>
                <time
                  dateTime={
                    message.createdAt
                      ? new Date(Number(message.createdAt.seconds) * 1000).toISOString()
                      : undefined
                  }
                >
                  {message.createdAt
                    ? new Date(Number(message.createdAt.seconds) * 1000).toLocaleTimeString([], {
                        hour: "2-digit",
                        minute: "2-digit",
                      })
                    : ""}
                </time>
                <Button
                  onClick={() => {
                    setReply(message);
                    inputRef.current?.focus();
                  }}
                >
                  Reply
                </Button>
                <Button onClick={() => void navigator.clipboard.writeText(message.text)}>
                  Copy
                </Button>
                {message.status === "aborted" && <span>Interrupted</span>}
              </footer>
            </article>
          );
        })}
        {reasoning && (
          <details className="tc-reasoning">
            <summary>Reasoning</summary>
            <MarkdownText text={reasoning} />
          </details>
        )}
        {/* The server already withholds hidden calls and summary-only detail; this only lays out
            what arrives. */}
        {Object.values(tools)
          .filter((tool) => tool.display !== ToolDisplay.HIDDEN)
          .map((tool) =>
            tool.display === ToolDisplay.SUMMARY ? (
              <p key={tool.id} className="tc-tool">
                {tool.summary || tool.name} · {tool.status}
              </p>
            ) : (
              <details key={tool.id} className="tc-tool">
                <summary>
                  {tool.summary || tool.name} · {tool.status}
                </summary>
                <pre>{tool.inputJson}</pre>
                <pre>{tool.outputJson || tool.error}</pre>
              </details>
            ),
          )}
        {typing && <p role="status">{typing} is typing…</p>}
      </div>
      {latest && (
        <Button
          className="tc-latest"
          onClick={() => {
            stick.current = true;
            scroll.current?.scrollTo({ top: scroll.current.scrollHeight, behavior: "smooth" });
            setLatest(false);
            setSearch("");
          }}
        >
          Jump to latest
        </Button>
      )}
      {queue.length > 0 && (
        <ol className="tc-queue" aria-label="Queued messages">
          {queue.map((item, index) => (
            <li
              key={item.id}
              draggable
              onDragStart={(event) => event.dataTransfer.setData("text/tilde-queue", item.id)}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                event.preventDefault();
                const id = event.dataTransfer.getData("text/tilde-queue");
                if (queue.some((q) => q.id === id))
                  void act(() =>
                    client!.reorderQueuedMessage({ threadId: sessionId, id, beforeId: item.id }),
                  );
              }}
            >
              <span>{item.text}</span>
              <Button
                aria-label="Move queued message up"
                disabled={index === 0}
                onClick={() =>
                  void act(() =>
                    client!.reorderQueuedMessage({
                      threadId: sessionId,
                      id: item.id,
                      beforeId: queue[index - 1].id,
                    }),
                  )
                }
              >
                ↑
              </Button>
              <Button
                aria-label="Move queued message down"
                disabled={index === queue.length - 1}
                onClick={() =>
                  void act(() =>
                    client!.reorderQueuedMessage({
                      threadId: sessionId,
                      id: item.id,
                      beforeId: queue[index + 2]?.id ?? "",
                    }),
                  )
                }
              >
                ↓
              </Button>
              <Button
                onClick={() =>
                  void act(() => client!.steerQueuedMessage({ threadId: sessionId, id: item.id }))
                }
              >
                Steer now
              </Button>
              <Button
                onClick={() =>
                  void act(async () => {
                    await client!.removeQueuedMessage({ threadId: sessionId, id: item.id });
                    form.setValue("draft", item.text);
                    inputRef.current?.focus();
                  })
                }
              >
                Edit
              </Button>
              <Button
                onClick={() =>
                  void act(() => client!.removeQueuedMessage({ threadId: sessionId, id: item.id }))
                }
              >
                Remove
              </Button>
            </li>
          ))}
        </ol>
      )}
      <ChatComposer
        agentAvailable={!!client && !loading}
        busy={busy}
        submitting={submitting}
        dragging={dragging}
        expanded={draft.includes("\n")}
        draft={draft}
        error={error}
        reply={reply ? { label: "Replying", text: reply.text } : undefined}
        attachments={files}
        inputRef={inputRef}
        fileInputRef={fileInputRef}
        onSubmit={form.handleSubmit(send)}
        onDraftChange={(value) => {
          form.setValue("draft", value);
          if (!value || Date.now() - lastTyping.current > 2000) sendTyping(!!value);
        }}
        onDragStateChange={setDragging}
        onFilesAdded={addFiles}
        onRemoveAttachment={removeFile}
        onCancelReply={() => setReply(undefined)}
        onStop={() =>
          void act(() => client!.cancelInvocation({ invocationId: session!.run!.invocationId }))
        }
        onFocus={() => sendTyping(true)}
        onBlur={() => sendTyping(false)}
      />
    </section>
  );
}
function sortMessages(messages: Message[]) {
  return messages.sort(
    (a, b) =>
      Number((a.createdAt?.seconds ?? 0n) - (b.createdAt?.seconds ?? 0n)) ||
      (a.createdAt?.nanos ?? 0) - (b.createdAt?.nanos ?? 0) ||
      a.id.localeCompare(b.id),
  );
}
