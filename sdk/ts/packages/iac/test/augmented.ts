// Compile-time checks with a `.d.ts` like the one `tilde iac types` writes: a generated slug
// takes its own type and tool names; any other slug, and untypedTools, take plain strings.
import { connection } from "../src/index.js";

declare module "../src/index.js" {
  interface Connections {
    "acme_mcp/prod": { type: "oauth"; tools: "search" | "fetch" };
  }
}

connection.ref("acme_mcp/prod").tools("search", "*");
connection.ref("acme_mcp/prod", { type: "oauth" });
connection.ref("acme_mcp/prod").untypedTools("anything").tools("fetch");
connection.ref("other/acme").tools("anything");
// @ts-expect-error a typo in a generated connection's tool names
connection.ref("acme_mcp/prod").tools("serch");
// @ts-expect-error not the generated connection's type
connection.ref("acme_mcp/prod", { type: "api" });
