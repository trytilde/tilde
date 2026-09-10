import { defineConfig } from "vite-plus";

// One root check policy covers the frontend, provider iframes, SDK, examples and scripts.
// Scan the repository, not just web/: catalog ui.tsx entries live under crates/tilde/src/connections.
// Generated contracts and build outputs are verified by their generators, not reformatted.
const generated = [
  "**/node_modules/**",
  "**/dist/**",
  "**/provider-dist/**",
  "**/gen/**",
  "**/generated/**",
  "target/**",
  ".tools/**",
  ".sqlx/**",
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
