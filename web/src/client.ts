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

import { DeploymentService } from "./gen/tilde/management/v1/deployments_pb.js";
export const deployments = createClient(
  DeploymentService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

import { TracingService } from "./gen/tilde/management/v1/tracing_pb.js";
export const traces = createClient(
  TracingService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

import { LogsService } from "./gen/tilde/management/v1/logs_pb.js";
export const logs = createClient(
  LogsService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
