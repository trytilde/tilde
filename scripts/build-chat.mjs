import { spawnSync } from "node:child_process";
import { copyFileSync } from "node:fs";
for (const name of ["chat-proxy", "chat-next", "chat-ui"]) {
  const result = spawnSync("pnpm", ["exec", "tsc", "-p", "tsconfig.json"], {
    cwd: `sdk/ts/packages/${name}`,
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
copyFileSync("sdk/ts/packages/chat-ui/src/style.css", "sdk/ts/packages/chat-ui/dist/style.css");
