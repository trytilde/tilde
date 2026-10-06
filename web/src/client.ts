import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { AgentAccessService } from "@trytilde/contracts/tilde/management/v1/access_pb.js";
import { AgentService } from "@trytilde/contracts/tilde/management/v1/agents_pb.js";
import { ConnectionsService } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { DeploymentService } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { InferenceService } from "@trytilde/contracts/tilde/management/v1/inference_pb.js";
import { LogsService } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { PromptService } from "@trytilde/contracts/tilde/management/v1/prompts_pb.js";
import { RoutineService } from "@trytilde/contracts/tilde/management/v1/routines_pb.js";
import { SandboxBlueprintService } from "@trytilde/contracts/tilde/management/v1/sandboxes_pb.js";
import { SkillService } from "@trytilde/contracts/tilde/management/v1/skills_pb.js";
import { TildeChatProviderService } from "@trytilde/contracts/tilde/management/v1/tilde_chat_pb.js";
import {
  ToolHostRegistryService,
  ToolService,
} from "@trytilde/contracts/tilde/management/v1/tools_pb.js";
import { TracingService } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { ConnectionSetupService } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";

const transport = createConnectTransport({
  baseUrl: window.location.origin,
});

export const agents = createClient(AgentService, transport);
export const agentAccess = createClient(AgentAccessService, transport);
export const connections = createClient(ConnectionsService, transport);
export const deployments = createClient(DeploymentService, transport);
export const traces = createClient(TracingService, transport);
export const logs = createClient(LogsService, transport);
export const inference = createClient(InferenceService, transport);
export const tildeChat = createClient(TildeChatProviderService, transport);
export const prompts = createClient(PromptService, transport);
export const routines = createClient(RoutineService, transport);
export const skills = createClient(SkillService, transport);
export const tools = createClient(ToolService, transport);
export const toolHosts = createClient(ToolHostRegistryService, transport);
export const sandboxes = createClient(SandboxBlueprintService, transport);
// Setup calls authenticate with each setup's own token.
export const connectionSetup = createClient(ConnectionSetupService, transport);
