import { context, createContextKey, type Context } from "@opentelemetry/api";
import { logs } from "@opentelemetry/api-logs";
import {
  LoggerProvider,
  BatchLogRecordProcessor,
  type LogRecordProcessor,
  type ReadWriteLogRecord,
} from "@opentelemetry/sdk-logs";
import { OTLPLogExporter } from "@opentelemetry/exporter-logs-otlp-proto";
import { resourceFromAttributes } from "@opentelemetry/resources";

const invocationKey = createContextKey("tilde.invocation.logs");
type Session = {
  processor: BatchLogRecordProcessor;
  attributes: Record<string, string>;
  closed: boolean;
};

/** Attach to any OTel LoggerProvider. Routing is captured at emit time, before batching. */
class InvocationLogProcessor implements LogRecordProcessor {
  private readonly sessions = new Set<Session>();
  private deployment?: Session;
  private deploymentConfig?: DeploymentLoggingOptions;
  configureDeployment(options: DeploymentLoggingOptions) {
    if (this.deploymentConfig) {
      if (
        this.deploymentConfig.gatewayUrl !== options.gatewayUrl ||
        this.deploymentConfig.deploymentToken !== options.deploymentToken
      )
        throw new Error("agentLogProcessor supports one deployment per process");
      return;
    }
    const { gatewayUrl, deploymentToken } = options;
    const processor = batchProcessor(gatewayUrl, () => `Bearer ${deploymentToken}`);
    this.deploymentConfig = { gatewayUrl, deploymentToken };
    this.deployment = {
      processor,
      attributes: { "tilde.log.scope": "deployment" },
      closed: false,
    };
    this.sessions.add(this.deployment);
  }
  onEmit(record: ReadWriteLogRecord, parent?: Context) {
    // A closed invocation stays closed: never reclassify its late logs as deployment logs.
    const invocation = (parent ?? context.active()).getValue(invocationKey) as Session | undefined;
    const session = invocation ?? this.deployment;
    if (!session || session.closed) return;
    record.setAttributes(session.attributes);
    session.processor.onEmit(record);
  }
  add(session: Session) {
    this.sessions.add(session);
  }
  remove(session: Session) {
    this.sessions.delete(session);
  }
  async forceFlush() {
    await Promise.all([...this.sessions].map((s) => s.processor.forceFlush()));
  }
  async shutdown() {
    const sessions = [...this.sessions];
    for (const session of sessions) session.closed = true;
    try {
      await Promise.all(sessions.map((s) => s.processor.shutdown()));
    } finally {
      this.sessions.clear();
      this.deployment = undefined;
      this.deploymentConfig = undefined;
    }
  }
}
export const agentLogProcessor = new InvocationLogProcessor();
let initialized = false;
/** One process owns one deployment. Configure before startup logs are emitted. */
export type DeploymentLoggingOptions = { gatewayUrl: string; deploymentToken: string };
export function configureDeploymentLogging(options: DeploymentLoggingOptions) {
  agentLogProcessor.configureDeployment(options);
}
export function initializeLogging(existing: boolean, deployment?: DeploymentLoggingOptions) {
  if (deployment) configureDeploymentLogging(deployment);
  if (existing || initialized) return;
  const provider = new LoggerProvider({
    resource: resourceFromAttributes({
      "service.name": process.env.OTEL_SERVICE_NAME || "tilde-agent",
    }),
    processors: [agentLogProcessor],
  });
  logs.setGlobalLoggerProvider(provider);
  initialized = true;
}
export function invocationLogging(
  request: {
    callbackUrl: string;
    agentId: string;
    threadId: string;
    runId: string;
    invocationId: string;
  },
  parent: Context,
  token: () => string,
) {
  let authorization: (() => string) | undefined = token;
  const processor = batchProcessor(request.callbackUrl, () => authorization?.() ?? "");
  const session: Session = {
    processor,
    closed: false,
    attributes: {
      "tilde.log.scope": "invocation",
      "tilde.agent.id": request.agentId,
      "tilde.thread.id": request.threadId,
      "tilde.run.id": request.runId,
      "tilde.invocation.id": request.invocationId,
    },
  };
  agentLogProcessor.add(session);
  const active = parent.setValue(invocationKey, session);
  let finished: Promise<void> | undefined;
  return {
    run<T>(fn: () => T): T {
      return context.with(active, fn);
    },
    end() {
      return (finished ??= (async () => {
        session.closed = true;
        try {
          await processor.shutdown();
        } catch {
          /* Export failure must not fail the invocation. */
        } finally {
          authorization = undefined;
          agentLogProcessor.remove(session);
        }
      })());
    },
  };
}

function batchProcessor(baseUrl: string, authorization: () => string) {
  const endpoint = new URL(baseUrl);
  endpoint.pathname = `${endpoint.pathname.replace(/\/$/, "")}/v1/logs`;
  endpoint.search = "";
  endpoint.hash = "";
  return new BatchLogRecordProcessor({
    exporter: new OTLPLogExporter({
      url: endpoint.href,
      headers: async () => ({ Authorization: authorization() }),
      timeoutMillis: 15000,
    }),
    maxQueueSize: 2048,
    maxExportBatchSize: 256,
    scheduledDelayMillis: 200,
    exportTimeoutMillis: 20000,
  });
}
