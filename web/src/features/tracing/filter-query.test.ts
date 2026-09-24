import { describe, expect, it } from "vitest";
import { create } from "@bufbuild/protobuf";
import { ObservationSchema } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { parseQuery, serializeQuery, suggestions } from "./filter-query";

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
  });
  it.each([
    "level:",
    'name:"unfinished',
    "latency:>2",
    "name:a name:b",
    "input:>x",
    "-level:ERROR",
    "invocation:not-a-uuid",
  ])("rejects unsupported or incomplete input without dropping it: %s", (query) => {
    expect(() => parseQuery(query)).toThrow();
  });
  it("completes fields and observed values at the caret without replacing other terms", () => {
    const rows = [create(ObservationSchema, { model: "gpt-4o-mini", name: "model call" })];
    expect(suggestions("lev", 3, rows)[0]).toMatchObject({ label: "level:", value: "level:" });
    expect(suggestions("level:ER model:gpt", 8, rows)[0]?.value).toBe("level:ERROR  model:gpt");
    expect(suggestions("level:ERROR model:gp", 20, rows)[0]?.value).toBe(
      "level:ERROR model:gpt-4o-mini ",
    );
    expect(suggestions("name:mod", 8, rows)[0]?.value).toBe('name:"model call" ');
  });
});
