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

/** `ref.tools({ delete: true })`: the agent stops using the connection's tools. */
export type RemovedSource = { readonly connection: ConnectionRef<string>; readonly delete: true };

export type ConnectionRef<Tool extends string = string, Provider extends string = string> = {
  readonly slug: string;
  /** The slug's provider (`github` of `github/acme`), typed on catalog refs: an ontology slot
   * bound to it must be of that provider. */
  readonly provider: Provider;
  /** The connection type preflight requires, such as `github_app`. */
  readonly type?: string;
  tools(remove: { delete: true }): RemovedSource;
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

function ref<Tool extends string, Provider extends string = string>(
  slug: string,
  type?: string,
): ConnectionRef<Tool, Provider> {
  if (!/^[^/\s]+\/[^/\s]+$/.test(slug))
    throw new Error(`Connection ${slug} is not a provider/name slug`);
  const connection: ConnectionRef<Tool, Provider> = Object.freeze({
    slug,
    provider: slug.split("/")[0] as Provider,
    type,
    tools: ((...names: (Tool | All | { delete: true })[]) =>
      typeof names[0] === "object"
        ? Object.freeze({ connection, delete: true as const })
        : selection(connection, names as (Tool | All)[])) as ConnectionRef<Tool, Provider>["tools"],
    untypedTools: (...names: string[]) => selection(connection, names),
  });
  return connection;
}

/**
 * The organization's connections by slug, filled in by the `.d.ts` that `tilde iac types`
 * writes (module augmentation of `@trytilde/iac`): each slug's connection type and tool names.
 * Empty until then, so every slug takes plain strings.
 */
export interface Connections {}
type Known<Slug extends string> = Slug extends keyof Connections ? Connections[Slug] : never;
type ToolsOf<Slug extends string> =
  Known<Slug> extends { tools: infer Tool extends string } ? Tool : string;
type TypeOf<Slug extends string> =
  Known<Slug> extends { type: infer Type extends string } ? Type : string;
type ProviderOf<Slug extends string> = Slug extends `${infer Provider}/${string}`
  ? Provider
  : string;

/**
 * Any connection by its full slug. A slug `tilde iac types` generated takes its own type and
 * tool names (a typo does not compile); any other takes plain strings, which preflight checks.
 */
export const connection = {
  ref: <Slug extends string>(
    slug: Slug,
    options: { type?: TypeOf<Slug> } = {},
  ): ConnectionRef<ToolsOf<Slug>, ProviderOf<Slug>> => ref(slug, options.type),
};

/** A typed provider of the catalog: `github.ref("acme")` is the connection `github/acme`. */
export function catalogProvider<
  Tool extends string,
  Type extends string,
  Provider extends string = string,
>(provider: Provider) {
  return {
    ref: (name: string, options: { type?: Type } = {}): ConnectionRef<Tool, Provider> =>
      ref(`${provider}/${name}`, options.type),
  };
}
