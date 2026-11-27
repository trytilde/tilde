import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Chat, ListSessions } from "../../sdk/ts/packages/chat-ui/src/index";
import {
  ActivitySchema,
  MessageSchema,
  ToolCallSchema,
  ThreadSchema,
  UserSchema,
} from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import { SessionSchema } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
const api = vi.hoisted(() => ({
  getIdentity: vi.fn(),
  getSession: vi.fn(),
  listMessages: vi.fn(),
  listSessions: vi.fn(),
  listQueuedMessages: vi.fn(),
  postMessage: vi.fn(),
  uploadAttachment: vi.fn(),
  downloadAttachment: vi.fn(),
  watchThread: vi.fn(),
  setReadState: vi.fn(),
  renameSession: vi.fn(),
  setTyping: vi.fn(),
  searchMessages: vi.fn(),
  removeQueuedMessage: vi.fn(),
  reorderQueuedMessage: vi.fn(),
  steerQueuedMessage: vi.fn(),
  createThread: vi.fn(),
  cancelInvocation: vi.fn(),
}));
vi.mock("../../sdk/ts/packages/chat-ui/src/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../sdk/ts/packages/chat-ui/src/client")>();
  return {
    ...actual,
    chatClient: () => api,
    listAgents: async () => [{ id: "agent", name: "Helper" }],
  };
});
const thread = create(ThreadSchema, {
  id: "session",
  title: "Conversation",
  primaryAgentId: "agent",
  participants: [
    { id: "human", userId: "verified-user", name: "You", active: true },
    { id: "bot", agentId: "agent", name: "Helper", active: true },
  ],
});
const session = create(SessionSchema, { thread });
const message = (id: string, text: string) =>
  create(MessageSchema, {
    id,
    text,
    threadId: "session",
    participantId: "bot",
    status: "complete",
    createdAt: { seconds: 1n },
  });
