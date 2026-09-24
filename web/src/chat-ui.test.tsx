import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Chat, ListSessions } from "../../sdk/ts/packages/chat-ui/src/index";
import {
  ActivitySchema,
  MessageSchema,
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
it("searches the provider and coordinates sidebar rename and per-user unread changes", async () => {
  const select = vi.fn();
  render(<ListSessions identity="alice" proxy="/api/chat" onSelectSession={select} />);
  fireEvent.click(await screen.findByRole("button", { name: "Conversation" }));
  expect(select).toHaveBeenCalledWith("session", "agent");
  fireEvent.click(screen.getByLabelText("Actions for Conversation"));
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  fireEvent.change(screen.getByLabelText("Conversation name"), {
    target: { value: "Updated title" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(api.renameSession).toHaveBeenCalledWith({ threadId: "session", title: "Updated title" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Mark unread" }));
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
