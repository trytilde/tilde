/**
 * Connection references and tool selections. A connection is only ever referenced, by slug
 * (`provider/name`); Tilde never creates one from a spec. A missing one makes `tilde deploy`
 * print a link that creates it.
 */

/** Every tool the connection offers at apply time. `"*"` is the same value. */
export const ALL = "*";
export type All = typeof ALL;

export type ToolSelection<Tool extends string = string> = {
  readonly connection: ConnectionRef<Tool>;
  readonly names: readonly string[];
  /** Adds checked names: on a typed catalog ref, a typo is a compile error. */
  tools(...names: (Tool | All)[]): ToolSelection<Tool>;
  /** Adds names without checking them in TypeScript; `tilde deploy` still validates them. */
  untypedTools(...names: string[]): ToolSelection<Tool>;
};

export type ConnectionRef<Tool extends string = string> = {
  readonly slug: string;
  /** The connection type preflight requires, such as `github_app`. */
  readonly type?: string;
  tools(...names: (Tool | All)[]): ToolSelection<Tool>;
  untypedTools(...names: string[]): ToolSelection<Tool>;
};

function selection<Tool extends string>(
  connection: ConnectionRef<Tool>,
  names: readonly string[],
): ToolSelection<Tool> {
  return Object.freeze({
    connection,
    names,
    tools: (...more: (Tool | All)[]) => selection(connection, [...names, ...more]),
    untypedTools: (...more: string[]) => selection(connection, [...names, ...more]),
  });
}

function ref<Tool extends string>(slug: string, type?: string): ConnectionRef<Tool> {
  if (!/^[^/\s]+\/[^/\s]+$/.test(slug))
    throw new Error(`Connection ${slug} is not a provider/name slug`);
  const connection: ConnectionRef<Tool> = Object.freeze({
    slug,
    type,
    tools: (...names: (Tool | All)[]) => selection(connection, names),
    untypedTools: (...names: string[]) => selection(connection, names),
  });
  return connection;
}

/** Any connection by its full slug; its tool names are plain strings, validated by preflight. */
export const connection = {
  ref: (slug: string, options: { type?: string } = {}): ConnectionRef<string> =>
    ref(slug, options.type),
};

/** A typed provider of the catalog: `github.ref("acme")` is the connection `github/acme`. */
export function catalogProvider<Tool extends string, Type extends string>(provider: string) {
  return {
    ref: (name: string, options: { type?: Type } = {}): ConnectionRef<Tool> =>
      ref(`${provider}/${name}`, options.type),
  };
}
