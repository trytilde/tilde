"use client";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ChatService } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
export interface ChatAgent {
  id: string;
  name: string;
}
export const chatClient = (proxy: string, agentId: string) =>
  createClient(
    ChatService,
    createConnectTransport({
      baseUrl: `${proxy.replace(/\/$/, "")}/${encodeURIComponent(agentId)}`,
      useBinaryFormat: true,
    }),
  );
export type ChatClient = ReturnType<typeof chatClient>;
export const errorText = (error: unknown) =>
  error instanceof Error ? error.message : "Unable to load chat";
export async function listAgents(proxy: string, signal: AbortSignal): Promise<ChatAgent[]> {
  const response = await fetch(`${proxy.replace(/\/$/, "")}/agents`, {
    signal,
    credentials: "same-origin",
    cache: "no-store",
  });
  if (!response.ok)
    throw new Error(response.status === 401 ? "Sign in to chat" : "Unable to load agents");
  return ((await response.json()) as { agents: ChatAgent[] }).agents;
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