let emit: ((value: ReturnType<typeof create<typeof ActivitySchema>>) => void) | undefined;
let watchSignal: AbortSignal | undefined;
beforeEach(() => {
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  HTMLElement.prototype.scrollTo = vi.fn();
  api.getIdentity.mockResolvedValue({
    user: create(UserSchema, { id: "verified-user", name: "You" }),
  });
  api.getSession.mockResolvedValue({
    session,
    cursor: "resume",
    sequence: 1n,
    messages: [message("old", "Earlier answer")],
    nextMessageToken: "",
    recentActivity: [],
  });
  api.listMessages.mockResolvedValue({
    messages: [message("old", "Earlier answer")],
    nextPageToken: "",
  });
  api.listSessions.mockResolvedValue({ sessions: [session], nextPageToken: "" });
  api.listQueuedMessages.mockResolvedValue({ messages: [] });
  api.setReadState.mockResolvedValue({});
  api.setTyping.mockResolvedValue({});
  api.renameSession.mockResolvedValue({});
  api.searchMessages.mockResolvedValue({
    messages: [message("found", "An older matching message")],
    nextPageToken: "",
  });
  api.uploadAttachment.mockResolvedValue({});
  api.postMessage.mockImplementation(async (r) => ({
    message: create(MessageSchema, { ...r, status: "complete", createdAt: { seconds: 2n } }),
  }));
  api.watchThread.mockImplementation(async function* (_request, { signal }) {
    watchSignal = signal;
    while (!signal.aborted) {
      const activity = await new Promise<
        ReturnType<typeof create<typeof ActivitySchema>> | undefined
      >((resolve) => {
        emit = resolve;
        signal.addEventListener("abort", () => resolve(undefined), { once: true });
      });
      if (activity) yield { activity, cursor: "next" };
    }
  });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.resetAllMocks();
});
it("loads history, sends as the server-resolved identity, renders live messages and cancels subscriptions", async () => {
  const view = render(
    <Chat sessionId="session" identity="browser-supplied-id" proxy="/api/chat" />,
  );
  await screen.findByText("Earlier answer");
  await waitFor(() => expect(emit).toBeDefined());
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Hello" },
  });
  const file = new File(["attached text"], "notes.txt", { type: "text/plain" });
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new TextEncoder().encode("attached text").buffer,
  });
  fireEvent.change(view.container.querySelector('input[type="file"]')!, {
    target: { files: [file] },
  });
  api.postMessage.mockRejectedValueOnce(new Error("Retry this send"));
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await screen.findByText("Retry this send");
  expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toBe(
    "Hello",
  );
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() => expect(api.postMessage).toHaveBeenCalledTimes(2));
  expect(api.uploadAttachment).toHaveBeenCalledTimes(1);
  expect(api.postMessage.mock.calls[1][0].id).toBe(api.postMessage.mock.calls[0][0].id);
  expect(api.postMessage.mock.calls[1][0].attachmentIds).toEqual([
    api.uploadAttachment.mock.calls[0][0].id,
  ]);
  await waitFor(() => expect(api.postMessage).toHaveBeenCalled());
  expect(api.postMessage.mock.calls[0][0]).toMatchObject({
    participantId: "human",
    text: "Hello",
    threadId: "session",
  });
  await act(async () =>
    emit?.(
      create(ActivitySchema, {
        sequence: 2n,
        kind: "message.completed",
        detail: { case: "message", value: message("live", "Live answer") },
      }),
    ),
  );
  await screen.findByText("Live answer");
  fireEvent.click(screen.getByRole("button", { name: "Find in conversation" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Search messages" }), {
    target: { value: "older" },
  });
  await screen.findByText("An older matching message");
  expect(api.searchMessages.mock.calls[0][0]).toMatchObject({
    threadId: "session",
    query: "older",
  });
  view.unmount();
  expect(watchSignal?.aborted).toBe(true);
});
// The agent searches, tells the person what it found with its sendMessage tool, the person
// answers, and the agent carries on with another call: each message lands between the calls
// around it, and only calls with no message between them share a block.
const at = (seconds: number) => ({ seconds: BigInt(seconds), nanos: 0 });
const tool = (id: string, name: string, status: string) =>
  create(ToolCallSchema, { id, name, status, providerId: "tilde" });
const agentMessage = create(MessageSchema, {
  id: "found",
  text: "I found two flights",
  threadId: "session",
  participantId: "bot",
  status: "complete",
  createdAt: at(102),
});
const turn = (reply: ReturnType<typeof create<typeof MessageSchema>>) => [
  create(ActivitySchema, {
    sequence: 10n,
    kind: "tool.started",
    createdAt: at(100),
    detail: { case: "toolCall", value: tool("search", "searchFlights", "running") },
  }),
  create(ActivitySchema, {
    sequence: 11n,
    kind: "tool.completed",
    createdAt: at(101),
    detail: { case: "toolCall", value: tool("search", "searchFlights", "completed") },
  }),
  create(ActivitySchema, {
    sequence: 12n,
    kind: "tool.started",
    createdAt: at(102),
    detail: { case: "toolCall", value: tool("send", "sendMessage", "running") },
  }),
  create(ActivitySchema, {
    sequence: 13n,
    kind: "message.completed",
    createdAt: at(102),
    detail: { case: "message", value: agentMessage },
  }),
  create(ActivitySchema, {
    sequence: 14n,
    kind: "tool.completed",
    createdAt: at(103),
    detail: { case: "toolCall", value: tool("send", "sendMessage", "completed") },
  }),
  create(ActivitySchema, {
    sequence: 15n,
    kind: "message.completed",
    createdAt: at(104),
    detail: { case: "message", value: reply },
  }),
  create(ActivitySchema, {
    sequence: 16n,
    kind: "tool.started",
    createdAt: at(105),
    detail: { case: "toolCall", value: tool("book", "bookFlight", "running") },
  }),
];
const reply = create(MessageSchema, {
  id: "reply",
  text: "Book the morning one",
  threadId: "session",
  participantId: "human",
  status: "complete",
  createdAt: at(104),
});
// The transcript top to bottom: message text, or a tool block's call names.
const transcript = (container: HTMLElement) =>
  Array.from(
    container.querySelectorAll<HTMLElement>(
      '[data-message-id], [role="group"][aria-label="Tool calls"]',
    ),
  ).map((node) =>
    node.dataset.messageId
      ? (node.textContent ?? "").replace(/\s+/g, " ").includes("I found two flights")
        ? "found"
        : node.dataset.messageId
      : `tools: ${Array.from(node.querySelectorAll("button"))
          .slice(1)
          .map((row) => (row.textContent ?? "").trim())
          .join(", ")}`,
  );
