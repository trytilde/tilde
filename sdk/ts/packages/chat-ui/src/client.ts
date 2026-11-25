"use client";
import { Code, ConnectError, createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ChatService } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
export interface ChatAgent {
  id: string;
  name: string;
}
/** How requests reach the proxy. By default with cookies: the proxy may live on another
 * origin (an app's API host) and authenticate by cookie. A replacement must be stable (defined
 * once, or memoised): the components reconnect whenever it changes. */
export type ChatFetch = typeof globalThis.fetch;
const withCookies: ChatFetch = (input, init) => fetch(input, { ...init, credentials: "include" });
export const chatClient = (proxy: string, agentId: string, send: ChatFetch = withCookies) =>
  createClient(
    ChatService,
    createConnectTransport({
      baseUrl: `${proxy.replace(/\/$/, "")}/${encodeURIComponent(agentId)}`,
      useBinaryFormat: true,
      fetch: send,
    }),
  );
export type ChatClient = ReturnType<typeof chatClient>;
export const errorText = (error: unknown) =>
  error instanceof Error ? error.message : "Unable to load chat";
/** The agents this user may chat with through `proxy`: `ChatService/ListAgents` at the proxy
 * itself, which asks every agent it is configured for. */
export async function listAgents(
  proxy: string,
  signal: AbortSignal,
  send: ChatFetch = withCookies,
): Promise<ChatAgent[]> {
  const client = createClient(
    ChatService,
    createConnectTransport({
      baseUrl: proxy.replace(/\/$/, ""),
      useBinaryFormat: true,
      fetch: send,
    }),
  );
  try {
    const { agents } = await client.listAgents({}, { signal });
    return agents.map((agent) => ({ id: agent.id, name: agent.name }));
  } catch (error) {
    if (signal.aborted) throw error;
    // A Connect error the proxy sent says why in its own words: a sign-in, a refused space.
    if (error instanceof ConnectError && error.isWireError && error.rawMessage)
      throw new Error(error.rawMessage);
    const unauthenticated = error instanceof ConnectError && error.code === Code.Unauthenticated;
    throw new Error(unauthenticated ? "Sign in to chat" : "Unable to load agents");
  }
}
// Invalidation only; no user data or credentials live in a global cache.
export function changed(proxy: string, identity: string) {
  window.dispatchEvent(new CustomEvent("tilde-chat-change", { detail: { proxy, identity } }));
}
export function subscribe(proxy: string, identity: string, refresh: () => void) {
  const listener = (event: Event) => {
    const detail = (event as CustomEvent<{ proxy: string; identity: string }>).detail;
    if (detail.proxy === proxy && detail.identity === identity) refresh();
  };
  window.addEventListener("tilde-chat-change", listener);
  return () => window.removeEventListener("tilde-chat-change", listener);
}

export function randomUUID(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
