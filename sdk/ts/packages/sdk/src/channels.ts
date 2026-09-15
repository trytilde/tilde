import { randomUUID } from "node:crypto";
import type { ChannelBinding, Message } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import type { Tool, ToolCatalog } from "./index.js";
import type { Channels, ChannelTool, ChannelTools } from "./channel-types.js";

/** Group only the tools published for this invocation. The API remains the authorization boundary. */
export function createChannels(
  catalog: ToolCatalog,
  binding?: ChannelBinding,
  incoming?: Message,
): Channels {
  const wrapped = new WeakMap<Tool, ChannelTool>();
  const key = (id: string) => id.replaceAll("-", "").toLowerCase();
  function entries() {
    return Object.entries(catalog).flatMap(([name, tool]) => {
      const match = /^channel_([a-f0-9]{32})\.(.+)$/i.exec(name);
      if (match && tool.providerId)
        return [{ name, tool, connectionId: match[1].toLowerCase(), shortName: match[2] }];
      if (tool.providerId === "native")
        return [{ name, tool, connectionId: "native", shortName: name }];
      return [];
    });
  }
  function wrap(name: string, source: Tool): ChannelTool {
    const cached = wrapped.get(source);
    if (cached) return cached;
    const available = () => {
      if (catalog[name] !== source)
        throw new Error("Channel tool is no longer available in this invocation");
    };
    const execute = (input: unknown, execution: { toolCallId?: string } = {}) => {
      available();
      return source.execute(input, { toolCallId: execution.toolCallId ?? randomUUID() });
    };
    const call: ChannelTool = Object.assign(execute, {
      description: source.description,
      inputSchema: source.inputSchema,
      providerId: source.providerId,
      chunkSchema: source.chunkSchema,
      toolName: name,
      execute,
    });
    if (source.stream)
      call.stream = (input, chunks, execution = {}) => {
        available();
        return source.stream!(input, chunks, { toolCallId: execution.toolCallId ?? randomUUID() });
      };
    if (
      binding &&
      name.startsWith(`channel_${key(binding.connectionId)}.`) &&
      incoming?.delivery?.externalMessageId
    )
      call.description += ` Inbound message reference: ${incoming.delivery.externalMessageId}.`;
    wrapped.set(source, call);
    return call;
  }
  function forConnection(id: string): ChannelTools {
    const tools: ChannelTools = Object.create(null);
    for (const entry of entries()) {
      if (entry.connectionId === key(id)) tools[entry.shortName] = wrap(entry.name, entry.tool);
    }
    return tools;
  }
  function provider(id: string, connectionId?: string): ChannelTools | undefined {
    const candidates = entries().filter((entry) => entry.tool.providerId === id);
    const ids = [...new Set(candidates.map((entry) => entry.connectionId))];
    if (connectionId)
      return ids.includes(key(connectionId)) ? forConnection(connectionId) : undefined;
    if (binding && ids.includes(key(binding.connectionId)))
      return forConnection(binding.connectionId);
    if (ids.length > 1)
      throw new Error(`Multiple ${id} connections are available; use channel.forConnection(id)`);
    return ids.length ? forConnection(ids[0]) : undefined;
  }
  return {
    get current() {
      return forConnection(binding?.connectionId ?? "native");
    },
    get native() {
      return provider("native");
    },
    get slack() {
      return provider("slack");
    },
    get github() {
      return provider("github");
    },
    get agentmail() {
      return provider("agentmail");
    },
    get linq() {
      return provider("linq");
    },
    get whatsapp() {
      return provider("whatsapp");
    },
    get telnyxWhatsapp() {
      return provider("telnyx");
    },
    connections() {
      return [
        ...new Map(
          entries().map((entry) => [
            entry.connectionId,
            {
              connectionId:
                entry.connectionId === "native"
                  ? "native"
                  : `${entry.connectionId.slice(0, 8)}-${entry.connectionId.slice(8, 12)}-${entry.connectionId.slice(12, 16)}-${entry.connectionId.slice(16, 20)}-${entry.connectionId.slice(20)}`,
              providerId: entry.tool.providerId!,
            },
          ]),
        ).values(),
      ];
    },
    forConnection,
    provider,
    async call_channel_tool(name, serializedArgs, execution) {
      if (!Object.hasOwn(catalog, name) || !catalog[name].providerId)
        throw new Error("Channel tool is not available in this invocation");
      let input: unknown;
      try {
        input = JSON.parse(serializedArgs);
      } catch {
        throw new Error("Channel tool arguments must be valid JSON");
      }
      return wrap(name, catalog[name])(input, execution);
    },
  };
}
