import type { JsonValue } from "@bufbuild/protobuf";
import type { AgentContext } from "./index.js";
import type { Goal, Message, Task } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

export type ConversationMessage = {
  type: "message";
  id: string;
  role: "user" | "assistant";
  message: Message;
  cachedAgentRepresentation?: JsonValue;
};
export type ObjectiveMessage = { type: "objective"; id: string; objective: string };
export type GoalMessage = { type: "goal"; id: string; goal: Goal };
export type TaskMessage = { type: "task"; id: string; task: Task };
export type ContextMessage = ConversationMessage | ObjectiveMessage | GoalMessage | TaskMessage;
export type MessageHistory = { items: ContextMessage[]; nextPageToken: string };
export type MessageHistoryOptions = {
  limit?: number;
  beforeMessageId?: string;
  /** Include the current run objective on the latest page (default true). */
  includeObjective?: boolean;
  /** Include current goal/task state; requires the invocation's work.read grant. */
  includeWork?: boolean;
};
export interface MessageClient {
  history(options?: MessageHistoryOptions): Promise<MessageHistory>;
}

type Context = Pick<
  AgentContext,
  | "agentId"
  | "runId"
  | "objective"
  | "participants"
  | "messages"
  | "getMessages"
  | "goals"
  | "tasks"
>;

/** Invocation-bound facade: callers cannot select another thread or acting agent. */
export function createMessageClient(ctx: Context): MessageClient {
  const own = new Set(ctx.participants.filter((p) => p.agentId === ctx.agentId).map((p) => p.id));
  let latestReceived = ctx.messages
    .filter((message) => message.status === "complete" && !own.has(message.participantId))
    .at(-1);
  return {
    async history(options = {}) {
      const [page, goals, tasks] = await Promise.all([
        ctx.getMessages({ limit: options.limit, beforeMessageId: options.beforeMessageId }),
        options.includeWork ? ctx.goals.list() : [],
        options.includeWork ? ctx.tasks.list() : [],
      ]);
      const cached = new Map(
        page.cachedMessages.map((entry) => [entry.messageId, entry.messageJson]),
      );
      const items: ContextMessage[] = page.messages.map((message) => {
        let cachedAgentRepresentation: JsonValue | undefined;
        const stored = cached.get(message.id);
        if (stored) {
          try {
            cachedAgentRepresentation = JSON.parse(stored) as JsonValue;
          } catch {
            /* Invalid cache never hides canonical history. */
          }
        }
        return {
          type: "message",
          id: message.id,
          role: own.has(message.participantId) ? "assistant" : "user",
          message,
          cachedAgentRepresentation,
        };
      });
      if (!options.beforeMessageId) {
        const received = page.messages
          .filter((message) => message.status === "complete" && !own.has(message.participantId))
          .at(-1);
        // Small pages retain the latest received text when deciding whether to include the objective.
        if (received) latestReceived = received;
        if (
          options.includeObjective !== false &&
          ctx.objective &&
          ctx.objective !== latestReceived?.text
        )
          items.push({ type: "objective", id: `objective:${ctx.runId}`, objective: ctx.objective });
      }
      items.push(
        ...goals.map((goal): GoalMessage => ({ type: "goal", id: `goal:${goal.id}`, goal })),
      );
      items.push(
        ...tasks.map((task): TaskMessage => ({ type: "task", id: `task:${task.id}`, task })),
      );
      return { items, nextPageToken: page.nextPageToken };
    },
  };
}
