import { spawnSync } from "node:child_process";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
const temp = await mkdtemp(join(tmpdir(), "tilde-codegen-"));
async function files(root, relative = "") {
  const entries = await readdir(join(root, relative), { withFileTypes: true }).catch(() => []);
  return (
    await Promise.all(
      entries.map(async (entry) =>
        entry.isDirectory()
          ? files(root, join(relative, entry.name))
          : [join(relative, entry.name)],
      ),
    )
  )
    .flat()
    .sort((a, b) => a.localeCompare(b));
}
try {
  const result = spawnSync("pnpm", ["exec", "buf", "generate", "--output", temp], {
    stdio: "inherit",
  });
  if (result.status !== 0) process.exitCode = 1;
  else {
    const changed = [];
    // TypeScript contracts are not committed; @trytilde/contracts regenerates them on build.
    for (const folder of ["crates/tilde/src/generated"]) {
      const names = new Set([...(await files(folder)), ...(await files(join(temp, folder)))]);
      for (const name of names) {
        const [a, b] = await Promise.all([
          readFile(join(folder, name)).catch(() => undefined),
          readFile(join(temp, folder, name)).catch(() => undefined),
        ]);
        if (!a || !b || !a.equals(b)) changed.push(join(folder, name));
      }
    }
    if (changed.length) {
      console.error("Run pnpm generate; stale generated files:\n" + changed.join("\n"));
      process.exitCode = 1;
    } else console.log("Generated Rust matches the Protobuf contracts.");
  }
} finally {
  await rm(temp, { recursive: true, force: true });
}
