// Compile-time checks (tsc --noEmit): typed catalog refs close their tool names, `"*"`/ALL
// included; `untypedTools` and untyped refs take any string.
import { ALL, connection, github } from "../src/index.js";

github.ref("acme").tools("github_get_issue", ALL, "*");
github.ref("acme").untypedTools("anything").tools("github_get_issue");
github.ref("acme", { type: "github_app" });
connection.ref("custom/acme").tools("whatever", "*");
github.ref("acme").tools({ delete: true });
// @ts-expect-error a typo is a compile error on a typed ref
github.ref("acme").tools("github_get_isue");
// @ts-expect-error not a connection type of the provider
github.ref("acme", { type: "api_key" });
// @ts-expect-error also after untypedTools
github.ref("acme").untypedTools("x").tools("github_get_isue");

// An ontology's slots type its bindings: the slot's provider flows into the connection.
import { defineProject, ontology, secret } from "../src/index.js";
type Slots = { github: { provider: "github" }; linear: { provider: "linear" } };
defineProject({
  ontologies: {
    faro: ontology<Slots>({
      dir: "ontologies/faro",
      environments: {
        production: {
          slots: { github: github.ref("faroai"), linear: connection.ref("linear/faro") },
          secrets: { PG_PASSWORD: secret.ref("FARO_PG_PASSWORD") },
        },
      },
    }),
  },
});
ontology<Slots>({
  dir: "ontologies/faro",
  // @ts-expect-error a GitHub slot takes a GitHub connection, not a Linear one
  environments: { production: { slots: { github: connection.ref("linear/faro") } } },
});
ontology<Slots>({
  dir: "ontologies/faro",
  // @ts-expect-error no such slot
  environments: { production: { slots: { gitlab: github.ref("faroai") } } },
});
