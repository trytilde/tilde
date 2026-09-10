import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { readdirSync, existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
const web = fileURLToPath(new URL(".", import.meta.url));
const root = resolve(web, "../crates/tilde/src/connections/catalog");
const port = Number(process.env.CONNECTION_UI_PORT ?? 5174);
const entries = readdirSync(root).filter((name) => existsSync(resolve(root, name, "ui.tsx")));
const inputs = new Map(entries.map((name) => [resolve(root, name, "ui.html"), name]));
const template = () => readFileSync(resolve(web, "connection-ui.html"), "utf8");
const html = (name: string) => template().replace("__CONNECTION_UI_ENTRY__", `/${name}/ui.tsx`);
const pages: Plugin = {
  name: "connection-ui-pages",
  enforce: "pre",
  resolveId(id) {
    if (inputs.has(id)) return id;
  },
  load(id) {
    const name = inputs.get(id);
    if (name) {
      this.addWatchFile(resolve(web, "connection-ui.html"));
      return html(name);
    }
  },
  transform(code, id) {
    if (entries.some((name) => id === resolve(root, name, "ui.tsx"))) {
      return `import "@trytilde/connection-ui/style.css";\n${code}`;
    }
  },
  configureServer(server) {
    server.watcher.add(resolve(web, "connection-ui.html"));
    server.watcher.on("change", (file) => {
      if (file === resolve(web, "connection-ui.html")) {
        server.ws.send({ type: "full-reload", path: "*" });
      }
    });
    server.middlewares.use(async (req, res, next) => {
      if ((req.url ?? "").includes("html-proxy")) return next();
      const pathname = (req.url ?? "").split("?")[0].replace(/^\/connections\/ui/, "");
      const match = pathname.match(/^\/([^/]+)\/ui\.html$/);
      if (!match || !entries.includes(match[1])) return next();
      try {
        const content = await server.transformIndexHtml(
          `/connections/ui/${match[1]}/ui.html`,
          html(match[1]),
        );
        res.setHeader("Content-Type", "text/html");
        res.end(content);
      } catch (error) {
        next(error);
      }
    });
  },
};
export default defineConfig({
  root,
  base: "/connections/ui/",
  plugins: [pages, react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(web, "src"),
      "@trytilde/connection-ui/style.css": resolve(web, "connection-ui.css"),
      "@trytilde/connection-ui": resolve(web, "../sdk/ts/packages/connection-ui/src/index.ts"),
      react: resolve(web, "node_modules/react"),
      "react-dom": resolve(web, "node_modules/react-dom"),
    },
  },
  server: {
    host: process.env.WEB_HOST ?? "127.0.0.1",
    port,
    strictPort: true,
    cors: { origin: "*" },
    fs: { allow: [resolve(web, "..")] },
    hmr: { host: process.env.WEB_HOST ?? "127.0.0.1", clientPort: port },
  },
  build: {
    outDir: resolve(web, "provider-dist"),
    emptyOutDir: true,
    rollupOptions: {
      input: Object.fromEntries([...inputs].map(([path, name]) => [name, path])),
    },
  },
});
