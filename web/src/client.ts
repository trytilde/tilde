import { ConnectionsService } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { AgentAccessService } from "@trytilde/contracts/tilde/management/v1/access_pb.js";
import { authInterceptor } from "@/auth";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { AgentService } from "@trytilde/contracts/tilde/management/v1/agents_pb.js";
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

import { DeploymentService } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
export const deployments = createClient(
  DeploymentService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

import { TracingService } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
export const traces = createClient(
  TracingService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

import { LogsService } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
export const logs = createClient(
  LogsService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

import { ApiKeysService } from "@trytilde/contracts/tilde/management/v1/api_keys_pb.js";
import { InferenceService } from "@trytilde/contracts/tilde/management/v1/inference_pb.js";
export const apiKeys = createClient(
  ApiKeysService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);

export const inference = createClient(
  InferenceService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
import { TildeChatProviderService } from "@trytilde/contracts/tilde/management/v1/tilde_chat_pb.js";
export const tildeChat = createClient(
  TildeChatProviderService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
import { IamService } from "@trytilde/contracts/tilde/management/v1/iam_pb.js";
export const iam = createClient(
  IamService,
  createConnectTransport({ baseUrl: window.location.origin, interceptors: [authInterceptor] }),
);
