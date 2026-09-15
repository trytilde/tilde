import type { Tool } from "./index.js";

export type ChannelExecution = { toolCallId?: string };
/** Callable programmatically; its provider-authored descriptor can also be given to a model. */
export type ChannelTool<Input = unknown> = Omit<Tool, "execute" | "stream"> & {
  (input: Input, execution?: ChannelExecution): Promise<unknown>;
  execute(input: Input, execution?: ChannelExecution): Promise<unknown>;
  stream?(
    input: Input,
    chunks: AsyncIterable<unknown>,
    execution?: ChannelExecution,
  ): Promise<unknown>;
  readonly toolName: string;
};
export type ChannelTools = Record<string, ChannelTool>;
export type NativeChannelTools = {
  sendMessage?: ChannelTool<{
    text: string;
    addressedParticipantIds?: string[];
    inReplyToMessageId?: string;
    attachmentIds?: string[];
  }>;
};
export type SlackChannelTools = {
  sendMessage?: ChannelTool<{ channelId: string; text: string; threadTs?: string }>;
  reactToMessage?: ChannelTool<{ channelId: string; messageTs: string; emoji: string }>;
  removeReaction?: ChannelTool<{ channelId: string; messageTs: string; emoji: string }>;
};
export type GitHubChannelTools = {
  sendMessage?: ChannelTool<{ owner: string; repository: string; number: number; text: string }>;
  replyToReview?: ChannelTool<{
    owner: string;
    repository: string;
    number: number;
    commentId: number;
    text: string;
  }>;
  reactToMessage?: ChannelTool<{
    owner: string;
    repository: string;
    commentId: number;
    reaction: "+1" | "-1" | "laugh" | "confused" | "heart" | "hooray" | "rocket" | "eyes";
  }>;
};
export type AgentMailChannelTools = {
  sendMessage?: ChannelTool<{
    to: string[];
    subject: string;
    text: string;
    html?: string;
    cc?: string[];
    bcc?: string[];
  }>;
  replyToMessage?: ChannelTool<{
    messageId: string;
    text: string;
    replyAll?: boolean;
    html?: string;
  }>;
  getThread?: ChannelTool<{ threadId: string }>;
};
export type LinqChannelTools = {
  sendMessage?: ChannelTool<{ chatId: string; text: string }>;
  reactToMessage?: ChannelTool<{
    messageId: string;
    operation: "add" | "remove";
    reaction: "love" | "like" | "dislike" | "laugh" | "emphasize" | "question";
  }>;
  getMessages?: ChannelTool<{ chatId: string }>;
};
export type TelnyxWhatsappChannelTools = {
  sendMessage?: ChannelTool<{ to: string; text: string; replyToMessageId?: string }>;
  sendTemplate?: ChannelTool<{
    to: string;
    templateName: string;
    language: string;
    components?: unknown[];
  }>;
  sendMedia?: ChannelTool<{
    to: string;
    mediaType: "image" | "video" | "audio" | "document" | "sticker";
    mediaId?: string;
    link?: string;
    caption?: string;
    filename?: string;
    replyToMessageId?: string;
  }>;
  reactToMessage?: ChannelTool<{ to: string; messageId: string; emoji: string }>;
};
export type WhatsappChannelTools = TelnyxWhatsappChannelTools & {
  markRead?: ChannelTool<{ messageId: string; typing?: boolean }>;
};
export interface Channels {
  readonly current: ChannelTools;
  readonly native: NativeChannelTools | undefined;
  readonly slack: SlackChannelTools | undefined;
  readonly github: GitHubChannelTools | undefined;
  readonly agentmail: AgentMailChannelTools | undefined;
  readonly linq: LinqChannelTools | undefined;
  readonly whatsapp: WhatsappChannelTools | undefined;
  readonly telnyxWhatsapp: TelnyxWhatsappChannelTools | undefined;
  connections(): { connectionId: string; providerId: string }[];
  forConnection(connectionId: string): ChannelTools;
  provider(providerId: "native", connectionId?: string): NativeChannelTools | undefined;
  provider(providerId: "slack", connectionId?: string): SlackChannelTools | undefined;
  provider(providerId: "github", connectionId?: string): GitHubChannelTools | undefined;
  provider(providerId: "agentmail", connectionId?: string): AgentMailChannelTools | undefined;
  provider(providerId: "linq", connectionId?: string): LinqChannelTools | undefined;
  provider(providerId: "whatsapp", connectionId?: string): WhatsappChannelTools | undefined;
  provider(providerId: "telnyx", connectionId?: string): TelnyxWhatsappChannelTools | undefined;
  provider(providerId: string, connectionId?: string): ChannelTools | undefined;
  /** Use the full server-published tool name for custom provider tools. */
  call_channel_tool(
    name: string,
    serializedArgs: string,
    execution?: ChannelExecution,
  ): Promise<unknown>;
}
