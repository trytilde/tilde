import { expect, it } from "vitest";
import { create } from "@bufbuild/protobuf";
import { ObservationSchema } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { sessionTranscript } from "./session-transcript";
const row = (id: string, at: number, values: Record<string, unknown> = {}) =>
  create(ObservationSchema, {
    id,
    traceId: "trace",
    type: "GENERATION",
    name: id,
    startTime: `2026-09-16T10:00:${String(at).padStart(2, "0")}Z`,
    ...values,
  });
const prompt = { role: "user", content: "What is the weather?" };
const call = {
  type: "tool-call",
  toolCallId: "call-1",
  toolName: "weather",
  input: { city: "Berlin" },
};
const result = {
  type: "tool-result",
  toolCallId: "call-1",
  toolName: "weather",
  output: { temperature: 20 },
};

it("joins model history, reasoning, tool spans and results without repeating nested generation output", () => {
  const observations = [
    row("wrapper", 0, { input: JSON.stringify([prompt]), output: "It is 20 degrees." }),
    row("first", 1, {
      parentId: "wrapper",
      input: JSON.stringify([prompt]),
      output: JSON.stringify({
        role: "assistant",
        content: [{ type: "reasoning", text: "I will check the weather." }, call],
      }),
    }),
    row("weather", 2, {
      type: "TOOL",
      parentId: "first",
      input: JSON.stringify(call.input),
      output: JSON.stringify(result.output),
      attributes: [{ key: "attributes.ai.toolCall.id", value: "call-1" }],
    }),
    row("second", 3, {
      parentId: "wrapper",
      input: JSON.stringify([
        prompt,
        { role: "assistant", content: [call] },
        { role: "tool", content: [result] },
      ]),
      output: "It is 20 degrees.",
    }),
  ];
  const entries = sessionTranscript(observations);
  expect(entries.map((e) => e.kind)).toEqual(["message", "reasoning", "tool", "message"]);
  expect(entries.filter((e) => e.kind === "message").map((e) => e.text)).toEqual([
    prompt.content,
    "It is 20 degrees.",
  ]);
  expect(entries[2]).toMatchObject({
    kind: "tool",
    name: "weather",
    sourceId: "trace:weather",
    input: JSON.stringify(call.input, null, 2),
    output: JSON.stringify(result.output, null, 2),
  });
});

it("keeps identical user messages on separate turns and extends cumulative history", () => {
  const entries = sessionTranscript([
    row("one", 1, {
      traceId: "a",
      input: JSON.stringify([{ role: "user", content: "Again" }]),
      output: "First answer",
    }),
    row("two", 2, {
      traceId: "b",
      input: JSON.stringify([
        { role: "user", content: "Again" },
        { role: "assistant", content: "First answer" },
        { role: "user", content: "Again" },
      ]),
      output: "Second answer",
    }),
    row("three", 3, { traceId: "c", input: "Again", output: "Third answer" }),
  ]);
  expect(entries.filter((e) => e.kind === "message").map((e) => e.text)).toEqual([
    "Again",
    "First answer",
    "Again",
    "Second answer",
    "Again",
    "Third answer",
  ]);
});

it("recognizes OpenAI and Anthropic tool and reasoning payloads and retains failures", () => {
  const entries = sessionTranscript([
    row("openai", 1, {
      output: JSON.stringify({
        choices: [
          {
            message: {
              role: "assistant",
              reasoning_content: "Check the file.",
              tool_calls: [
                {
                  id: "read",
                  type: "function",
                  function: { name: "read_file", arguments: '{"path":"a.ts"}' },
                },
              ],
            },
          },
        ],
      }),
    }),
    row("result", 2, {
      type: "TOOL",
      name: "read_file",
      level: "ERROR",
      statusMessage: "File not found",
      input: '{"path":"a.ts"}',
      attributes: [{ key: "attributes.ai.toolCall.id", value: "read" }],
    }),
    row("anthropic", 3, {
      traceId: "anthropic",
      input: JSON.stringify([
        {
          role: "user",
          content: [
            { type: "image", source: { url: "https://example.com/private" } },
            { type: "text", text: "Explain" },
          ],
        },
      ]),
      output: JSON.stringify({
        role: "assistant",
        content: [
          { type: "thinking", thinking: "Inspect the captured image." },
          { type: "text", text: "Done" },
        ],
      }),
    }),
  ]);
  expect(entries.find((e) => e.kind === "tool")).toMatchObject({
    name: "read_file",
    error: "File not found",
    input: '{\n  "path": "a.ts"\n}',
  });
  expect(entries.filter((e) => e.kind === "reasoning").map((e) => e.text)).toEqual([
    "Check the file.",
    "Inspect the captured image.",
  ]);
  expect(JSON.stringify(entries)).not.toContain("https://example.com/private");
});

it("shows missing captures without inventing messages and bounds malformed payloads", () => {
  const entries = sessionTranscript([
    row("uncaptured", 1),
    row("plain", 2, { output: "{not JSON" }),
  ]);
  expect(entries[0]).toMatchObject({
    kind: "notice",
    text: "uncaptured: message content was not recorded.",
  });
  expect(entries[1]).toMatchObject({ kind: "message", role: "assistant", text: "{not JSON" });
});

it("preserves reasoning captured only on a parent generation and ignores empty transport spans", () => {
  const entries = sessionTranscript([
    row("transport", 0, { type: "SPAN", traceId: "transport" }),
    row("wrapper", 1, {
      attributes: [{ key: "attributes.ai.response.reasoning", value: '"Consider the request."' }],
    }),
    row("call", 2, { parentId: "wrapper", input: "Hello", output: "Hi" }),
  ]);
  expect(entries.map((e) => e.kind)).toEqual(["message", "reasoning", "message"]);
  expect(entries[1]).toMatchObject({ text: "Consider the request.", sourceId: "trace:wrapper" });
});
