import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { create } from "@bufbuild/protobuf";
import {
  ActivitySchema,
  MessageSchema,
  MessageChunkSchema,
  ToolCallSchema,
  ParticipantSchema,
  TypingSchema,
} from "@/gen/tilde/types/v1/chat_pb.js";
import { AuditEvent, conversationEvents } from "./conversations";
afterEach(cleanup);
it("replays chunks without duplicating the final message and retains reasoning, membership and tool results", () => {
  const started = create(ActivitySchema, {
    sequence: 1n,
    kind: "message.started",
    entityId: "message",
    detail: {
      case: "message",
      value: create(MessageSchema, { id: "message", participantId: "agent", status: "streaming" }),
    },
  });
  const chunk = create(ActivitySchema, {
    sequence: 2n,
    kind: "message.delta",
    entityId: "message",
    textDelta: "Hello",
    detail: {
      case: "messageChunk",
      value: create(MessageChunkSchema, { messageId: "message", textDelta: "Hello" }),
    },
  });
  expect(conversationEvents([started, chunk])[0].detail.case).toBe("message");
  const intermediate = conversationEvents([started, chunk])[0];
  if (intermediate.detail.case !== "message") throw new Error("message");
  expect(intermediate.detail.value.text).toBe("Hello");
  const completed = create(ActivitySchema, {
    sequence: 3n,
    kind: "message.completed",
    entityId: "message",
    detail: {
      case: "message",
      value: create(MessageSchema, {
        id: "message",
        participantId: "agent",
        text: "Hello",
        status: "complete",
      }),
    },
  });
  const reasoning = create(ActivitySchema, {
    sequence: 4n,
    kind: "reasoning.delta",
    textDelta: "Considering the answer",
  });
  const joined = create(ActivitySchema, {
    sequence: 5n,
    kind: "participant.joined",
    detail: {
      case: "participant",
      value: create(ParticipantSchema, { id: "person", name: "Daniel", active: true }),
    },
  });
  const left = create(ActivitySchema, {
    sequence: 6n,
    kind: "participant.left",
    detail: {
      case: "participant",
      value: create(ParticipantSchema, { id: "person", name: "Daniel", active: false }),
    },
  });
  const typing = create(ActivitySchema, {
    sequence: 7n,
    kind: "typing.changed",
    detail: {
      case: "typing",
      value: create(TypingSchema, { participantId: "person", typing: false }),
    },
  });
  const tool = create(ActivitySchema, {
    sequence: 8n,
    kind: "tool.completed",
    detail: {
      case: "toolCall",
      value: create(ToolCallSchema, {
        id: "tool",
        name: "lookup",
        status: "completed",
        inputJson: '{"query":"example"}',
        outputJson: '{"found":true}',
      }),
    },
  });
  const events = conversationEvents([
    started,
    chunk,
    completed,
    reasoning,
    joined,
    left,
    typing,
    tool,
  ]);
  render(
    <>
      {events.map((e) => (
        <AuditEvent key={String(e.sequence)} event={e} />
      ))}
    </>,
  );
  expect(screen.getAllByText("Hello")).toHaveLength(1);
  expect(screen.getByText("Considering the answer")).toBeTruthy();
  expect(screen.getByText("Daniel joined")).toBeTruthy();
  expect(screen.getByText("Daniel left")).toBeTruthy();
  expect(screen.getByText("person stopped typing")).toBeTruthy();
  expect(screen.getByText('{"found":true}')).toBeTruthy();
});
it("renders email in a sandbox and attachment download metadata without injecting HTML into the app", () => {
  const e = create(ActivitySchema, {
    sequence: 1n,
    kind: "message.completed",
    detail: {
      case: "message",
      value: create(MessageSchema, {
        id: "mail",
        format: "html",
        subject: "Invoice",
        text: "<script>window.parent.stolen=true</script><b>Email</b>",
        attachments: [
          {
            id: "file",
            threadId: "thread",
            filename: "invoice.pdf",
            mediaType: "application/pdf",
            sizeBytes: 20n,
          },
        ],
      }),
    },
  });
  render(<AuditEvent event={e} />);
  expect(screen.getByText("Invoice")).toBeTruthy();
  expect(screen.getByTitle("Email content").getAttribute("sandbox")).toBe("");
  expect(document.querySelector("script")).toBeNull();
  expect(screen.getByRole("button", { name: /invoice.pdf/ })).toBeTruthy();
});
