import assert from "node:assert/strict";
import { test } from "node:test";
import { ChatPromptTemplate, MessagesPlaceholder, PromptTemplate } from "@langchain/core/prompts";
import { SystemMessage } from "@langchain/core/messages";
import { createAgent, dynamicSystemPromptMiddleware, fakeModel } from "langchain";
import { tool } from "@langchain/core/tools";
import { PromptFormat, defineTools } from "@trytilde/sdk";
import { discover, tildeMiddleware } from "../dist/index.js";

const at = (origin) => ({ origin, projectDir: "/project" });

test("discover reads createAgent system prompts and prompt templates", async () => {
  const support = createAgent({
    name: "Support Desk",
    model: fakeModel(),
    systemPrompt: new SystemMessage("Help the customer."),
    middleware: [tildeMiddleware()],
  });
  assert.deepEqual(discover(support, at("dist/index.js#support")), {
    prompts: [
      {
        name: "Support-Desk/system_prompt",
        format: PromptFormat.PLAIN,
        template: "Help the customer.",
        origin: "dist/index.js#support.systemPrompt",
      },
    ],
    skills: [],
    tools: [],
    warnings: [],
  });
  // Unnamed: the export names it. A dynamic prompt's function is out of reach, and without
  // tildeMiddleware() the invocation's tools and steering cannot reach the agent.
  const dynamic = createAgent({
    model: fakeModel(),
    middleware: [dynamicSystemPromptMiddleware(() => "Dynamic")],
  });
  assert.deepEqual(discover(dynamic, at("dist/index.js#agents.dynamic")), {
    prompts: [],
    skills: [],
    tools: [],
    warnings: [
      "LangChain agent agents.dynamic: its dynamic system prompt cannot be read; declare it with definePrompt to version it (dist/index.js#agents.dynamic.middleware)",
      "LangChain agent agents.dynamic: add tildeMiddleware() to its middleware so channel tools, steering and skills reach it (dist/index.js#agents.dynamic.middleware)",
    ],
  });

  assert.deepEqual(
    discover(
      PromptTemplate.fromTemplate("Summarise {topic} in {{braces}}"),
      at("dist/index.js#summary"),
    ),
    {
      prompts: [
        {
          name: "summary",
          format: PromptFormat.BRACES,
          template: "Summarise {topic} in {{braces}}",
          origin: "dist/index.js#summary",
        },
      ],
      warnings: [],
    },
  );
  const chat = ChatPromptTemplate.fromMessages(
    [
      ["system", "You answer questions about {{product}}."],
      new MessagesPlaceholder("history"),
      ["human", "{{question}}"],
    ],
    { templateFormat: "mustache" },
  );
  const fixed = ChatPromptTemplate.fromMessages([new SystemMessage("Be brief.")]);
  assert.deepEqual(discover(chat, at("dist/index.js#chat")).prompts, [
    {
      name: "chat/0",
      format: PromptFormat.MUSTACHE,
      template: "You answer questions about {{product}}.",
      origin: "dist/index.js#chat.promptMessages[0]",
    },
    {
      name: "chat/2",
      format: PromptFormat.MUSTACHE,
      template: "{{question}}",
      origin: "dist/index.js#chat.promptMessages[2]",
    },
  ]);
  assert.deepEqual(discover(fixed, at("dist/index.js#fixed")).prompts, [
    {
      name: "fixed/0",
      format: PromptFormat.PLAIN,
      template: "Be brief.",
      origin: "dist/index.js#fixed.promptMessages[0]",
    },
  ]);
  assert.equal(discover({ invoke() {} }, at("dist/index.js#x")), undefined);
});

test("discover declares bundled tools and createAgent tools as withTildeTools publishes them", () => {
  const readOnly = { readOnly: true, destructive: false, idempotent: true, openWorld: false };
  const input = { type: "object", properties: { city: { type: "string" } }, required: ["city"] };
  const weather = tool(async () => "Sunny", {
    name: "weather",
    description: "Weather",
    schema: input,
    metadata: { tilde: { summary: "Checked the weather", annotations: readOnly } },
  });
  assert.deepEqual(
    discover(
      defineTools([weather], { weather: { display: "summary" } }),
      at("dist/index.js#bundledTools"),
    ),
    {
      tools: [
        {
          name: "weather",
          description: "Weather",
          summary: "Checked the weather",
          annotations: readOnly,
          display: "summary",
          inputSchema: input,
          origin: "dist/index.js#bundledTools.tools.weather",
        },
      ],
    },
  );
  const agent = createAgent({
    model: fakeModel(),
    tools: [weather],
    middleware: [tildeMiddleware()],
  });
  assert.deepEqual(
    discover(agent, at("dist/index.js#agent")).tools.map((t) => [t.name, t.summary, t.origin]),
    [["weather", "Checked the weather", "dist/index.js#agent.tools.weather"]],
  );
});
