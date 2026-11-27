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
// The agent thinks, searches, tells the person what it found with its sendMessage tool, the
// person answers, and the agent thinks again and carries on with another call. Each message lands
// between the calls around it, only calls with no message between them share a block, and the
// turn's reasoning splits where a message or call came between.
const at = (seconds: number) => ({
  seconds: BigInt(Math.floor(seconds)),
  nanos: Math.round((seconds % 1) * 1e9),
});
const tool = (id: string, name: string, status: string) =>
  create(ToolCallSchema, { id, name, status, providerId: "native" });
const agentMessage = create(MessageSchema, {
  id: "found",
  text: "I found two flights",
  threadId: "session",
  participantId: "bot",
  status: "complete",
  createdAt: at(102),
});
const reply = create(MessageSchema, {
  id: "reply",
  text: "Book the morning one",
  threadId: "session",
  participantId: "human",
  status: "complete",
  createdAt: at(104),
});
const toolEvent = (sequence: bigint, seconds: number, call: ReturnType<typeof tool>) =>
  create(ActivitySchema, {
    sequence,
    kind: call.status === "running" ? "tool.started" : "tool.completed",
    createdAt: at(seconds),
    detail: { case: "toolCall", value: call },
  });
const messageEvent = (sequence: bigint, seconds: number, value: typeof reply) =>
  create(ActivitySchema, {
    sequence,
    kind: "message.completed",
    createdAt: at(seconds),
    entityId: value.id,
    detail: { case: "message", value },
  });
const thought = (sequence: bigint, seconds: number, text: string) =>
  create(ActivitySchema, {
    sequence,
    kind: "reasoning.delta",
    createdAt: at(seconds),
    entityId: "invocation",
    textDelta: text,
  });
const beforeReply = [
  thought(9n, 99, "Looking for flights"),
  toolEvent(10n, 100, tool("search", "searchFlights", "running")),
  toolEvent(11n, 101, tool("search", "searchFlights", "completed")),
  // Native sendMessage reports its start, the message and its end at one instant.
  toolEvent(12n, 102, tool("send", "sendMessage", "running")),
  messageEvent(13n, 102, agentMessage),
  toolEvent(14n, 102, tool("send", "sendMessage", "completed")),
];
const afterReply = [
  messageEvent(15n, 104, reply),
  thought(16n, 105, "Booking the morning flight"),
  // Stamped with its transaction's start, before the reasoning it committed after.
  toolEvent(17n, 104.5, tool("book", "bookFlight", "running")),
];
// The transcript top to bottom: a message's id, or a block's kind and contents.
const transcript = (container: HTMLElement) =>
  Array.from(
    container.querySelectorAll<HTMLElement>("[data-message-id], .chat-event[role='group']"),
  ).map((node) => {
    if (node.dataset.messageId) return node.dataset.messageId;
    if (node.getAttribute("aria-label") === "Reasoning")
      return `reasoning: ${(node.textContent ?? "").replace(/^(Thinking|Reasoning)/, "").trim()}`;
    const calls = Array.from(node.querySelectorAll("button:not([aria-expanded='true'])"))
      .map((row) => (row.textContent ?? "").trim())
      .filter(Boolean);
    return `tools: ${calls.join(", ")}`;
  });
const expected = [
  "old",
  "reasoning: Looking for flights",
  "tools: searchFlights, sendMessage",
  "found",
  "reply",
  "reasoning: Booking the morning flight",
  "tools: bookFlight",
];
it("interleaves messages, tool calls and reasoning in event order after a reload", async () => {
  api.getSession.mockResolvedValue({
    session,
    cursor: "resume",
    sequence: 17n,
    messages: [message("old", "Earlier answer"), agentMessage, reply],
    nextMessageToken: "",
    recentActivity: [...beforeReply, ...afterReply],
  });
  const view = render(<Chat sessionId="session" identity="alice" proxy="/api/chat" />);
  await screen.findByText("bookFlight");
  expect(transcript(view.container)).toEqual(expected);
});
it("after a reload puts history from before the replayed activity first, whatever its clock", async () => {
  // The person's message was created just before the replayed window; the runtime that reported
  // the next call runs three seconds behind the database.
  const before = create(MessageSchema, {
    id: "before",
    text: "Find me a flight",
    threadId: "session",
    participantId: "human",
    status: "complete",
    createdAt: at(100),
  });
  api.getSession.mockResolvedValue({
    session,
    cursor: "resume",
    sequence: 52n,
    messages: [message("old", "Earlier answer"), before],
    nextMessageToken: "",
    recentActivity: [
      toolEvent(51n, 97.5, tool("search", "searchFlights", "running")),
      thought(52n, 97.6, "Comparing fares"),
    ],
  });
  const view = render(<Chat sessionId="session" identity="alice" proxy="/api/chat" />);
  await screen.findByText("searchFlights");
  expect(transcript(view.container)).toEqual([
    "old",
    "before",
    "tools: searchFlights",
    "reasoning: Comparing fares",
  ]);
});
it("interleaves live messages, including the person's reply, and keeps an edited message in place", async () => {
  const view = render(<Chat sessionId="session" identity="alice" proxy="/api/chat" />);
  await screen.findByText("Earlier answer");
  await waitFor(() => expect(emit).toBeDefined());
  for (const event of beforeReply) await act(async () => emit?.(event));
  await screen.findByText("I found two flights");
  // The person replies through the composer: the posted message shows before its activity.
  api.postMessage.mockResolvedValueOnce({ message: reply });
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), {
    target: { value: "Book the morning one" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
  await screen.findByText("Book the morning one");
  expect(transcript(view.container)).toEqual(expected.slice(0, 5));
  for (const event of afterReply) await act(async () => emit?.(event));
  await screen.findByText("bookFlight");
  expect(transcript(view.container)).toEqual(expected);
  // An edit to the agent's message and the call's completion arrive mid-thought: the message stays
  // between the calls around it, and neither splits the reasoning around it.
  await act(async () => emit?.(thought(18n, 107, "Checking seats")));
  await act(async () =>
    emit?.(
      create(ActivitySchema, {
        sequence: 19n,
        kind: "message.updated",
        createdAt: at(108),
        entityId: "found",
        detail: { case: "message", value: { ...agentMessage, text: "I found three flights" } },
      }),
    ),
  );
  await act(async () => emit?.(toolEvent(20n, 109, tool("book", "bookFlight", "completed"))));
  await act(async () => emit?.(thought(21n, 110, " in row 3")));
  await screen.findByText("I found three flights");
  expect(transcript(view.container)).toEqual([...expected, "reasoning: Checking seats in row 3"]);
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
