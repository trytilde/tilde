import { expect, it } from "vitest";
import { readPrefill, withPrefill } from "@trytilde/connection-ui";

it("reads the type, the name and non-secret fields, ignoring everything else", () => {
  expect(
    readPrefill(
      "?connection_setup_token=cst&type=github_app&name=Acme&field.app_name=Acme%20Factory" +
        "&field.owner_type=organization&field.webhook_secret=s&field.private_key=pem" +
        "&field.api_key=k&field.token=t&field.password=p&utm_source=mail",
    ),
  ).toEqual({
    type: "github_app",
    name: "Acme",
    fields: { app_name: "Acme Factory", owner_type: "organization" },
  });
});

it("keeps the first of repeated parameters and drops malformed or oversized ones", () => {
  expect(
    readPrefill(
      new URLSearchParams([
        ["type", "oauth"],
        ["type", "pat"],
        ["name", "first"],
        ["name", "second"],
        ["field.region", "eu-west-1"],
        ["field.region", "us-east-1"],
        ["field.", "nameless"],
        ["field.a.b", "dotted"],
        ["field.notes", "x".repeat(1001)],
        ["field.empty", ""],
      ]),
    ),
  ).toEqual({ type: "oauth", name: "first", fields: { region: "eu-west-1" } });
  // A type is an id, never a path or a script.
  expect(readPrefill("?type=../../admin")).toEqual({});
  expect(readPrefill("")).toEqual({});
});

it("adds the prefill to relative and absolute setup URLs, never a secret", () => {
  expect(
    withPrefill("/connections/broker/stp_1?connection_setup_token=cst", {
      name: "Acme Inc",
      fields: { app_name: "Acme Factory", client_secret: "s" },
    }),
  ).toBe(
    "/connections/broker/stp_1?connection_setup_token=cst&name=Acme+Inc&field.app_name=Acme+Factory",
  );
  expect(withPrefill("https://tilde.test/catalog/github/ui", { type: "github_app" })).toBe(
    "https://tilde.test/catalog/github/ui?type=github_app",
  );
  expect(withPrefill("/catalog/_standard/ui")).toBe("/catalog/_standard/ui");
  // What one end builds the other reads back unchanged.
  const prefill = { type: "oidc", name: "acme", fields: { region: "eu-west-1" } };
  expect(readPrefill(new URL(withPrefill("/x", prefill), "https://t.test").search)).toEqual(
    prefill,
  );
});
