// `tilde deploy`'s bundle of a Hosted agent (crates/tilde-cli/src/hosted.rs): esbuild's API with
// text imports (`import prompt from "./x.md" with { type: "text" }`, which esbuild's own loaders
// refuse) inlined as strings. Prints the bundle's exports as JSON.
const [esbuildPath, entry, outfile, banner] = process.argv.slice(2);
const { build } = await import(esbuildPath);
const { readFile } = await import("node:fs/promises");
const { resolve } = await import("node:path");
const text = {
  name: "text-imports",
  setup(b) {
    b.onResolve({ filter: /.*/ }, (args) =>
      args.with?.type === "text"
        ? { path: resolve(args.resolveDir, args.path), namespace: "text" }
        : undefined,
    );
    b.onLoad({ filter: /.*/, namespace: "text" }, async (args) => ({
      contents: await readFile(args.path, "utf8"),
      loader: "text",
    }));
  },
};
const result = await build({
  entryPoints: [entry],
  bundle: true,
  platform: "node",
  target: "node22",
  format: "esm",
  outfile,
  banner: { js: banner },
  metafile: true,
  logLevel: "warning",
  plugins: [text],
});
const exports = Object.values(result.metafile.outputs).flatMap((output) => output.exports ?? []);
process.stdout.write(JSON.stringify(exports));
