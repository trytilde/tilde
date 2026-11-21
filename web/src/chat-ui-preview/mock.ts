/**
 * In-browser chat proxy for the chat-ui preview page. `installMockProxy` patches `window.fetch`
 * so `/preview-chat/agents` and `/preview-chat/<agent>/tilde.provider.tilde.v1.ChatService/*`
 * are answered by a real Connect router over fixtures; everything else passes through.
 */
import { create } from "@bufbuild/protobuf";
import { createConnectRouter } from "@connectrpc/connect";
import { createFetchHandler } from "@connectrpc/connect/protocol";
import { ChatService, SessionSchema } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import {
  ActivitySchema,
  MessageSchema,
  QueuedInputSchema,
  ThreadSchema,
  ToolDisplay,
  UserSchema,
  type Message,
  type QueuedInput,
  type Thread,
} from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

export const PROXY = "/preview-chat";
const agents = [
  { id: "triage", name: "Triage" },
  { id: "writer", name: "Writer" },
];
const user = create(UserSchema, { id: "maya", name: "Maya Okafor" });
const at = (minutesAgo: number) => ({
  seconds: BigInt(Math.floor(Date.now() / 1000) - minutesAgo * 60),
  nanos: 0,
});
function thread(id: string, agentId: string, title: string, people: string[] = []): Thread {
  return create(ThreadSchema, {
    id,
    title,
    primaryAgentId: agentId,
    participants: [
      { id: `${id}-maya`, userId: user.id, name: user.name, active: true },
      {
        id: `${id}-agent`,
        agentId,
        name: agents.find((a) => a.id === agentId)!.name,
        active: true,
      },
      ...people.map((name) => ({ id: `${id}-${name}`, userId: name, name, active: true })),
    ],
  });
}
const message = (
  threadId: string,
  id: string,
  who: string,
  minutesAgo: number,
  text: string,
  extra: { inReplyToMessageId?: string } = {},
) =>
  create(MessageSchema, {
    id: `${threadId}-${id}`,
    threadId,
    participantId: `${threadId}-${who}`,
    text,
    status: "completed",
    createdAt: at(minutesAgo),
    ...extra,
  });

const threads = [
  thread("sev2", "triage", "SEV-2: checkout latency spike"),
  thread("standup", "triage", "Platform standup notes", ["Jonas"]),
  thread("launch", "writer", "Launch post draft"),
];
const messages: Record<string, Message[]> = {
  sev2: [
    message("sev2", "m1", "agent", 60 * 26, "Sentry opened **ACME-API-1F3** and paged on-call."),
    message("sev2", "m2", "maya", 60 * 26 - 1, "Is it the deploy from this morning?"),
    message(
      "sev2",
      "m3",
      "agent",
      14,
      "Likely. The spike starts 4 minutes after `api@a91c2e0` rolled out. The new `pricing_rules` query does a sequential scan on `orders`.",
    ),
    message(
      "sev2",
      "m4",
      "agent",
      13,
      "- Linear: created **PLAT-1288**\n- Slack: posted in #incidents",
    ),
    message("sev2", "m5", "maya", 10, "Roll it back and keep the ticket open."),
    message("sev2", "m6", "maya", 10, "And attach the query plan, please."),
    message(
      "sev2",
      "m7",
      "agent",
      8,
      "Rolled back to `api@6f0d1b4`. p95 is back to 430 ms:\n\n```sql\nEXPLAIN SELECT * FROM orders WHERE rule_id = $1;\n```",
      { inReplyToMessageId: "sev2-m5" },
    ),
  ],
  standup: [
    message("standup", "m1", "Jonas", 40, "Billing webhook tests are green again."),
    message("standup", "m2", "agent", 39, "Noted. I'll drop the flaky tag from `billing_webhook`."),
    message("standup", "m3", "maya", 38, "Thanks both."),
  ],
  launch: [
    message("launch", "m1", "maya", 5, "Draft the launch post for the new chat UI."),
    message("launch", "m2", "agent", 4, "On it. Outlining sections first."),
  ],
};
let queue: QueuedInput[] = [
  create(QueuedInputSchema, { id: "q1", text: "Keep it under 400 words." }),
  create(QueuedInputSchema, { id: "q2", text: "Mention the dark theme and phone layout." }),
];
const unread = new Set(["standup"]);
const busy = new Set(["launch"]);

