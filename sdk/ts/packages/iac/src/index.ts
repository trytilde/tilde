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
import type { ConnectionRef, RemovedSource, ToolSelection } from "./refs.js";

export { ALL, connection, catalogProvider } from "./refs.js";
export type { All, ConnectionRef, Connections, RemovedSource, ToolSelection } from "./refs.js";
// Typed catalog providers, generated: rerun `task iac:catalog` when the provider catalog changes.
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
  /**
   * The sandbox connection it launches through, like any connection: an E2B or Modal one, or on
   * Tilde Cloud the team's own Tilde sandbox connection, `connection.ref("tilde_sandbox/sandbox")`.
   */
  connection: ConnectionRef;
  /** The connection's template or image; on the Tilde sandbox connection one of {@link TildeSandboxImage}. */
  template: string;
  reuse?: Reuse;
  timings?: {
    sleepAfterSeconds?: number;
    terminateAfterSeconds?: number;
    connectTimeoutSeconds?: number;
  };
  /**
   * A literal (never a secret: it lives in code) or `secret.ref("NAME")`. Variables not named
   * here (such as ones set in the app) stay.
   */
  env?: Record<string, string | SecretRef | Deleted>;
  egress?: Egress[];
};
export const blueprint = (spec: Blueprint): Blueprint => spec;

/** The Tilde sandbox connection's images, Debian with the Tilde CLI: Node.js, Python, or both with Docker Engine and Compose. */
export type TildeSandboxImage = "debian-node" | "debian-python" | "debian-full";

/** A secret set in the app (Connections → Secrets), referenced by name; its value never enters a spec. */
export type SecretRef = { readonly secret: string };
export const secret = {
  ref(name: string): SecretRef {
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(name))
      throw new Error(`Secret ${name} is not a valid name (letters, digits and underscores)`);
    return Object.freeze({ secret: name });
  },
};

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

/**
 * A chat channel the agent answers on: a channel connection (`googleChat.ref("support")`), or
 * without `connection` the agent's own Tilde chat, which every agent has. `access` is who may
 * talk to it there: `private` (identities it allows), `public` (anyone the channel delivers) or
 * `disabled`; absent keeps the channel's own (a new one starts private). Channels not named stay
 * as they are; `delete: true` unassigns a connection.
 */
export type Channel = {
  connection?: ConnectionRef;
  access?: "private" | "public" | "disabled";
  delete?: boolean;
};

/**
 * Where the agent's code runs. `gateway`, `sidecar` and `lambda` are deployed by the customer and
 * registered by Tilde. `hosted` (Tilde Cloud) is run by Tilde: `tilde deploy` bundles the agent's
 * code (its entry module exports `handler = createLambdaHandler({ run })`) and Tilde runs it on its
 * own Lambda, paid from the organization's credits: `deploy: { target: "hosted" }`.
 */
export type Deploy =
  | { target: "gateway" | "sidecar" | "hosted" }
  | { target: "lambda"; functionArn: string };
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
  /** `github.ref("acme").tools("github_get_issue")`; `ref.tools({ delete: true })` removes one. */
  tools?: (ToolSelection | RemovedSource)[];
  sandbox?: Blueprint;
  routines?: Routine[];
  channels?: Channel[];
  deploy?: Deploy;
  /**
   * Where the code `deploy` releases is, from the project's directory: an entry module, or a
   * folder with its own package.json or pyproject.toml. Default `agents/<key>/`; without either,
   * the entry module exporting this spec is the code (`tilde deploy` from the agent's folder).
   */
  code?: string;
  /** People declared beside the agent (monorepos); merged with `defineTeam`'s by email and name. */
  users?: User[];
  groups?: Group[];
  /** Who may see (`visibility`) or change (`ownership`) the agent: `grant(group("eng"), "ownership")`. */
  access?: Grant[];
};
export type AgentSpec = {
  readonly [SPEC]: "agent";
  readonly key: string;
  readonly json: unknown;
  /** The agent's `code`: read by `tilde deploy`, never sent. */
  readonly code?: string;
};

