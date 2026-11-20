/**
 * `@trytilde/iac`: an agent's Tilde configuration written in code. `tilde deploy` finds the
 * `defineAgent` and `defineProject` values the entry module exports and applies them with
 * `ApplyService.Apply`: preflight lists every action a person must take first (missing
 * connections with links that create them, unknown tools with the nearest name), then one
 * transaction upserts every resource by its key.
 *
 * Values here are plain data with no side effects, so importing them in agent code is free.
 * Nothing is removed because it is missing from a spec: an entry with `delete: true` removes it.
 * The JSON built here is the `AgentSpec` protobuf JSON the server takes.
 */
import type { ConnectionRef, ToolSelection } from "./refs.js";

export { ALL, connection, catalogProvider } from "./refs.js";
export type { All, ConnectionRef, ToolSelection } from "./refs.js";
export * from "./catalog.js";

/** Marks values `tilde deploy` collects; shared with `@trytilde/sdk`'s reader by its key. */
export const SPEC = Symbol.for("tilde.iac");

type Deleted = { delete: true };
const isDeleted = (value: unknown): value is Deleted =>
  !!value && typeof value === "object" && (value as Deleted).delete === true;
const ref = (connection: ConnectionRef) => ({
  slug: connection.slug,
  ...(connection.type ? { type: connection.type } : {}),
});

export type Egress = {
  slug: string;
  connection: ConnectionRef;
  config: Record<string, unknown>;
  /** Default true. */
  enabled?: boolean;
  delete?: boolean;
};
/** A grant for the blueprint's sandboxes to reach the connection's upstream through Tilde. */
export function egress(
  slug: string,
  connection: ConnectionRef,
  config: Record<string, unknown> = {},
  options: { enabled?: boolean; delete?: boolean } = {},
): Egress {
  return { slug, connection, config, ...options };
}

export type Reuse = "thread" | "agent" | "agent_identity" | "global" | "global_identity";
export type Blueprint = {
  /** The blueprint's key: agents declaring the same name share it and must agree on it. */
  name: string;
  /** An E2B or Modal connection. */
  provider: ConnectionRef;
  template: string;
  reuse?: Reuse;
  timings?: {
    sleepAfterSeconds?: number;
    terminateAfterSeconds?: number;
    connectTimeoutSeconds?: number;
  };
  /** Non-secret values. Variables not named here (such as ones set in the app) stay. */
  env?: Record<string, string | Deleted>;
  egress?: Egress[];
};
export const blueprint = (spec: Blueprint): Blueprint => spec;

export type Routine = {
  /** The routine's key within its agent. */
  name: string;
  prompt: string;
  threadTitle?: string;
  /** A five-field UTC cron expression; or `signal`. */
  cron?: string;
  signal?: { connection: ConnectionRef; type: string; filters?: Record<string, string[]> };
  /** Absent: created disabled, then left to people. */
  enabled?: boolean;
  delete?: boolean;
};
export const routine = (spec: Routine): Routine => spec;

/** A deployment the customer already deployed: Tilde registers it, it does not host it. */
export type Deploy = { target: "gateway" | "sidecar" } | { target: "lambda"; functionArn: string };
/** A Lambda function; a version-qualified ARN keeps the deployment fixed. */
export const lambda = (functionArn: string): Deploy => ({ target: "lambda", functionArn });
/** A host that dials in to Tilde's gateway with its deployment token. */
export const gateway = (): Deploy => ({ target: "gateway" });
export const sidecar = (): Deploy => ({ target: "sidecar" });

export type Agent = {
  /** Stable identity in code: lowercase letters, digits and dashes. */
  key: string;
  /** The ID to create the agent with; an existing agent with this ID and no key is taken over. */
  id?: string;
  name: string;
  description?: string;
  /** `tilde.types.v1.Capabilities` as protobuf JSON. Absent keeps the agent's own. */
  capabilities?: Record<string, unknown>;
  concurrency?: "queue" | "interrupt" | "queue_and_batch";
  toolMode?: "direct" | "dynamic";
  /** Alias to inference connection, such as `{ default: openai.ref("prod") }`. */
  inference?: Record<string, ConnectionRef | Deleted>;
  /** `github.ref("acme").tools("get_issue")`, or `{ connection, delete: true }` to remove one. */
  tools?: (ToolSelection | { connection: ConnectionRef; delete: true })[];
  sandbox?: Blueprint;
  routines?: Routine[];
  deploy?: Deploy;
};
export type AgentSpec = { readonly [SPEC]: "agent"; readonly key: string; readonly json: unknown };
export type ProjectSpec = {
  readonly [SPEC]: "project";
  readonly team?: string;
  readonly agents: readonly AgentSpec[];
};

