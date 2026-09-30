import {
  PromptFormat,
  isBundledTools,
  type BundledToolOptions,
  type DiscoveredPrompt,
  type Discovery,
  type DiscoveryContext,
} from "@trytilde/sdk";
import { BaseMessage } from "@langchain/core/messages";
import type { StructuredToolInterface } from "@langchain/core/tools";
import type { ReactAgent } from "langchain";
import { describeTool } from "./tools.js";

type Options = ReactAgent["options"];
// Duck-typed rather than `instanceof`: the project's langchain may be another copy.
const isReactAgent = (value: unknown): value is ReactAgent =>
  !!value &&
  typeof value === "object" &&
  typeof (value as ReactAgent).invoke === "function" &&
  typeof (value as ReactAgent).withConfig === "function" &&
  "graph" in value &&
  !!(value as ReactAgent).options?.model;
// LangChain tools; OpenAI Agents tools also have `invoke` and `name` but carry a `type` instead.
const isTool = (value: unknown): value is StructuredToolInterface =>
  !!value &&
  typeof value === "object" &&
  typeof (value as StructuredToolInterface).name === "string" &&
  typeof (value as StructuredToolInterface).invoke === "function" &&
  "schema" in value;
/** Tools as `withTildeTools` publishes them, each at `<origin>.<name>`. */
const discoverTools = (
  tools: StructuredToolInterface[],
  options: BundledToolOptions,
  origin: string,
) => tools.map((tool) => ({ ...describeTool(tool, options), origin: `${origin}.${tool.name}` }));
type Template = { template?: unknown; templateFormat?: string; _getPromptType?: () => string };
const promptType = (value: unknown) =>
  !!value && typeof value === "object" && typeof (value as Template)._getPromptType === "function"
    ? (value as Template)._getPromptType!()
    : undefined;

/** Prompt names allow `[A-Za-z0-9._/-]`. */
const safe = (name: string) => name.replace(/[^A-Za-z0-9._/-]/g, "-");
/** The export path in an origin such as `dist/index.js#prompts.greeting`. */
const exportName = (origin: string) => origin.slice(origin.indexOf("#") + 1);
/** A message's text: a string, or its text content blocks. */
function messageText(message: unknown): string | undefined {
  const content = (message as { content?: unknown })?.content;
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return undefined;
  return content
    .map((block) => (block?.type === "text" && typeof block.text === "string" ? block.text : ""))
    .join("");
}
/** f-string templates are `{var}` (BRACES), mustache `{{var}}`; jinja2 is not read. */
function templatePrompt(
  name: string,
  template: Template,
  origin: string,
  warnings: string[],
): DiscoveredPrompt | undefined {
  if (typeof template.template !== "string") {
    warnings.push(`LangChain prompt ${name}: only text templates are read (${origin})`);
    return undefined;
  }
  const format =
    template.templateFormat === "mustache"
      ? PromptFormat.MUSTACHE
      : template.templateFormat === "f-string"
        ? PromptFormat.BRACES
        : undefined;
  if (format === undefined) {
    warnings.push(
      `LangChain prompt ${name}: ${template.templateFormat} templates are not read (${origin})`,
    );
    return undefined;
  }
  return { name, format, template: template.template, origin };
}

function discoverAgent(agent: ReactAgent, origin: string): Discovery {
  const options = agent.options as Options;
  const discovery: Required<Discovery> = { prompts: [], skills: [], tools: [], warnings: [] };
  const name = safe(options.name ?? exportName(origin));
  // Other entries (provider tool definitions, ToolNodes) are not run as LangChain tools here.
  discovery.tools = discoverTools(
    (Array.isArray(options.tools) ? options.tools : []).filter(isTool),
    {},
    `${origin}.tools`,
  );
  const system = options.systemPrompt;
  const text = typeof system === "string" ? system : messageText(system);
  if (text)
    discovery.prompts.push({
      name: `${name}/system_prompt`,
      format: PromptFormat.PLAIN,
      template: text,
      origin: `${origin}.systemPrompt`,
    });
  const middleware = options.middleware ?? [];
  // dynamicSystemPromptMiddleware keeps its function in a closure Tilde cannot read.
  if (middleware.some((m) => m.name === "DynamicSystemPromptMiddleware"))
    discovery.warnings.push(
      `LangChain agent ${name}: its dynamic system prompt cannot be read; declare it with definePrompt to version it (${origin}.middleware)`,
    );
  if (!middleware.some((m) => m.name === "tilde"))
    discovery.warnings.push(
      `LangChain agent ${name}: add tildeMiddleware() to its middleware so channel tools, steering and skills reach it (${origin}.middleware)`,
    );
  return discovery;
}

/** Each message of a chat template: templates by their format, fixed messages as plain text. */
function discoverChat(chat: { promptMessages?: unknown[] }, origin: string): Discovery {
  const discovery: Required<Discovery> = { prompts: [], skills: [], tools: [], warnings: [] };
  const name = safe(exportName(origin));
  (chat.promptMessages ?? []).forEach((message, i) => {
    const path = `${origin}.promptMessages[${i}]`;
    const inner = (message as { prompt?: unknown }).prompt;
    if (promptType(inner) === "prompt") {
      const prompt = templatePrompt(`${name}/${i}`, inner as Template, path, discovery.warnings);
      if (prompt) discovery.prompts.push(prompt);
    } else if (BaseMessage.isInstance(message)) {
      const text = messageText(message);
      if (text)
        discovery.prompts.push({
          name: `${name}/${i}`,
          format: PromptFormat.PLAIN,
          template: text,
          origin: path,
        });
    }
    // MessagesPlaceholder and multi-part (image) templates carry no prompt text of their own.
  });
  return discovery;
}

/**
 * `tilde deploy` calls this for each export of the entry module. A `createAgent` agent
 * contributes its system prompt (`<agent name or export>/system_prompt`) and tools; a
 * `PromptTemplate` its template (`<export>`), a `ChatPromptTemplate` each message
 * (`<export>/<index>`), and a `defineTools` value of LangChain tools its tools.
 */
export function discover(value: unknown, context: DiscoveryContext): Discovery | undefined {
  if (isBundledTools(value) && Array.isArray(value.tools) && value.tools.every(isTool))
    return { tools: discoverTools(value.tools, value.options, `${context.origin}.tools`) };
  if (isReactAgent(value)) return discoverAgent(value, context.origin);
  const type = promptType(value);
  if (type === "chat") return discoverChat(value as { promptMessages?: unknown[] }, context.origin);
  if (type !== "prompt") return undefined;
  const warnings: string[] = [];
  const prompt = templatePrompt(
    safe(exportName(context.origin)),
    value as Template,
    context.origin,
    warnings,
  );
  return { prompts: prompt ? [prompt] : [], warnings };
}