const session = (t: Thread) =>
  create(SessionSchema, {
    thread: t,
    unread: unread.has(t.id),
    preview: messages[t.id]?.at(-1)?.text.split("\n")[0] ?? "",
    run: busy.has(t.id)
      ? { id: `${t.id}-run`, threadId: t.id, status: "running", invocationStatus: "running" }
      : undefined,
  });
const find = (id: string) => threads.find((t) => t.id === id)!;

function router(agentId: string) {
  return createConnectRouter().service(ChatService, {
    getIdentity: () => ({ user }),
    listAgents: () => ({ agents }),
    listSessions: ({ query }) => ({
      sessions: threads
        .filter((t) => t.primaryAgentId === agentId)
        .filter((t) => !query || t.title.toLowerCase().includes(query.toLowerCase()))
        .map(session),
    }),
    getSession: ({ threadId, includeHistory }) => ({
      session: session(find(threadId)),
      sequence: 1n,
      messages: includeHistory ? messages[threadId] : [],
      recentActivity: busy.has(threadId)
        ? [
            create(ActivitySchema, {
              kind: "tool.call",
              detail: {
                case: "toolCall",
                value: {
                  id: "tool-1",
                  name: "wiki.read",
                  summary: "Read the release notes",
                  status: "completed",
                  inputJson: '{"page":"release-notes"}',
                  outputJson: '{"sections":4}',
                  display: ToolDisplay.FULL,
                },
              },
            }),
            create(ActivitySchema, {
              kind: "tool.call",
              detail: {
                case: "toolCall",
                value: {
                  id: "tool-2",
                  name: "draft.write",
                  summary: "Writing the outline",
                  status: "running",
                },
              },
            }),
          ]
        : [],
    }),
    listQueuedMessages: ({ threadId }) => ({ messages: busy.has(threadId) ? queue : [] }),
    removeQueuedMessage: ({ id }) => {
      queue = queue.filter((q) => q.id !== id);
      return {};
    },
    searchMessages: ({ threadId, query }) => ({
      messages: (messages[threadId] ?? []).filter((m) =>
        m.text.toLowerCase().includes(query.toLowerCase()),
      ),
    }),
    setReadState: ({ threadId, unread: value }) => {
      if (value) unread.add(threadId);
      else unread.delete(threadId);
      return {};
    },
    renameSession: ({ threadId, title }) => {
      find(threadId).title = title;
      return {};
    },
    setTyping: () => ({}),
    postMessage: ({ threadId, text, inReplyToMessageId }) => {
      const posted = message(threadId, `m${Date.now()}`, "maya", 0, text, { inReplyToMessageId });
      messages[threadId] = [...(messages[threadId] ?? []), posted];
      return { message: posted };
    },
    async *watchThread(_request, context) {
      // Nothing streams in the preview; hold the subscription open until the chat unmounts.
      await new Promise((resolve) => context.signal.addEventListener("abort", resolve));
      yield* [];
    },
  });
}

const handlers = new Map<string, ReturnType<typeof createFetchHandler>>();
export function installMockProxy() {
  const real = window.fetch.bind(window);
  window.fetch = async (input, init) => {
    const request = new Request(input, init);
    const url = new URL(request.url);
    if (!url.pathname.startsWith(`${PROXY}/`)) return real(input, init);
    if (url.pathname === `${PROXY}/agents`) return Response.json({ agents });
    const [, agentId, ...rest] = url.pathname.slice(PROXY.length).split("/");
    const path = `/${rest.join("/")}`;
    const key = `${agentId}${path}`;
    if (!handlers.has(key)) {
      const handler = router(agentId).handlers.find((h) => h.requestPath === path);
      if (!handler) return new Response("unknown method", { status: 404 });
      handlers.set(key, createFetchHandler(handler));
    }
    return handlers.get(key)!(
      new Request(new URL(path, url.origin), {
        method: request.method,
        headers: request.headers,
        body: request.method === "GET" ? undefined : await request.arrayBuffer(),
        signal: request.signal,
      }),
    );
  };
}
