#!/usr/bin/env node
/**
 * Pack the CLI for npm: one @trytilde/cli-<platform> package per built target, then the
 * @trytilde/cli launcher with those platform packages as exact optionalDependencies.
 *
 * Usage: scripts/pack-cli.mjs <binaries-dir> [output-dir]
 *
 * <binaries-dir> holds one directory per Rust target (as the release workflow uploads them),
 * each containing the `tilde` binary. Targets with no binary present are skipped, so a partial
 * local run still produces a usable launcher.
 */
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const version = readFileSync(join(root, "VERSION"), "utf8").trim();
const targets = JSON.parse(readFileSync(join(root, "packaging/targets.json"), "utf8"));
const [binaries, output = join(root, "dist/npm")] = process.argv.slice(2);
if (!binaries) {
  process.stderr.write("Usage: scripts/pack-cli.mjs <binaries-dir> [output-dir]\n");
  process.exit(2);
}

const staging = join(root, "dist/cli-npm");
rmSync(staging, { recursive: true, force: true });
mkdirSync(staging, { recursive: true });
mkdirSync(output, { recursive: true });

const pack = (dir) =>
  execFileSync("npm", ["pack", "--pack-destination", output], { cwd: dir, stdio: "inherit" });

const published = [];
for (const target of targets) {
  const exe = `tilde${target.exe ?? ""}`;
  const built = join(resolve(binaries), target.rust, exe);
  if (!existsSync(built)) {
    process.stderr.write(`skipping ${target.npm}: no binary at ${built}\n`);
    continue;
  }
  const dir = join(staging, target.npm);
  mkdirSync(dir, { recursive: true });
  copyFileSync(built, join(dir, exe));
  chmodSync(join(dir, exe), 0o755);
  writeFileSync(
    join(dir, "package.json"),
    `${JSON.stringify(
      {
        name: `@trytilde/cli-${target.npm}`,
        version,
        description: `Tilde CLI binary for ${target.os} ${target.cpu}`,
        license: "Apache-2.0",
        repository: {
          type: "git",
          url: "https://github.com/trytilde/tilde",
          directory: "crates/tilde-cli",
        },
        os: [target.os],
        cpu: [target.cpu],
        files: [exe],
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(
    join(dir, "README.md"),
    `# @trytilde/cli-${target.npm}\n\nThe Tilde CLI binary for ${target.os} ${target.cpu}. Installed automatically by [@trytilde/cli](https://www.npmjs.com/package/@trytilde/cli); do not depend on it directly.\n`,
  );
  pack(dir);
  published.push(target);
}
if (!published.length) throw new Error(`No CLI binaries found under ${binaries}`);

// The launcher resolves a platform package at runtime, so every one it could need must be an
// exact-version optional dependency. They are injected here, not committed, so that installing
// this repository never has to resolve a version that is not published yet.
const launcher = join(staging, "cli");
mkdirSync(launcher, { recursive: true });
const source = join(root, "sdk/ts/packages/cli");
for (const file of ["package.json", "README.md"])
  copyFileSync(join(source, file), join(launcher, file));
mkdirSync(join(launcher, "bin"), { recursive: true });
copyFileSync(join(source, "bin/tilde.mjs"), join(launcher, "bin/tilde.mjs"));
const manifest = JSON.parse(readFileSync(join(launcher, "package.json"), "utf8"));
manifest.optionalDependencies = Object.fromEntries(
  published.map((target) => [`@trytilde/cli-${target.npm}`, version]),
);
writeFileSync(join(launcher, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
pack(launcher);

process.stderr.write(
  `Packed the CLI for ${published.map((t) => t.npm).join(", ")} into ${output}\n`,
);
