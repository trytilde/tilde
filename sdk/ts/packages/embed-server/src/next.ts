import type { Embed } from "./index.js";
export * from "./index.js";
/** Mount in app/api/tilde/[...path]/route.ts. Web streams and abort signals pass through. */
export function createEmbedRouteHandlers(embed: Embed) {
  const handle = async (request: Request, context: { params: Promise<{ path: string[] }> }) => {
    const { path } = await context.params;
    return embed.handle(request, path);
  };
  return { GET: handle, POST: handle, DELETE: handle };
}
