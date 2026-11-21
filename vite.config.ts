import { defineConfig } from "vite-plus";

// One root check policy covers the frontend, provider iframes and scripts, plus the SDK and
// examples where the repository has them.
// Scan the repository, not just web/: catalog ui.tsx entries live under crates/tilde/src/connections.
// Generated contracts and build outputs are verified by their generators, not reformatted.
const generated = [
  "**/node_modules/**",
  "**/dist/**",
  "**/provider-dist/**",
  "**/storybook-static/**",
  "**/gen/**",
  "**/routeTree.gen.ts",
  "**/generated/**",
  "target/**",
  ".tools/**",
  "crates/queries/**",
  // Canonical generated data from scripts/update-inference-prices.py.
  "crates/tilde/src/inference/prices.json",
  // Tool catalog snapshots embedded with include_str!: the AWS and curated MCP server tool lists and
  // Sentry's generated OpenAPI catalog.
  "crates/tilde/src/tools/mcp_catalog_tools.json",
  "crates/tilde/src/tools/providers/*/catalog.json",
  ".run/**",
  ".git/**",
  // The ontology and ducktail crates format and type-check their TypeScript with Deno in their own
  // tasks (`task ontology:check`), under Deno's rules rather than the web app's.
  "crates/ontology/**",
  "crates/ducktail/**",
  // A sample ontology project (Deno-style imports of the ontology SDK) the deploy tests build.
  "crates/ontology-control/tests/fixtures/**",
  // SOPS-encrypted: reformatting would invalidate its MAC.
  "secrets.enc.yaml",
];
export default defineConfig({
  lint: {
    ignorePatterns: generated,
    options: { typeAware: true, typeCheck: true },
  },
  fmt: {
    ignorePatterns: [...generated, "**/*.md", "**/pnpm-lock.yaml"],
  },
});
