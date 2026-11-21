import { defineConfig } from "vite-plus";

// One root check policy covers the frontend, provider iframes and scripts, plus the SDK and
// examples where the repository has them.
// Scan the repository, not just web/: catalog ui.tsx entries live under crates/tilde/src/connections.
// Generated contracts and build outputs are verified by their generators, not reformatted.
const generated = [
  "**/node_modules/**",
  "**/dist/**",
  "**/provider-dist/**",
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
