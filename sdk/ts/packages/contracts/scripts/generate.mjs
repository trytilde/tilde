// Generate the TypeScript contracts from the repository's proto module into gen/.
// The output is not committed: every consumer builds it from the protos.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const here = fileURLToPath(new URL(".", import.meta.url));
const pkg = resolve(here, "..");
const root = resolve(pkg, "../../../..");
const result = spawnSync(
  "pnpm",
  ["exec", "buf", "generate", root, "--template", resolve(pkg, "buf.gen.yaml")],
  { cwd: pkg, stdio: "inherit" },
);
process.exit(result.status ?? 1);
