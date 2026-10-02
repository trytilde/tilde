#!/usr/bin/env node
// Hands straight over to the platform binary. `scripts/pack-cli.mjs` publishes one
// @trytilde/cli-<platform> package per target and lists them as this package's
// optionalDependencies, so a plain `npm install` fetches only the one that can run here.
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";

const PLATFORMS = {
  "darwin arm64": "darwin-arm64",
  "darwin x64": "darwin-x64",
  "linux arm64": "linux-arm64",
  "linux x64": "linux-x64",
  "win32 x64": "win32-x64",
};

function binary() {
  // Set by `task cli:link` and by CI, so a local build can stand in for a published binary.
  if (process.env.TILDE_CLI_BINARY) return process.env.TILDE_CLI_BINARY;
  const platform = PLATFORMS[`${process.platform} ${process.arch}`];
  if (!platform) {
    throw new Error(
      `The Tilde CLI has no build for ${process.platform} ${process.arch}. ` +
        "Install it from source with `cargo install --git https://github.com/trytilde/tilde tilde-cli`.",
    );
  }
  const name = `@trytilde/cli-${platform}`;
  const file = `tilde${process.platform === "win32" ? ".exe" : ""}`;
  try {
    return createRequire(import.meta.url).resolve(`${name}/${file}`);
  } catch {
    throw new Error(
      `${name} is not installed. Reinstall without --no-optional, or install the CLI with ` +
        "`curl -fsSL https://trytilde.ai/install.sh | sh`.",
    );
  }
}

try {
  const { status, error } = spawnSync(binary(), process.argv.slice(2), { stdio: "inherit" });
  if (error) throw error;
  process.exit(status ?? 1);
} catch (error) {
  process.stderr.write(`tilde: ${error.message}\n`);
  process.exit(1);
}
