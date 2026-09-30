import { createHash } from "node:crypto";
import { PromptFormat } from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { currentInvocation, declaration } from "./invocation.js";

/** What agent code declares. Sections are partials the template includes as `{{> name}}`. */
export type PromptDeclaration<C extends Record<string, unknown> = Record<string, unknown>> = {
  template: string;
  sections?: Record<string, string>;
  /** Model settings versioned with the text, for example `{ model, temperature }`. */
  config?: C;
};
export type PromptVariables = Record<string, string | number | boolean>;

/** JSON with object keys sorted at every level, so equal configs hash equally. */
export function canonicalJson(value: unknown): string {
  return JSON.stringify(value, (_key, inner: unknown) =>
    inner && typeof inner === "object" && !Array.isArray(inner)
      ? Object.fromEntries(
          Object.entries(inner as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : 1)),
        )
      : inner,
  );
}
/**
 * The engine computes the same digest; `crates/tilde/src/prompts/mod.rs` holds the rule. It is
 * the same for every format: a dynamic prompt hashes its function source as the template.
 */
export function promptHash(
  template: string,
  sections: Record<string, string>,
  config: string,
): string {
  const hash = createHash("sha256");
  hash.update("template\0").update(template).update("\0");
  for (const name of Object.keys(sections).sort()) {
    hash.update("section\0").update(name).update("\0").update(sections[name]).update("\0");
  }
  hash.update("config\0").update(config);
  return hash.digest("hex");
}
const placeholder = /\{\{\s*(>\s*)?([A-Za-z0-9_./-]+)\s*\}\}/g;
export function promptVariables(template: string, sections: Record<string, string>): string[] {
  const variables = new Set<string>();
  for (const text of [template, ...Object.values(sections)])
    for (const match of text.matchAll(placeholder))
      if (!match[1] && /^[A-Za-z0-9_]+$/.test(match[2])) variables.add(match[2]);
  return [...variables];
}
export const validPromptName = (name: string) => /^[A-Za-z0-9._/-]{1,128}$/.test(name);

/** Anything that can stamp inference calls: `name@hash` in `x-tilde-prompt`. */
export type PromptStamp = { readonly name: string; readonly hash: string };

/**
 * A prompt declared in code with `definePrompt`. `tilde deploy` finds it among the entry
 * module's exports and registers it with the deployment; at run time `render()` marks it
 * active for the current invocation so inference calls carry its stamp.
 */
export class PromptDefinition<C extends Record<string, unknown> = Record<string, unknown>> {
  readonly [declaration] = "prompt" as const;
  readonly name: string;
  /** Mustache when the text has placeholders or sections, otherwise plain. */
  readonly format: PromptFormat;
  readonly template: string;
  readonly sections: Readonly<Record<string, string>>;
  readonly config: Readonly<C>;
  readonly hash: string;
  readonly variables: readonly string[];
  /** Attach to AI SDK `experimental_telemetry.metadata` so spans name the version too. */
  readonly telemetry: { "tilde.prompt.name": string; "tilde.prompt.hash": string };
  constructor(name: string, declaration: PromptDeclaration<C>) {
    if (!validPromptName(name))
      throw new Error("Prompt names use letters, digits, '.', '_', '-' or '/'");
    if (!declaration.template) throw new Error(`Prompt ${name} has an empty template`);
    this.name = name;
    this.template = declaration.template;
    this.sections = Object.freeze({ ...declaration.sections });
    this.config = Object.freeze({ ...((declaration.config ?? {}) as C) });
    for (const match of this.template.matchAll(placeholder))
      if (match[1] && !(match[2] in this.sections))
        throw new Error(`Prompt ${name} includes an undeclared section {{> ${match[2]}}}`);
    this.hash = promptHash(this.template, this.sections, canonicalJson(this.config));
    this.variables = Object.freeze(promptVariables(this.template, this.sections));
    this.format =
      this.variables.length || Object.keys(this.sections).length
        ? PromptFormat.MUSTACHE
        : PromptFormat.PLAIN;
    this.telemetry = { "tilde.prompt.name": name, "tilde.prompt.hash": this.hash };
  }
  /** `name@hash`, the value of the `x-tilde-prompt` header on inference calls. */
  get stamp(): string {
    return `${this.name}@${this.hash}`;
  }
  /**
   * Inline sections, then substitute every `{{variable}}`; a missing variable is an error.
   * Inside an invocation a successful render marks the prompt active for it.
   */
  render(variables: PromptVariables = {}): string {
    const sections = this.sections;
    const rendered = this.template.replace(
      placeholder,
      (whole, partial: string | undefined, key: string) => {
        if (partial) {
          return sections[key].replace(placeholder, (inner, nested, name: string) =>
            nested ? inner : substitute(name, inner),
          );
        }
        return substitute(key, whole);
      },
    );
    // Only a successful render may become a stamp for later inference calls.
    currentInvocation()?.activatePrompt(this);
    return rendered;
    function substitute(key: string, whole: string) {
      if (!/^[A-Za-z0-9_]+$/.test(key)) return whole;
      const value = variables[key];
      if (value === undefined) throw new Error(`Prompt variable ${key} was not supplied`);
      return String(value);
    }
  }
}

/**
 * Declare a prompt at module scope and export it from the agent's entry module. `tilde
 * deploy` registers it with the deployment; Tilde keeps one version per distinct content,
 * so an unchanged prompt never creates a version.
 */
export function definePrompt<C extends Record<string, unknown> = Record<string, unknown>>(
  name: string,
  declaration: PromptDeclaration<C>,
): PromptDefinition<C> {
  return new PromptDefinition(name, declaration);
}
