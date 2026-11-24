import { create } from "@bufbuild/protobuf";
import { createConnectRouter } from "@connectrpc/connect";
import { createFetchHandler } from "@connectrpc/connect/protocol";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Chat, type ChatFetch, ListSessions } from "../../sdk/ts/packages/chat-ui/src/index";
import { ChatService, SessionSchema } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import {
  MessageSchema,
  ThreadSchema,
  UserSchema,
} from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

// `Chat.fetch` replaces the cookie-carrying fetch for every request the chat makes: agent
// discovery and each ChatService call, streams included. Here it serves a real Connect router
// (the way a Tilde space's Storybook mocks its chat) and requires a bearer token, as Tilde
// Cloud's space chat proxy does.
const thread = (member: boolean) =>
  create(ThreadSchema, {
    id: "thread",
    title: "Fix checkout",
    primaryAgentId: "agent",
    participants: [
      ...(member ? [{ id: "me", userId: "viewer", name: "You", active: true }] : []),
      { id: "bot", agentId: "agent", name: "Bug Fixer", active: true },
    ],
  });
const posted: { text: string; participantId: string }[] = [];
/** The proxy: `member` whether the viewer is in the thread yet, `agents` its discovery answer. */
function proxy({
  member = true,
  agents = () => Response.json({ agents: [{ id: "agent", name: "Bug Fixer" }] }),
}: { member?: boolean; agents?: () => Response } = {}): { send: ChatFetch; seen: string[] } {
  const router = createConnectRouter().service(ChatService, {
    getIdentity: () => ({ user: create(UserSchema, { id: "viewer", name: "You" }) }),
    listSessions: () => ({ sessions: [create(SessionSchema, { thread: thread(member) })] }),
    getSession: () => ({
      session: create(SessionSchema, { thread: thread(member) }),
      cursor: "c0",
      sequence: 1n,
      messages: [
        create(MessageSchema, {
          id: "m1",
          threadId: "thread",
          participantId: "bot",
          text: "Opened a pull request with the fix",
          status: "complete",
          createdAt: { seconds: 1n },
        }),
      ],
    }),
    listQueuedMessages: () => ({ messages: [] }),
    setReadState: () => ({}),
    setTyping: () => ({}),
    postMessage: (r) => {
      posted.push({ text: r.text, participantId: r.participantId });
      return {
        message: create(MessageSchema, {
          id: r.id,
          threadId: r.threadId,
          participantId: "me",
          text: r.text,
          status: "complete",
          createdAt: { seconds: 2n },
        }),
      };
    },
    // Live updates: none until the chat closes the stream.
    watchThread: (_r, { signal }) => ({
      [Symbol.asyncIterator]: () => ({
        next: () =>
          new Promise<IteratorResult<never>>((resolve) =>
            signal.addEventListener("abort", () => resolve({ done: true, value: undefined }), {
              once: true,
            }),
          ),
      }),
    }),
  });
  const handlers = router.handlers.map((h) => ({
    path: h.requestPath,
    serve: createFetchHandler(h),
  }));
  const seen: string[] = [];
  const send: ChatFetch = async (input, init) => {
    const request = new Request(input, init);
    seen.push(request.headers.get("authorization") ?? "");
    if (request.headers.get("authorization") !== "Bearer space-token")
      return new Response(null, { status: 401 });
    const path = new URL(request.url).pathname;
    if (path.endsWith("/agents")) return agents();
    const handler = handlers.find((h) => path.endsWith(h.path));
    return handler ? handler.serve(request) : new Response(null, { status: 404 });
  };
  return { send, seen };
}
beforeEach(() => {
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  HTMLElement.prototype.scrollTo = vi.fn();
  posted.length = 0;
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
/** What a space passes: its token on each request. */
const withToken =
  (server: ChatFetch): ChatFetch =>
  (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set("authorization", "Bearer space-token");
    return server(input, { ...init, headers });
  };
const space = "https://tilde.example/api/v1/space-chat/env/threads/thread";
it("sends every request through the given fetch, never the cookie-carrying default", async () => {
  const global = vi.spyOn(globalThis, "fetch");
  const { send: server, seen } = proxy();
  const send = withToken(server);
  render(
    <Chat
      sessionId="thread"
      identity="viewer"
      proxy="https://tilde.example/api/v1/space-chat/env/threads/thread"
      fetch={send}
    />,
  );
  await screen.findByText("Opened a pull request with the fix");
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Why this file?" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() => expect(posted).toEqual([{ text: "Why this file?", participantId: "me" }]));
  expect(seen.length).toBeGreaterThan(3);
  expect(seen.every((authorization) => authorization === "Bearer space-token")).toBe(true);
  expect(global).not.toHaveBeenCalled();
});
it("posts naming no participant when the viewer reads a thread they are not in yet", async () => {
  const send = withToken(proxy({ member: false }).send);
  render(<Chat sessionId="thread" identity="viewer" proxy={space} fetch={send} />);
  await screen.findByText("Opened a pull request with the fix");
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Why this file?" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() => expect(posted).toEqual([{ text: "Why this file?", participantId: "" }]));
});
it("says why the proxy refused, in its own words", async () => {
  const refused = () =>
    Response.json(
      { code: "permission_denied", message: "This space may not chat with that agent here" },
      { status: 403 },
    );
  const send = withToken(proxy({ agents: refused }).send);
  render(<Chat sessionId="thread" identity="viewer" proxy={space} fetch={send} />);
  expect(
    (await screen.findAllByText("This space may not chat with that agent here")).length,
  ).toBeGreaterThan(0);
});
it("lists sessions through the given fetch too", async () => {
  const global = vi.spyOn(globalThis, "fetch");
  const send = withToken(proxy().send);
  render(<ListSessions identity="viewer" proxy={space} fetch={send} onSelectSession={() => {}} />);
  await screen.findByText("Fix checkout");
  expect(global).not.toHaveBeenCalled();
});