/**
 * People (Tilde Cloud). A user is found by email and upserted: a changed `name` updates them (an
 * organization administrator's apply). In a team's apply a declared user is a member of the
 * team (joining as a member). `login: true` invites them to sign in and keeps an existing
 * member's role; `login: { role }` also sets their role. `team: { delete: true }` removes them
 * from the team. A group is found by name in
 * the team and created if missing. `delete: true` disables a user or deletes a group. A string
 * names an existing or declared one, as a grant's principal or a group member.
 */
export type User = {
  readonly kind: "user";
  readonly email: string;
  readonly name?: string;
  readonly login?: true | { role: "member" | "admin" };
  readonly team?: { delete: true };
  readonly delete?: boolean;
};
export function user(
  spec:
    | string
    | {
        email: string;
        name?: string;
        login?: true | { role: "member" | "admin" };
        team?: { delete: true };
        delete?: boolean;
      },
): User {
  const fields = typeof spec === "string" ? { email: spec } : spec;
  if (!fields.email.includes("@")) throw new Error(`${fields.email} is not an email address`);
  return Object.freeze({ kind: "user" as const, ...fields });
}
export type Group = {
  readonly kind: "group";
  readonly name: string;
  /** Emails, or `{ email, delete: true }` to remove a member. */
  readonly members?: readonly (string | { email: string; delete: true })[];
  readonly delete?: boolean;
};
export function group(
  spec:
    | string
    | { name: string; members?: (string | { email: string; delete: true })[]; delete?: boolean },
): Group {
  const fields = typeof spec === "string" ? { name: spec } : spec;
  return Object.freeze({ kind: "group" as const, ...fields });
}
export type Grant = {
  readonly principal: User | Group;
  readonly plane: "visibility" | "ownership";
  readonly delete?: boolean;
};
export const grant = (
  principal: User | Group,
  plane: "visibility" | "ownership",
  options: { delete?: boolean } = {},
): Grant => Object.freeze({ principal, plane, ...options });
export type TeamSpec = {
  readonly [SPEC]: "team";
  readonly team: string;
  readonly users: readonly unknown[];
  readonly groups: readonly unknown[];
};

const ROLES = { member: "MEMBER_ROLE_MEMBER", admin: "MEMBER_ROLE_ADMIN" } as const;
const userJson = (u: User) => ({
  email: u.email,
  name: u.name ?? "",
  login: !!u.login,
  delete: u.delete ?? false,
  // Only an explicit role is sent: `login: true` keeps an existing member's role.
  ...(u.team?.delete
    ? { team: { delete: true } }
    : u.login && u.login !== true
      ? { team: { role: ROLES[u.login.role] } }
      : {}),
});
const groupJson = (g: Group) => ({
  name: g.name,
  members: (g.members ?? []).map((m) =>
    typeof m === "string" ? { email: m, delete: false } : { email: m.email, delete: true },
  ),
  delete: g.delete ?? false,
});
const grantJson = (g: Grant) => ({
  ...(g.principal.kind === "user" ? { user: g.principal.email } : { group: g.principal.name }),
  plane: g.plane === "ownership" ? "GRANT_PLANE_OWNERSHIP" : "GRANT_PLANE_VISIBILITY",
  delete: g.delete ?? false,
});

/** The team's people, declared once for every agent applied with it. Export it from an entry. */
export function defineTeam(spec: { team: string; users?: User[]; groups?: Group[] }): TeamSpec {
  return Object.freeze({
    [SPEC]: "team" as const,
    team: spec.team,
    users: (spec.users ?? []).map(userJson),
    groups: (spec.groups ?? []).map(groupJson),
  });
}
/**
 * An ontology's connections, as its own `connections.ts` declares them (Tilde Cloud): each one's
 * provider. Pass the ontology's `connections` type to `ontology<...>()` so a binding to a
 * connection of another provider does not compile:
 * `ontology<typeof import("./ontologies/faro/connections.ts").connections>({ ... })` (type-only).
 */
