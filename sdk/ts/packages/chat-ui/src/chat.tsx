"use client";
import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { ReplyIcon, XIcon } from "lucide-react";
import { AgentAvatar } from "@trytilde/agent-avatar";
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
import { QueuePanel } from "./queue.js";
import { ConversationMessage } from "./dispatch/conversation-message.js";
import { ToolChipsBlock, type ToolChipRow } from "./dispatch/tool-chips-block.js";
import { TraceBlock } from "./dispatch/trace-block.js";
import { SearchIcon } from "./dispatch/icons.js";
import {
  ChatComposerDock,
  ChatFindBar,
  ConversationSkeleton,
  ScrollToLatestButton,
  ThinkingIndicator,
  TranscriptTimeSeparator,
  dayKey,
} from "./dispatch/chat-surface.js";
import {
  randomUUID,
  chatClient,
  changed,
  errorText,
  listAgents,
  type ChatAgent,
  type ChatFetch,
} from "./client.js";
import { themeStyle, type Theme } from "./theme.js";
import { millis, noActivity, record, timeline, type Activities } from "./timeline.js";
export interface ChatProps {
  sessionId: string;
  identity: string;
  proxy: string;
  agentId?: string;
  className?: string;
  /** The host's theme (the one `@trytilde/embed` takes); tokens left out keep the defaults. */
  theme?: Theme;
  /** `false` leaves only the transcript and the message bar: no agent avatar and name, thread
   * title, Resume or find control. Default true. */
  header?: boolean;
  /** Replaces the cookie-carrying fetch, e.g. to send a bearer token (a Tilde space's chat).
   * Keep it stable (define it once or memoise it): a new function reconnects the chat. */
  fetch?: ChatFetch;
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
  theme,
  header = true,
  fetch: proxyFetch,
  onRetry,
}: ChatProps & { onRetry: () => void }) {
  const [agent, setAgent] = useState<ChatAgent>();
  const [session, setSession] = useState<Session>();
  const [user, setUser] = useState<User>();
  const [messages, setMessages] = useState<Message[]>([]);
  const [queue, setQueue] = useState<QueuedInput[]>([]);
  const [tools, setTools] = useState<Record<string, ToolCall>>({});
  const [activities, setActivities] = useState<Activities>(noActivity);
  // Messages posted from this composer, placed last until their activity arrives.
  const [posted, setPosted] = useState<ReadonlySet<string>>(new Set());
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
  const [findOpen, setFindOpen] = useState(false);
  const [matchIndex, setMatchIndex] = useState(0);
  const [menuFor, setMenuFor] = useState<string>();
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
  const client = useMemo(
    () => (agent ? chatClient(proxy, agent.id, proxyFetch) : undefined),
    [proxy, agent, proxyFetch],
  );
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
      const agents = await listAgents(proxy, abort.signal, proxyFetch);
      const candidates = agentId ? agents.filter((a) => a.id === agentId) : agents;
      if (candidates.length === 1) {
        if (!abort.signal.aborted) setAgent(candidates[0]);
        return;
      }
      for (const candidate of candidates) {
        try {
          await chatClient(proxy, candidate.id, proxyFetch).getSession(
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
  }, [proxy, identity, agentId, sessionId, proxyFetch]);
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
      const recent = initial.recentActivity ?? [];
      setActivities(record(noActivity, recent));
      setTools(
        Object.fromEntries(
          recent.flatMap((event) =>
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
            setActivities((old) => record(old, [activity]));
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
  }, [messages, activities, tools]);
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
    // Someone reading a thread they are not in yet (a Tilde space's chat) posts naming no
    // participant: the server binds the post to them, or refuses it.
    const participant = session?.thread?.participants.find((p) => p.userId === user.id && p.active);
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
          participantId: participant?.id ?? "",
          text: attempt.text,
          attachmentIds,
          inReplyToMessageId: attempt.reply,
        },
        { signal: lifetime.current.signal },
      );
      if (lifetime.current.signal.aborted) return;
      pendingSend.current = undefined;
      if (result.message) {
        const id = result.message.id;
        setPosted((old) => new Set(old).add(id));
        setMessages((old) =>
          sortMessages([...old.filter((m) => m.id !== result.message!.id), result.message!]),
        );
      }
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
  const me = session?.thread?.participants.find((p) => p.userId === user?.id);
  const others = session?.thread?.participants.filter((p) => p.id !== me?.id) ?? [];
  const agentName = agent?.name || "Chat";
  // Search shows only matching messages; otherwise messages, tool calls and reasoning interleave
  // in the order they happened.
  const items = useMemo(
    () =>
      search
        ? matches.map((message) => ({
            kind: "message" as const,
            key: `message:${message.id}`,
            at: message.createdAt ? millis(message.createdAt) : 0,
            message,
          }))
        : timeline(
            messages,
            Object.values(tools).filter((tool) => tool.display !== ToolDisplay.HIDDEN),
            activities,
            posted,
          ),
    [search, matches, messages, tools, activities, posted],
  );
  // Reasoning is still being thought only while it is the newest entry.
  const newest = items.at(-1);
  const thinking = newest?.kind === "reasoning" ? newest.key : undefined;
  const dayOf = (item?: (typeof items)[number]) => (item?.at ? dayKey(new Date(item.at)) : "");
  const rows = items.map((item, index) => {
    const sameSender = (other?: (typeof items)[number]) =>
      item.kind === "message" &&
      other?.kind === "message" &&
      other.message.participantId === item.message.participantId &&
      dayOf(other) === dayOf(item);
    const itemDay = dayOf(item);
    return {
      item,
      showDay: !search && !!itemDay && itemDay !== dayOf(items[index - 1]),
      itemDay,
      continuedPrevious: sameSender(items[index - 1]),
      continuedNext: sameSender(items[index + 1]),
    };
  });
  function jumpToLatest() {
    stick.current = true;
    scroll.current?.scrollTo({ top: scroll.current.scrollHeight, behavior: "smooth" });
    setLatest(false);
  }
  function stepMatch(delta: number) {
    if (!matches.length) return;
    const next = (matchIndex + delta + matches.length) % matches.length;
    setMatchIndex(next);
    scroll.current
      ?.querySelector(`[data-message-id="${CSS.escape(matches[next].id)}"]`)
      ?.scrollIntoView({ block: "center", behavior: "smooth" });
  }
  return (
    <section
      className={`tilde-chat rich-chat tc-chat ${className}`}
      data-scheme={theme?.scheme}
      style={themeStyle(theme)}
      aria-label="Chat"
    >
      <div className="chat-pane">
        {header ? (
          <header className="chat-header" data-find-open={findOpen || undefined}>
            <div className="chat-identity">
              {agent ? (
                <AgentAvatar id={agent.id} state={busy ? "working" : "idle"} />
              ) : (
                <span className="agent-avatar">{agentName.charAt(0).toUpperCase()}</span>
              )}
              <div className="chat-title">
                <h2
                  title={
                    session?.thread?.title ? `${agentName} · ${session.thread.title}` : agentName
                  }
                >
                  <span className="chat-participant-names">{agentName}</span>
                  {session?.thread?.title ? (
                    <>
                      <span aria-hidden="true"> · </span>
                      <span>{session.thread.title}</span>
                    </>
                  ) : null}
                </h2>
                {reconnecting || busy ? (
                  <span className="chat-status" role="status">
                    {reconnecting ? "Reconnecting…" : "Working…"}
                  </span>
                ) : null}
              </div>
            </div>
            {findOpen ? (
              <div className="chat-header-find">
                <ChatFindBar
                  query={search}
                  matchCount={matches.length}
                  currentOrdinal={matches.length ? matchIndex + 1 : 0}
                  loading={searching}
                  onQueryChange={(value) => {
                    setSearch(value);
                    setMatchIndex(0);
                  }}
                  onStepNext={() => stepMatch(1)}
                  onStepPrevious={() => stepMatch(-1)}
                  onClose={() => {
                    setFindOpen(false);
                    setSearch("");
                  }}
                />
              </div>
            ) : null}
            <div className="chat-actions" hidden={findOpen}>
              {session?.run && ["waiting", "failed"].includes(session.run.status) && !busy ? (
                <button
                  type="button"
                  className="chat-resume"
                  onClick={() => void act(() => client!.resumeRun({ id: session.run!.id }))}
                >
                  Resume
                </button>
              ) : null}
              <button
                type="button"
                aria-label="Find in conversation"
                title="Find in conversation"
                onClick={() => setFindOpen(true)}
              >
                <SearchIcon />
              </button>
            </div>
          </header>
        ) : null}
        <div
          className="conversation"
          aria-live="polite"
          ref={scroll}
          onScroll={() => {
            const el = scroll.current!;
            stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
            setLatest(!stick.current);
            if (stick.current) markRead();
          }}
        >
          {loading ? (
            <ConversationSkeleton />
          ) : (
            <div className="message-list">
              {!user && error ? (
                <div className="tc-state" role="alert">
                  <p>{error}</p>
                  <button type="button" onClick={onRetry}>
                    Retry connection
                  </button>
                </div>
              ) : null}
              {!search && before ? (
                <button
                  type="button"
                  className="tc-load-earlier"
                  disabled={paging}
                  onClick={() => void older()}
                >
                  {paging ? "Loading…" : "Load earlier messages"}
                </button>
              ) : null}
              {user && !messages.length ? (
                <div className="tc-state">
                  {agent ? <AgentAvatar id={agent.id} /> : null}
                  <p>Start a conversation with {agent?.name}.</p>
                </div>
              ) : null}
              {search && !searching && !matches.length ? (
                <p className="tc-state">No matching messages</p>
              ) : null}
              {rows.map(({ item, showDay, itemDay, continuedPrevious, continuedNext }) => {
                const separator = showDay ? <TranscriptTimeSeparator dateTime={itemDay} /> : null;
                if (item.kind === "reasoning")
                  return (
                    <Fragment key={item.key}>
                      {separator}
                      <div className="chat-event" role="group" aria-label="Reasoning">
                        <TraceBlock
                          activeLabel="Thinking"
                          doneLabel="Reasoning"
                          working={busy && item.key === thinking}
                          rows={item.text
                            .split(/\n{2,}/)
                            .filter(Boolean)
                            .map((text, i) => ({ id: String(i), primary: text, prose: true }))}
                        />
                      </div>
                    </Fragment>
                  );
                // The server already withholds hidden calls and summary-only detail; this only
                // lays out what arrives.
                if (item.kind === "tools")
                  return (
                    <Fragment key={item.key}>
                      {separator}
                      <div className="chat-event" role="group" aria-label="Tool calls">
                        <ToolChipsBlock
                          headerLabel={`${item.tools.length} tool call${item.tools.length === 1 ? "" : "s"}`}
                          rows={item.tools.map(toolRow)}
                        />
                      </div>
                    </Fragment>
                  );
                const message = item.message;
                const own = message.participantId === me?.id;
                const created = message.createdAt
                  ? new Date(Number(message.createdAt.seconds) * 1000)
                  : undefined;
                const sender =
                  others.length > 1 && !own
                    ? session?.thread?.participants.find((p) => p.id === message.participantId)
                    : undefined;
                const quoted = message.inReplyToMessageId
                  ? messages.find((m) => m.id === message.inReplyToMessageId)?.text ||
                    "Reply to earlier message"
                  : undefined;
                return (
                  <Fragment key={item.key}>
                    {separator}
                    <ConversationMessage
                      messageId={message.id}
                      role={own ? "user" : "assistant"}
                      createdAt={created?.toISOString() ?? ""}
                      continuedPrevious={continuedPrevious}
                      continuedNext={continuedNext}
                      mediaOnly={!message.text.trim() && message.attachments.length > 0}
                      sender={
                        sender ? (
                          <>
                            {sender.agentId ? <AgentAvatar id={sender.agentId} /> : null}
                            <span>{sender.name}</span>
                          </>
                        ) : undefined
                      }
                      quote={quoted}
                      attachments={
                        client && message.attachments.length
                          ? message.attachments.map((attachment) => (
                              <AttachmentView
                                key={attachment.id}
                                client={client}
                                attachment={attachment}
                              />
                            ))
                          : undefined
                      }
                      notice={message.status === "aborted" ? "Interrupted" : undefined}
                      menuOpen={menuFor === message.id}
                      onReply={() => {
                        setReply(message);
                        inputRef.current?.focus();
                      }}
                      onToggleMenu={() =>
                        setMenuFor((open) => (open === message.id ? undefined : message.id))
                      }
                      onCopy={() => {
                        setMenuFor(undefined);
                        void navigator.clipboard.writeText(message.text);
                      }}
                    >
                      {message.text.trim() ? <MarkdownText text={message.text} /> : null}
                    </ConversationMessage>
                  </Fragment>
                );
              })}
              {search && matchCursor ? (
                <button
                  type="button"
                  className="tc-load-earlier"
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
                </button>
              ) : null}
              {!search && typing ? (
                <ThinkingIndicator>{typing} is typing…</ThinkingIndicator>
              ) : null}
              {!search && !typing && busy ? (
                <ThinkingIndicator>{agentName} is working…</ThinkingIndicator>
              ) : null}
            </div>
          )}
        </div>
        <ChatComposerDock>
          {reply ? (
            <div className="prompt-reply" aria-label="Replying to message">
              <ReplyIcon className="prompt-reply-icon" aria-hidden />
              <div>
                <strong>
                  {session?.thread?.participants.find((p) => p.id === reply.participantId)?.name ||
                    agentName}
                </strong>
                <p>{reply.text}</p>
              </div>
              <button
                type="button"
                aria-label="Cancel reply"
                disabled={submitting}
                onClick={() => setReply(undefined)}
              >
                <XIcon aria-hidden />
              </button>
            </div>
          ) : (
            <div className="prompt-above">
              {latest ? (
                <div className="prompt-notices">
                  <ScrollToLatestButton
                    onClick={() => {
                      jumpToLatest();
                      setSearch("");
                    }}
                  />
                </div>
              ) : null}
              {queue.length > 0 ? (
                <QueuePanel
                  items={queue}
                  onMove={(id, beforeId) =>
                    void act(() =>
                      client!.reorderQueuedMessage({ threadId: sessionId, id, beforeId }),
                    )
                  }
                  onSteer={(id) =>
                    void act(() => client!.steerQueuedMessage({ threadId: sessionId, id }))
                  }
                  onEdit={(item) =>
                    void act(async () => {
                      await client!.removeQueuedMessage({ threadId: sessionId, id: item.id });
                      form.setValue("draft", item.text);
                      inputRef.current?.focus();
                    })
                  }
                  onRemove={(id) =>
                    void act(() => client!.removeQueuedMessage({ threadId: sessionId, id }))
                  }
                />
              ) : null}
            </div>
          )}
          <ChatComposer
            agentAvailable={!!client && !loading}
            busy={busy}
            submitting={submitting}
            dragging={dragging}
            expanded={draft.includes("\n")}
            draft={draft}
            error={error}
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
            onStop={() =>
              void act(() => client!.cancelInvocation({ invocationId: session!.run!.invocationId }))
            }
            onFocus={() => sendTyping(true)}
            onBlur={() => sendTyping(false)}
          />
        </ChatComposerDock>
      </div>
    </section>
  );
}
function toolRow(tool: ToolCall): ToolChipRow {
  const failed = Boolean(tool.error) || /fail|error/.test(tool.status);
  const detail =
    tool.display === ToolDisplay.SUMMARY
      ? undefined
      : [
          ...(tool.inputJson ? [{ text: "Input" }, ...lines(tool.inputJson)] : []),
          ...(tool.outputJson ? [{ text: "Output" }, ...lines(tool.outputJson)] : []),
          ...(tool.error ? lines(tool.error, "error") : []),
        ];
  return {
    id: tool.id,
    icon: "tool",
    label: tool.summary || tool.name,
    chip: tool.summary ? tool.name : undefined,
    mono: true,
    detailMono: true,
    detail: detail?.length ? detail : undefined,
    failed,
    pending: !failed && /running|pending|started/.test(tool.status),
  };
}
function lines(text: string, tone?: "error") {
  return text.split("\n").map((line) => ({ text: line, ...(tone ? { tone } : {}) }));
}
function sortMessages(messages: Message[]) {
  return messages.sort(
    (a, b) =>
      Number((a.createdAt?.seconds ?? 0n) - (b.createdAt?.seconds ?? 0n)) ||
      (a.createdAt?.nanos ?? 0) - (b.createdAt?.nanos ?? 0) ||
      a.id.localeCompare(b.id),
  );
}
