"use client";
import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { Button, Input } from "@trytilde/connection-ui";
import type { Session } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import { chatClient, changed, errorText, listAgents, subscribe, type ChatAgent } from "./client.js";
export interface ListSessionsProps {
  identity: string;
  proxy: string;
  selectedSessionId?: string;
  selectedAgentId?: string;
  onSelectSession: (sessionId: string, agentId: string) => void;
  className?: string;
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
  const refresh = () => setVersion((value) => value + 1);
  useEffect(() => {
    const timer = setTimeout(() => setSearch(query), 200);
    return () => clearTimeout(timer);
  }, [query]);
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
      const agents = await listAgents(proxy, abort.signal);
      const pages = await Promise.all(
        agents.map(async (agent) => {
          const wanted = groups.find((group) => group.agent.id === agent.id)?.sessions.length ?? 30;
          const sessions: Session[] = [];
          let next = "";
          do {
            const page = await chatClient(proxy, agent.id).listSessions(
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
  }, [proxy, identity, search, version]);
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
      const result = await chatClient(proxy, agent.id).createThread({
        title: "New conversation",
        primaryAgentId: agent.id,
      });
      if (result.thread) onSelectSession(result.thread.id, agent.id);
    });
  }
  async function more(group: Group) {
    await act(async () => {
      const page = await chatClient(proxy, group.agent.id).listSessions({
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
    <nav className={`tilde-chat tc-live tc-sidebar ${className}`} aria-label="Chat sessions">
      <header>
        <strong>Conversations</strong>
        <Input
          aria-label="Search conversations and messages"
          placeholder="Search conversations…"
          value={query}
          onChange={(event) => {
            setGroups([]);
            setQuery(event.target.value);
          }}
        />
      </header>
      {error && (
        <p role="alert">
          {error}
          <Button onClick={refresh}>Retry</Button>
        </p>
      )}
      {loading && <p role="status">Loading conversations…</p>}
      {groups.map((group) => (
        <section key={group.agent.id} className="tc-agent-group">
          <header>
            <h3>{group.agent.name}</h3>
            <Button
              disabled={busy}
              aria-label={`New conversation with ${group.agent.name}`}
              onClick={() => void create(group.agent)}
            >
              +
            </Button>
          </header>
          {!group.sessions.length && (
            <p className="tc-empty">
              {search ? "No matching conversations" : "No conversations yet"}
            </p>
          )}
          <ul>
            {group.sessions.map((session) => {
              const thread = session.thread!;
              const selected =
                thread.id === selectedSessionId &&
                (!selectedAgentId || selectedAgentId === group.agent.id);
              return (
                <li key={thread.id} className={selected ? "tc-selected" : ""}>
                  <Button
                    className="tc-session-select"
                    aria-current={selected ? "page" : undefined}
                    onClick={() => onSelectSession(thread.id, group.agent.id)}
                  >
                    <span>
                      {session.unread && <span className="tc-unread" aria-label="Unread" />}
                      <strong>{thread.title}</strong>
                      <small>{session.preview}</small>
                    </span>
                  </Button>
                  <details className="tc-session-menu">
                    <summary aria-label={`Actions for ${thread.title}`}>•••</summary>
                    <div>
                      <Button
                        disabled={busy}
                        onClick={() => {
                          setEditing({ id: thread.id, agentId: group.agent.id });
                          form.reset({ title: thread.title });
                        }}
                      >
                        Rename
                      </Button>
                      <Button
                        disabled={busy}
                        onClick={() =>
                          void act(async () => {
                            const client = chatClient(proxy, group.agent.id);
                            const current = await client.getSession({ threadId: thread.id });
                            await client.setReadState({
                              threadId: thread.id,
                              throughSequence: session.unread ? current.sequence : 0n,
                              unread: !session.unread,
                            });
                          })
                        }
                      >
                        {session.unread ? "Mark read" : "Mark unread"}
                      </Button>
                    </div>
                  </details>
                </li>
              );
            })}
          </ul>
          {group.next && (
            <Button disabled={busy} onClick={() => void more(group)}>
              More conversations
            </Button>
          )}
        </section>
      ))}
      {editing && (
        <form
          className="tc-rename"
          onSubmit={form.handleSubmit(({ title }) =>
            act(async () => {
              await chatClient(proxy, editing.agentId).renameSession({
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
          <Button type="submit" disabled={busy}>
            Save
          </Button>
          <Button type="button" onClick={() => setEditing(undefined)}>
            Cancel
          </Button>
        </form>
      )}
    </nav>
  );
}