export type OntologyConnections = { readonly [name: string]: { readonly provider: string } };
/** What one environment of the ontology binds: its connections to the organization's, secret
 * names to secrets. */
export type OntologyEnvironment<C extends OntologyConnections = OntologyConnections> = {
  connections?: { readonly [K in keyof C]?: ConnectionRef<string, C[K]["provider"]> };
  /** The ontology's secret name to a Tilde secret of the project's team (or the organization). */
  secrets?: { readonly [name: string]: SecretRef };
};
/**
 * An ontology of the project (Tilde Cloud), in the repository's `ontologies/<name>/`: per
 * environment, its bindings. `tilde deploy` builds it into a version, binds each environment, rolls the
 * version out, then applies the agents. Its name, the key in `defineProject({ ontologies })`, is
 * its project's in Tilde.
 */
export type Ontology<C extends OntologyConnections = OntologyConnections> = {
  environments: { readonly [environment: string]: OntologyEnvironment<C> };
};
export const ontology = <const C extends OntologyConnections = OntologyConnections>(
  spec: Ontology<C>,
): Ontology<C> => spec;
const ontologyJson = (spec: Ontology) => ({
  environments: Object.fromEntries(
    Object.entries(spec.environments).map(([name, env]) => [
      name,
      {
        connections: Object.fromEntries(
          Object.entries(env.connections ?? {}).flatMap(([name, connection]) =>
            connection ? [[name, ref(connection as ConnectionRef)]] : [],
          ),
        ),
        secrets: Object.fromEntries(
          Object.entries(env.secrets ?? {}).map(([secret, value]) => [secret, value.secret]),
        ),
      },
    ]),
  ),
});

export type ProjectSpec = {
  readonly [SPEC]: "project";
  readonly team?: string;
  readonly agents: readonly AgentSpec[];
  readonly ontologies: { readonly [name: string]: unknown };
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
    connection: ref(spec.connection),
    template: spec.template,
    reuse: REUSE[spec.reuse ?? "thread"],
    ...(spec.timings ? { timings: spec.timings } : {}),
    env: Object.fromEntries(
      Object.entries(spec.env ?? {}).map(([name, value]) => [
        name,
        isDeleted(value)
          ? { delete: true }
          : typeof value === "string"
            ? { literal: value }
            : { secret: { name: value.secret } },
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
    channels: (spec.channels ?? []).map((channel) => ({
      ...(channel.connection ? { connection: ref(channel.connection) } : {}),
      ...(channel.access ? { access: `CHANNEL_ACCESS_MODE_${channel.access.toUpperCase()}` } : {}),
      delete: channel.delete ?? false,
    })),
    users: (spec.users ?? []).map(userJson),
    groups: (spec.groups ?? []).map(groupJson),
    access: (spec.access ?? []).map(grantJson),
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
  return Object.freeze({ [SPEC]: "agent" as const, key: spec.key, json, code: spec.code });
}

/**
 * Several agents applied together in one transaction, the team they belong to, and the project's
 * ontologies (Tilde Cloud), which `tilde deploy` deploys first.
 */
export function defineProject(spec: {
  team?: string;
  agents?: AgentSpec[];
  ontologies?: { readonly [name: string]: Ontology<any> };
}): ProjectSpec {
  const ontologies = Object.fromEntries(
    Object.entries(spec.ontologies ?? {}).map(([name, o]) => {
      if (!/^[a-z0-9][a-z0-9-]{0,62}$/.test(name))
        throw new Error(
          `Ontology ${name}: a name is lowercase letters, digits and dashes (its project's)`,
        );
      return [name, ontologyJson(o)];
    }),
  );
  return Object.freeze({
    [SPEC]: "project" as const,
    team: spec.team,
    agents: spec.agents ?? [],
    ontologies,
  });
}
