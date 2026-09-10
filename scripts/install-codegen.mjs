// Install pinned official release binaries; generation never compiles the application.
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, chmod, rename } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const platform = { linux: "linux", darwin: "darwin", win32: "windows" }[process.platform];
const arch = { x64: "x86_64", arm64: "aarch64" }[process.arch];
if (!platform || !arch)
  throw new Error("Unsupported codegen platform; install the pinned plugins manually.");
const extension = process.platform === "win32" ? ".exe" : "";
await mkdir(join(root, ".tools"), { recursive: true });
/** @type {Array<[string, string, string[]]>} */
const releases = [
  ["anthropics/buffa", "v0.9.1", ["protoc-gen-buffa", "protoc-gen-buffa-packaging"]],
  ["connectrpc/connect-rust", "v0.9.0", ["protoc-gen-connect-rust"]],
];
for (const [repo, version, programs] of releases) {
  const base = `https://github.com/${repo}/releases/download/${version}`;
  const manifestResponse = await fetch(`${base}/checksums-sha256.txt`);
  if (!manifestResponse.ok) throw new Error(`Unable to fetch checksums for ${repo}`);
  const manifest = await manifestResponse.text();
  for (const program of programs) {
    const asset = `${program}-${version}-${platform}-${arch}${extension}`;
    const line = manifest
      .split("\n")
      .find((line) => line.trim().split(/\s+/).at(-1)?.replace(/^\*/, "") === asset);
    const expected = line?.trim().split(/\s+/)[0];
    if (!expected || !/^[a-f0-9]{64}$/.test(expected)) throw new Error(`No checksum for ${asset}`);
    const target = join(root, ".tools", `${program}${extension}`);
    const cached = await readFile(target).catch(() => undefined);
    if (cached && createHash("sha256").update(cached).digest("hex") === expected) continue;
    const response = await fetch(`${base}/${asset}`);
    if (!response.ok) throw new Error(`Unable to download ${asset}: ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    if (createHash("sha256").update(bytes).digest("hex") !== expected)
      throw new Error(`Checksum mismatch for ${asset}`);
    await writeFile(`${target}.tmp`, bytes);
    await chmod(`${target}.tmp`, 0o755);
    await rename(`${target}.tmp`, target);
    console.log(`Installed ${program} ${version}`);
  }
}
