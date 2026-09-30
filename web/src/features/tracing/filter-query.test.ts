import { describe, expect, it } from "vitest";
import { create } from "@bufbuild/protobuf";
import { ObservationSchema } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { condition, conditions, parseQuery, serializeQuery, suggestions } from "./filter-query";

describe("trace query editor", () => {
  it("round-trips quoted values and applies multiple supported server filters", () => {
    const query = 'name:"model call" level:error model:gpt-4o-mini input:"say \\"hello\\""';
    const parsed = parseQuery(query);
    expect(parsed).toMatchObject({
      name: "model call",
      level: "ERROR",
      model: "gpt-4o-mini",
      input: 'say "hello"',
    });
    expect(parseQuery(serializeQuery(parsed))).toEqual(parsed);
    expect(parseQuery("checkout failed").name).toBe("checkout failed");
    expect(parseQuery("name:*checkout*").name).toBe("checkout");
    expect(conditions(parsed).map((c) => [c.column, c.operator, c.values])).toEqual([
      ["name", "contains", ["model call"]],
      ["level", "=", ["ERROR"]],
      ["model", "=", ["gpt-4o-mini"]],
      ["input", "contains", ['say "hello"']],
    ]);
  });
  it("expresses every filter operator and keeps such terms verbatim in where", () => {
    const query =
      "latency:>2 tokens:1000 -level:DEBUG type:GENERATION|TOOL model:gpt* name:*step -name:retry " +
      'gen_ai.tool.name:search ai.usage.total:>=5 -status:"rate limit" name:a name:b';
    const parsed = parseQuery(query);
    expect(parsed.name).toBe("a");
    expect(parsed.where).toBe(query.replace(" name:a", ""));
    expect(parseQuery(serializeQuery(parsed))).toEqual(parsed);
    const table = conditions(parsed).map((c) => [c.column, c.operator, c.values, c.key]);
    expect(table).toEqual([
      ["name", "contains", ["a"], ""],
      ["latency", ">", ["2"], ""],
      ["total_tokens", "=", ["1000"], ""],
      ["level", "none of", ["DEBUG"], ""],
      ["type", "any of", ["GENERATION", "TOOL"], ""],
      ["model", "starts with", ["gpt"], ""],
      ["name", "ends with", ["step"], ""],
      ["name", "does not contain", ["retry"], ""],
      ["metadata", "=", ["search"], "gen_ai.tool.name"],
      ["metadata", ">=", ["5"], "ai.usage.total"],
      ["status_message", "does not contain", ["rate limit"], ""],
      ["name", "contains", ["b"], ""],
    ]);
    expect(conditions({ preset: "errors" })).toEqual([
      expect.objectContaining({ column: "level", operator: "=", values: ["ERROR"] }),
    ]);
    expect(condition("level:error").operator).toBe("=");
  });
  it("keeps quoted literals literal when a search is rebuilt from the URL", () => {
    for (const [query, column, value] of [
      ['name:"gpt*"', "name", "gpt*"],
      ['model:"a|b"', "model", "a|b"],
      ['model:">x"', "model", ">x"],
      ['type:"(x)"', "type", "(X)"],
    ] as const) {
      const parsed = parseQuery(query);
      expect(parsed[column]).toBe(value);
      const serialized = serializeQuery(parsed);
      expect(parseQuery(serialized)).toEqual(parsed);
      expect(conditions(parsed).map((c) => [c.column, c.operator, c.values])).toEqual([
        [column, column === "name" ? "contains" : "=", [value]],
      ]);
    }
  });
  it.each([
    "level:",
    'name:"unfinished',
    "latency:fast",
    "latency:>fast",
    "name:>x",
    "input:a|b",
    "-latency:2",
    "invocation:not-a-uuid",
    "unknown:1",
    "(name:a)",
  ])("rejects unsupported or incomplete input without dropping it: %s", (query) => {
    expect(() => parseQuery(query)).toThrow();
  });
  it("completes fields, attributes and observed values at the caret", () => {
    const rows = [
      create(ObservationSchema, {
        model: "gpt-4o-mini",
        name: "model call",
        attributes: [{ key: "gen_ai.tool.name", value: "search" }],
      }),
    ];
    expect(suggestions("lev", 3, rows)[0]).toMatchObject({ label: "level:", value: "level:" });
    expect(suggestions("lat", 3, rows)[0]).toMatchObject({ label: "latency:" });
    expect(suggestions("level:ER model:gpt", 8, rows)[0]?.value).toBe("level:ERROR  model:gpt");
    expect(suggestions("level:ERROR model:gp", 20, rows)[0]?.value).toBe(
      "level:ERROR model:gpt-4o-mini ",
    );
    expect(suggestions("name:mod", 8, rows)[0]?.value).toBe('name:"model call" ');
    expect(suggestions("gen_ai", 6, rows)[0]?.label).toBe("gen_ai.tool.name:");
    expect(suggestions("gen_ai.tool.name:se", 19, rows)[0]?.value).toBe("gen_ai.tool.name:search ");
  });
});
