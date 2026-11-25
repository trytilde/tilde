"use client";
import { useEffect, useRef, useState } from "react";
import { AgentAvatar } from "@trytilde/agent-avatar";
import { useForm } from "react-hook-form";
import { Button, Input } from "@trytilde/connection-ui";
import type { Session } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import { MoreIcon, PlusIcon, SearchIcon } from "./dispatch/icons.js";
import {
  chatClient,
  changed,
  errorText,
  listAgents,
  subscribe,
  type ChatAgent,
  type ChatFetch,
} from "./client.js";
import { themeStyle, type Theme } from "./theme.js";
export interface ListSessionsProps {
  identity: string;
  proxy: string;
  selectedSessionId?: string;
  selectedAgentId?: string;
  onSelectSession: (sessionId: string, agentId: string) => void;
  className?: string;
  /** The host's theme (the one `@trytilde/embed` takes); tokens left out keep the defaults. */
  theme?: Theme;
  /** Replaces the cookie-carrying fetch, as `Chat.fetch` does. Keep it stable (define it once
   * or memoise it): a new function reloads the list. */
  fetch?: ChatFetch;
}
interface Group {
  agent: ChatAgent;
  sessions: Session[];
  next: string;
}
export function ListSessions(props: ListSessionsProps) {
  return <Sessions key={`${props.proxy}:${props.identity}`} {...props} />;
}
function Sessions({
  identity,
  proxy,
  selectedSessionId,
  selectedAgentId,
  onSelectSession,
  className = "",
  theme,
  fetch: proxyFetch,
}: ListSessionsProps) {
  const [groups, setGroups] = useState<Group[]>([]);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [version, setVersion] = useState(0);
  const [editing, setEditing] = useState<{ id: string; agentId: string }>();
  const [busy, setBusy] = useState(false);
  const form = useForm({ defaultValues: { title: "" } });
  const searchRef = useRef<HTMLInputElement>(null);
  const refresh = () => setVersion((value) => value + 1);
  useEffect(() => {
    const timer = setTimeout(() => setSearch(query), 200);
    return () => clearTimeout(timer);
  }, [query]);
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey) && !event.defaultPrevented) {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener("keydown", focusSearch);
    return () => document.removeEventListener("keydown", focusSearch);
  }, []);
  useEffect(() => {
    const stop = subscribe(proxy, identity, refresh);
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") refresh();
    }, 10000);
    return () => {
      stop();
      clearInterval(timer);
    };
  }, [proxy, identity]);
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    void (async () => {
      const agents = await listAgents(proxy, abort.signal, proxyFetch);
      const pages = await Promise.all(
        agents.map(async (agent) => {
          const wanted = groups.find((group) => group.agent.id === agent.id)?.sessions.length ?? 30;
          const sessions: Session[] = [];
          let next = "";
          do {
            const page = await chatClient(proxy, agent.id, proxyFetch).listSessions(
              { query: search, pageSize: 30, pageToken: next },
              { signal: abort.signal },
            );
            sessions.push(...page.sessions);
            next = page.nextPageToken;
          } while (next && sessions.length < wanted);
          return { agent, sessions, next };
        }),
      );
      if (!abort.signal.aborted) setGroups(pages);
    })()
      .catch((error) => {
        if (!abort.signal.aborted) setError(errorText(error));
      })
      .finally(() => {
        if (!abort.signal.aborted) setLoading(false);
      });
    return () => abort.abort();
  }, [proxy, identity, search, version, proxyFetch]);
  async function act(operation: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await operation();
      changed(proxy, identity);
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function create(agent: ChatAgent) {
    await act(async () => {
      const result = await chatClient(proxy, agent.id, proxyFetch).createThread({
        title: "New conversation",
        primaryAgentId: agent.id,
      });
      if (result.thread) onSelectSession(result.thread.id, agent.id);
    });
  }
  async function more(group: Group) {
    await act(async () => {
      const page = await chatClient(proxy, group.agent.id, proxyFetch).listSessions({
        query: search,
        pageToken: group.next,
        pageSize: 30,
      });
      setGroups((old) =>
        old.map((g) =>
          g.agent.id === group.agent.id
            ? {
                ...g,
                sessions: [
                  ...g.sessions,
                  ...page.sessions.filter(
                    (s) => !g.sessions.some((existing) => existing.thread?.id === s.thread?.id),
                  ),
                ],
                next: page.nextPageToken,
              }
            : g,
        ),
      );
    });
  }
  return (
    <nav
      className={`tilde-chat tc-sessions ${className}`}
      data-scheme={theme?.scheme}
      style={themeStyle(theme)}
      aria-label="Chat sessions"
    >
      <div className="tc-sessions-search">
        <label className="tc-search">
          <SearchIcon />
          <input
            ref={searchRef}
            aria-label="Search conversations and messages"
            placeholder="Search"
            value={query}
            onChange={(event) => {
              setGroups([]);
              setQuery(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Escape") event.currentTarget.blur();
            }}
          />
          <kbd aria-hidden="true">⌘K</kbd>
        </label>
      </div>
      <div className="tc-sessions-list">
        {error && (
          <div className="tc-state" role="alert">
            <p>{error}</p>
            <button type="button" onClick={refresh}>
              Retry
            </button>
          </div>
        )}
        {loading && (
          <div aria-label="Loading conversations" className="tc-session-skeletons" role="status">
            {Array.from({ length: 4 }, (_, index) => (
              <div aria-hidden="true" className="tc-session-row" key={index}>
                <span className="tc-skeleton-avatar" />
                <span className="tc-session-meta">
                  <i />
                  <i />
                </span>
              </div>
            ))}
          </div>
        )}
        {!loading && !error && !groups.length && (
          <p className="tc-state">No agents are available.</p>
        )}
        {groups.map((group) => (
          <section key={group.agent.id} className="tc-agent-group">
            <header>
              <h3>{group.agent.name}</h3>
              <button
                type="button"
                disabled={busy}
                aria-label={`New conversation with ${group.agent.name}`}
                title={`New conversation with ${group.agent.name}`}
                onClick={() => void create(group.agent)}
              >
                <PlusIcon />
              </button>
            </header>
            {!group.sessions.length && (
              <p className="tc-group-empty">
                {search ? "No matching conversations" : "No conversations yet"}
              </p>
            )}
            <ul>
              {group.sessions.map((session) => {
                const thread = session.thread!;
                const selected =
                  thread.id === selectedSessionId &&
                  (!selectedAgentId || selectedAgentId === group.agent.id);
                const working = ["pending", "running"].includes(
                  session.run?.invocationStatus ?? "",
                );
                return (
                  <li key={thread.id} data-selected={selected || undefined}>
                    <button
                      type="button"
                      className="tc-session-row"
                      aria-current={selected ? "page" : undefined}
                      aria-labelledby={`tc-title-${thread.id}`}
                      aria-describedby={session.preview ? `tc-preview-${thread.id}` : undefined}
                      data-unread={session.unread || undefined}
                      onClick={() => onSelectSession(thread.id, group.agent.id)}
                    >
                      <AgentAvatar
                        id={group.agent.id}
                        state={working ? "loading" : selected ? "listening" : "idle"}
                      />
                      <span className="tc-session-meta">
                        <strong id={`tc-title-${thread.id}`}>{thread.title}</strong>
                        <span>
                          {session.preview ? (
                            <small id={`tc-preview-${thread.id}`}>{session.preview}</small>
                          ) : null}
                          {session.unread && (
                            <span className="tc-unread" aria-label="Unread" role="status" />
                          )}
                        </span>
                      </span>
                    </button>
                    <details className="tc-session-menu">
                      <summary aria-label={`Actions for ${thread.title}`}>
                        <MoreIcon />
                      </summary>
                      <div role="menu">
                        <button
                          type="button"
                          role="menuitem"
                          disabled={busy}
                          onClick={(event) => {
                            event.currentTarget.closest("details")?.removeAttribute("open");
                            setEditing({ id: thread.id, agentId: group.agent.id });
                            form.reset({ title: thread.title });
                          }}
                        >
                          Rename
                        </button>
                        <button
                          type="button"
                          role="menuitem"
                          disabled={busy}
                          onClick={(event) => {
                            event.currentTarget.closest("details")?.removeAttribute("open");
                            void act(async () => {
                              const client = chatClient(proxy, group.agent.id, proxyFetch);
                              const current = await client.getSession({ threadId: thread.id });
                              await client.setReadState({
                                threadId: thread.id,
                                throughSequence: session.unread ? current.sequence : 0n,
                                unread: !session.unread,
                              });
                            });
                          }}
                        >
                          {session.unread ? "Mark read" : "Mark unread"}
                        </button>
                      </div>
                    </details>
                  </li>
                );
              })}
            </ul>
            {group.next && (
              <button
                type="button"
                className="tc-more"
                disabled={busy}
                onClick={() => void more(group)}
              >
                More conversations
              </button>
            )}
          </section>
        ))}
      </div>
      {editing && (
        <form
          className="tc-rename"
          onSubmit={form.handleSubmit(({ title }) =>
            act(async () => {
              await chatClient(proxy, editing.agentId, proxyFetch).renameSession({
                threadId: editing.id,
                title,
              });
              setEditing(undefined);
            }),
          )}
        >
          <label htmlFor={`rename-${editing.id}`}>Conversation name</label>
          <Input
            id={`rename-${editing.id}`}
            autoFocus
            {...form.register("title", { required: true, maxLength: 200 })}
          />
          {form.formState.errors.title && <p role="alert">Enter a name of up to 200 characters.</p>}
          <div>
            <Button type="button" variant="ghost" onClick={() => setEditing(undefined)}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Save
            </Button>
          </div>
        </form>
      )}
    </nav>
  );
}
