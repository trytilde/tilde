import { authInterceptor } from "@/auth";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { AgentService } from "./gen/tilde/management/v1/agents_pb.js";
export const agents = createClient(
  AgentService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