const REUSE = {
  thread: "SANDBOX_REUSE_THREAD",
  agent: "SANDBOX_REUSE_AGENT",
  agent_identity: "SANDBOX_REUSE_AGENT_IDENTITY",
  global: "SANDBOX_REUSE_GLOBAL",
  global_identity: "SANDBOX_REUSE_GLOBAL_IDENTITY",
} as const;

function blueprintJson(spec: Blueprint) {
  return {
    name: spec.name,
    provider: ref(spec.provider),
    template: spec.template,
    reuse: REUSE[spec.reuse ?? "thread"],
    ...(spec.timings ? { timings: spec.timings } : {}),
    env: Object.fromEntries(
      Object.entries(spec.env ?? {}).map(([name, value]) => [
        name,
        isDeleted(value) ? { delete: true } : { literal: value },
      ]),
    ),
    egress: (spec.egress ?? []).map((grant) => ({
      slug: grant.slug,
      connection: ref(grant.connection),
      configJson: JSON.stringify(grant.config),
      enabled: grant.enabled ?? true,
      delete: grant.delete ?? false,
    })),
  };
}
function routineJson(spec: Routine) {
  if (!spec.delete && !!spec.cron === !!spec.signal)
    throw new Error(`Routine ${spec.name} needs exactly one of cron and signal`);
  return {
    name: spec.name,
    prompt: spec.prompt,
    threadTitle: spec.threadTitle ?? "",
    ...(spec.cron ? { trigger: { cron: { schedule: spec.cron } } } : {}),
    ...(spec.signal
      ? {
          trigger: {
            signal: {
              connection: ref(spec.signal.connection),
              signalType: spec.signal.type,
              filters: Object.entries(spec.signal.filters ?? {}).map(([key, values]) => ({
                key,
                values,
              })),
            },
          },
        }
      : {}),
    ...(spec.enabled === undefined ? {} : { enabled: spec.enabled }),
    delete: spec.delete ?? false,
  };
}

/** An agent and everything it is configured with. Export it from the entry module. */
export function defineAgent(spec: Agent): AgentSpec {
  const deploy = spec.deploy;
  const json = {
    key: spec.key,
    ...(spec.id ? { id: spec.id } : {}),
    name: spec.name,
    description: spec.description ?? "",
    ...(spec.capabilities ? { capabilities: spec.capabilities } : {}),
    ...(spec.concurrency
      ? { concurrencyPolicy: `AGENT_CONCURRENCY_POLICY_${spec.concurrency.toUpperCase()}` }
      : {}),
    ...(spec.toolMode ? { toolMode: `TOOL_MODE_${spec.toolMode.toUpperCase()}` } : {}),
    inference: Object.entries(spec.inference ?? {}).map(([alias, value]) =>
      isDeleted(value) ? { alias, delete: true } : { alias, connection: ref(value) },
    ),
    tools: (spec.tools ?? []).map((source) =>
      isDeleted(source)
        ? { connection: ref(source.connection), delete: true }
        : { connection: ref(source.connection), tools: [...new Set(source.names)] },
    ),
    ...(spec.sandbox ? { sandbox: blueprintJson(spec.sandbox) } : {}),
    routines: (spec.routines ?? []).map(routineJson),
    // The CLI fills the external ID and the code's declarations.
    ...(deploy
      ? {
          deployment: {
            target: `DEPLOYMENT_TARGET_${deploy.target.toUpperCase()}`,
            ...(deploy.target === "lambda" ? { targetReference: deploy.functionArn } : {}),
          },
        }
      : {}),
  };
  return Object.freeze({ [SPEC]: "agent" as const, key: spec.key, json });
}

/** Several agents applied together in one transaction, and the team they belong to (Tilde Cloud). */
export function defineProject(spec: { team?: string; agents: AgentSpec[] }): ProjectSpec {
  return Object.freeze({ [SPEC]: "project" as const, team: spec.team, agents: spec.agents });
}