const expected = [
  "old",
  "tools: searchFlights, sendMessage",
  "found",
  "reply",
  "tools: bookFlight",
];
it("interleaves messages with the tool calls around them after a reload", async () => {
  api.getSession.mockResolvedValue({
    session,
    cursor: "resume",
    sequence: 16n,
    messages: [message("old", "Earlier answer"), agentMessage, reply],
    nextMessageToken: "",
    recentActivity: turn(reply),
  });
  const view = render(<Chat sessionId="session" identity="alice" proxy="/api/chat" />);
  await screen.findByText("bookFlight");
  expect(transcript(view.container)).toEqual(expected);
});
it("interleaves live messages, including the person's own reply, with the tool calls around them", async () => {
  const view = render(<Chat sessionId="session" identity="alice" proxy="/api/chat" />);
  await screen.findByText("Earlier answer");
  await waitFor(() => expect(emit).toBeDefined());
  const events = turn(reply);
  for (const event of events.slice(0, 5)) await act(async () => emit?.(event));
  await screen.findByText("I found two flights");
  // The person replies through the composer: the posted message shows before its activity.
  api.postMessage.mockResolvedValueOnce({ message: reply });
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Book the morning one" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await screen.findByText("Book the morning one");
  for (const event of events.slice(5)) await act(async () => emit?.(event));
  await screen.findByText("bookFlight");
  expect(transcript(view.container)).toEqual(expected);
});
it("without its header shows only the transcript and the message bar, and still sends", async () => {
  const view = render(
    <Chat sessionId="session" identity="browser-supplied-id" proxy="/api/chat" header={false} />,
  );
  await screen.findByText("Earlier answer");
  await waitFor(() => expect(emit).toBeDefined());
  expect(view.container.querySelector("header")).toBeNull();
  expect(screen.queryByRole("button", { name: "Find in conversation" })).toBeNull();
  expect(screen.queryByText("Helper")).toBeNull();
  expect(screen.queryByText("Conversation")).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Hello" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() => expect(api.postMessage).toHaveBeenCalledTimes(1));
  expect(api.postMessage.mock.calls[0][0]).toMatchObject({
    participantId: "human",
    text: "Hello",
    threadId: "session",
  });
});
it("searches the provider and coordinates sidebar rename and per-user unread changes", async () => {
  const select = vi.fn();
  render(<ListSessions identity="alice" proxy="/api/chat" onSelectSession={select} />);
  fireEvent.click(await screen.findByRole("button", { name: "Conversation" }));
  expect(select).toHaveBeenCalledWith("session", "agent");
  fireEvent.click(screen.getByLabelText("Actions for Conversation"));
  fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
  fireEvent.change(screen.getByLabelText("Conversation name"), {
    target: { value: "Updated title" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(api.renameSession).toHaveBeenCalledWith({ threadId: "session", title: "Updated title" }),
  );
  fireEvent.click(screen.getByRole("menuitem", { name: "Mark unread" }));
  await waitFor(() =>
    expect(api.setReadState).toHaveBeenCalledWith({
      threadId: "session",
      throughSequence: 0n,
      unread: true,
    }),
  );
  fireEvent.change(screen.getByLabelText("Search conversations and messages"), {
    target: { value: "history" },
  });
  await waitFor(() =>
    expect(api.listSessions.mock.calls.some(([request]) => request.query === "history")).toBe(true),
  );
});
