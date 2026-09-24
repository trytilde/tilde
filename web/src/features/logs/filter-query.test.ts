import { expect, it } from "vitest";
import { create } from "@bufbuild/protobuf";
import { LogRecordSchema } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { parseLogQuery, serializeLogQuery, logSuggestions } from "./filter-query";
it("round-trips quoted message filters and minimum severity", () => {
  const search = parseLogQuery(
    'message:"retry failed" service:worker severity:ERROR trace:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  );
  expect(search).toMatchObject({
    text: "retry failed",
    service: "worker",
    severity: "17",
    trace: "a".repeat(32),
  });
  expect(parseLogQuery(serializeLogQuery(search))).toEqual(search);
});
it("rejects duplicate and unsupported filters without silently widening a query", () => {
  for (const query of [
    "service:a service:b",
    "unknown:value",
    "severity:99",
    "invocation:bad",
    "trace:bad",
    'message:"unclosed',
    "severity:>17",
  ])
    expect(() => parseLogQuery(query)).toThrow();
});
it("suggests loaded values with safe quoting and severity names", () => {
  const rows = [create(LogRecordSchema, { service: "worker pool" })];
  expect(logSuggestions("service:wor", 11, rows)[0]).toMatchObject({
    value: 'service:"worker pool" ',
    label: "worker pool",
  });
  expect(logSuggestions("severity:er", 11, rows)[0]).toMatchObject({
    value: "severity:ERROR ",
    label: "ERROR",
  });
});

it("maps session and run chips to UUID filters and preserves both on round trip", () => {
  const id = "00000000-0000-4000-8000-000000000001";
  const parsed = parseLogQuery(`session:${id} run:${id}`);
  expect(parsed).toMatchObject({ session: id, run: id });
  expect(parseLogQuery(serializeLogQuery(parsed))).toEqual(parsed);
  expect(() => parseLogQuery("run:invalid")).toThrow();
  expect(() => parseLogQuery("session:invalid")).toThrow();
});

it("offers both scopes before any rows load and preserves scope chips", () => {
  expect(logSuggestions("scope:", 6, []).map((item) => item.label)).toEqual([
    "deployment",
    "invocation",
  ]);
  const query = parseLogQuery("scope:deployment service:worker");
  expect(query.scope).toBe("deployment");
  expect(parseLogQuery(serializeLogQuery(query))).toEqual(query);
  expect(() => parseLogQuery("scope:other")).toThrow();
});
