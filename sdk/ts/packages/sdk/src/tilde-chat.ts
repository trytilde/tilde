import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";
import { ChatService } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import { tracingInterceptor } from "./tracing.js";

export * from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";

export type TildeChatClientOptions = {
  /** Gateway ingress origin. */
  baseUrl: string;
  agentId: string;
  /** Complete service base URL, including the agent prefix when required. */
  baseUrlOverride?: string;
} & (
  | { apiKey: string; identity: string; accessToken?: never }
  /** Privileged operator/internal ingress token; embedding uses apiKey + identity. */
  | { accessToken: string; apiKey?: never; identity?: never }
);

/** Server-side ConnectRPC client for the built-in Tilde chat provider. */
export function createTildeChatClient(options: TildeChatClientOptions) {
  return createClient(
    ChatService,
    createConnectTransport({
      baseUrl:
        options.baseUrlOverride ??
        `${options.baseUrl.replace(/\/$/, "")}/agents/${encodeURIComponent(options.agentId)}`,
      httpVersion: "2",
      interceptors: [
        tracingInterceptor,
        (next) => async (request) => {
          request.header.set("Authorization", `Bearer ${options.apiKey ?? options.accessToken}`);
          if (options.identity !== undefined)
            request.header.set(
              "x-tilde-identity",
              Buffer.from(options.identity).toString("base64url"),
            );
          return next(request);
        },
      ],
    }),
  );
}
