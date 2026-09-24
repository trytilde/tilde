import type { ChatProxy } from "@trytilde/chat-proxy";
export { createChatProxy, type ChatProxyOptions, type ChatAgentConfig } from "@trytilde/chat-proxy";
/** Mount in app/api/chat/[...path]/route.ts. Web streams and abort signals pass through. */
export function createChatRouteHandlers(proxy: ChatProxy) {
  const handle = async (request: Request, context: { params: Promise<{ path: string[] }> }) => {
    const { path } = await context.params;
    return proxy.handle(request, path);
  };
  return { GET: handle, POST: handle };
}
