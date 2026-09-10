import {
  context,
  trace,
  ROOT_CONTEXT,
  createContextKey,
  SpanKind,
  SpanStatusCode,
  type Context,
} from "@opentelemetry/api";
import { NodeTracerProvider } from "@opentelemetry/sdk-trace-node";
import {
  BatchSpanProcessor,
  AlwaysOnSampler,
  type SpanProcessor,
  type Span,
  type ReadableSpan,
} from "@opentelemetry/sdk-trace-base";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-proto";
import { W3CTraceContextPropagator } from "@opentelemetry/core";
import { resourceFromAttributes } from "@opentelemetry/resources";
import type { Interceptor } from "@connectrpc/connect";

const sessionKey = createContextKey("tilde.invocation.telemetry");
const propagator = new W3CTraceContextPropagator();
const getter = {
  get: (headers: Headers, key: string) => headers.get(key) ?? undefined,
  keys: (headers: Headers) => [...headers.keys()],
};
const setter = { set: (headers: Headers, key: string, value: string) => headers.set(key, value) };

/** Add this processor to an existing OTel provider, then use tracing: "existing" on createAgentServer. */
class InvocationSpanProcessor implements SpanProcessor {
  private readonly owners = new WeakMap<ReadableSpan, BatchSpanProcessor>();
  private readonly processors = new Set<BatchSpanProcessor>();
  onStart(span: Span, parent: Context) {
    const processor = parent.getValue(sessionKey) as BatchSpanProcessor | undefined;
    if (processor) this.owners.set(span, processor);
  }
  onEnd(span: ReadableSpan) {
    this.owners.get(span)?.onEnd(span);
    this.owners.delete(span);
  }
  add(processor: BatchSpanProcessor) {
    this.processors.add(processor);
  }
  remove(processor: BatchSpanProcessor) {
    this.processors.delete(processor);
  }
  async forceFlush() {
    await Promise.all([...this.processors].map((p) => p.forceFlush()));
  }
  async shutdown() {
    await Promise.all([...this.processors].map((p) => p.shutdown()));
    this.processors.clear();
  }
}
export const agentSpanProcessor = new InvocationSpanProcessor();
let initialized = false;
export function initializeTracing(existing: boolean) {
  if (existing || initialized) return;
  const provider = new NodeTracerProvider({
    resource: resourceFromAttributes({ "service.name": "tilde-agent" }),
    sampler: new AlwaysOnSampler(),
    spanProcessors: [agentSpanProcessor],
  });
  provider.register();
  initialized = true;
}

export function invocationTracing(
  request: {
    callbackUrl: string;
    invocationId: string;
    agentId: string;
    runId: string;
    threadId: string;
  },
  headers: Headers,
  token: () => string,
) {
  // No destination on Tilde means its invocation context is not sampled.
  const flags = headers.get("traceparent")?.split("-")[3];
  if (!flags || !(Number.parseInt(flags, 16) & 1)) {
    return {
      context: ROOT_CONTEXT,
      run<T>(fn: () => T): T {
        return context.with(ROOT_CONTEXT, fn);
      },
      async end(_failed: boolean) {},
    };
  }
  // Match the runtime RPC base path when a reverse proxy separates audiences by prefix.
  const traceEndpoint = new URL(request.callbackUrl);
  traceEndpoint.pathname = `${traceEndpoint.pathname.replace(/\/$/, "")}/v1/traces`;
  traceEndpoint.search = "";
  traceEndpoint.hash = "";
  const exporter = new OTLPTraceExporter({
    url: traceEndpoint.href,
    headers: async () => ({ Authorization: token() }),
    timeoutMillis: 15000,
  });
  const processor = new BatchSpanProcessor(exporter, {
    maxQueueSize: 2048,
    maxExportBatchSize: 256,
    scheduledDelayMillis: 200,
    exportTimeoutMillis: 20000,
  });
  agentSpanProcessor.add(processor);
  const parent = propagator.extract(ROOT_CONTEXT, headers, getter).setValue(sessionKey, processor);
  const span = trace.getTracer("@trytilde/sdk").startSpan(
    "agent.invoke",
    {
      kind: SpanKind.SERVER,
      attributes: {
        "tilde.invocation.id": request.invocationId,
        "tilde.agent.id": request.agentId,
        "tilde.run.id": request.runId,
        "tilde.thread.id": request.threadId,
      },
    },
    parent,
  );
  const active = trace.setSpan(parent, span);
  let ended = false;
  return {
    context: active,
    run<T>(fn: () => T): T {
      return context.with(active, fn);
    },
    async end(failed: boolean) {
      if (ended) return;
      ended = true;
      if (failed)
        span.setStatus({ code: SpanStatusCode.ERROR, message: "Agent invocation failed" });
      span.end();
      try {
        await processor.shutdown();
      } catch {
        /* OTel reports exporter failures; tracing must not fail agent work. */
      } finally {
        agentSpanProcessor.remove(processor);
      }
    },
  };
}

/** One span covers the complete response stream; request/response bodies stay opaque. */
export const tracingInterceptor: Interceptor = (next) => async (request) => {
  const parent = context.active();
  const span = trace
    .getTracer("@trytilde/sdk")
    .startSpan(
      `${request.service.typeName}/${request.method.name}`,
      { kind: SpanKind.CLIENT },
      parent,
    );
  const active = trace.setSpan(parent, span);
  let ended = false;
  const end = () => {
    if (ended) return;
    ended = true;
    request.signal.removeEventListener("abort", abort);
    span.end();
  };
  const abort = () => {
    span.setStatus({ code: SpanStatusCode.ERROR, message: "RPC canceled" });
    end();
  };
  request.signal.addEventListener("abort", abort, { once: true });
  if (request.signal.aborted) abort();

  propagator.inject(active, request.header, setter);
  try {
    const response = await context.with(active, () => next(request));
    if (!response.stream) {
      end();
      return response;
    }
    const source = response.message;
    async function* stream() {
      const iterator = source[Symbol.asyncIterator]();
      try {
        while (true) {
          const value = await context.with(active, () => iterator.next());
          if (value.done) return;
          yield value.value;
        }
      } catch (error) {
        span.setStatus({ code: SpanStatusCode.ERROR, message: "RPC stream failed" });
        throw error;
      } finally {
        try {
          await context.with(active, () => iterator.return?.());
        } finally {
          end();
        }
      }
    }
    return { ...response, message: stream() };
  } catch (error) {
    span.setStatus({ code: SpanStatusCode.ERROR, message: "RPC failed" });
    end();
    throw error;
  }
};
