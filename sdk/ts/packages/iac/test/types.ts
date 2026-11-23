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

// An ontology's connections type its bindings: each one's provider flows into the binding.
import { defineProject, ontology, secret } from "../src/index.js";
type Connections = { github: { provider: "github" }; linear: { provider: "linear" } };
defineProject({
  ontologies: {
    faro: ontology<Connections>({
      environments: {
        production: {
          connections: { github: github.ref("faroai"), linear: connection.ref("linear/faro") },
          secrets: { PG_PASSWORD: secret.ref("FARO_PG_PASSWORD") },
        },
      },
    }),
  },
});
ontology<Connections>({
  // @ts-expect-error a GitHub connection binds a GitHub connection, not a Linear one
  environments: { production: { connections: { github: connection.ref("linear/faro") } } },
});
ontology<Connections>({
  // @ts-expect-error no such connection
  environments: { production: { connections: { gitlab: github.ref("faroai") } } },
});
