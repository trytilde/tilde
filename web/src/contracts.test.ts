import { expect, it } from "vitest";
import type { DescMessage } from "@bufbuild/protobuf";
import { AgentService } from "./gen/tilde/management/v1/agents_pb.js";
import { ChatService } from "./gen/tilde/management/v1/chat_pb.js";
import { ConnectionsService } from "./gen/tilde/management/v1/connections_pb.js";
import { ChatService as RuntimeChatService } from "./gen/tilde/runtime/v1/chat_pb.js";
import { MessageSchema } from "./gen/tilde/types/v1/chat_pb.js";

it("management contracts cannot reach runtime cache types through shared entities", () => {
  const seen = new Set<DescMessage>();
  function visit(message: DescMessage) {
    if (seen.has(message)) return;
    seen.add(message);
    expect(message.typeName.startsWith("tilde.runtime.")).toBe(false);
    for (const field of message.fields) {
      expect(field.localName).not.toBe("cachedAgentRepresentationJson");
      if (
        field.fieldKind === "message" ||
        (field.fieldKind === "map" && field.mapKind === "message")
      )
        visit(field.message);
    }
  }
  for (const service of [AgentService, ChatService, ConnectionsService])
    for (const method of service.methods) {
      visit(method.input);
      visit(method.output);
    }
  expect(seen.has(MessageSchema)).toBe(true);
});
it("runtime history includes canonical messages and a separate conversion collection with implicit thread scope", () => {
  const history = RuntimeChatService.methods.find((m) => m.name === "ListMessages")!;
  expect(history.input.fields.some((f) => f.name === "thread_id")).toBe(false);
  expect(history.output.fields.map((f) => f.localName)).toEqual([
    "messages",
    "nextPageToken",
    "cachedMessages",
  ]);
  const get = RuntimeChatService.methods.find((m) => m.name === "GetThread")!;
  expect(get.input.fields).toHaveLength(0);
  expect(ChatService.methods.some((m) => m.name === "CacheConvertedMessages")).toBe(false);
  expect(RuntimeChatService.methods.some((m) => m.name === "CacheConvertedMessages")).toBe(true);
});
