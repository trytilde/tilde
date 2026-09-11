import { ConnectionsService } from "./gen/tilde/management/v1/connections_pb.js";
import { AgentAccessService } from "./gen/tilde/management/v1/access_pb.js";
import { authInterceptor } from "@/auth";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { AgentService } from "./gen/tilde/management/v1/agents_pb.js";
export const agents = createClient(
  AgentService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

export const agentAccess = createClient(
  AgentAccessService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

export const connections = createClient(
  ConnectionsService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
