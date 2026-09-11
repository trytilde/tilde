import { defineConfig } from "vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
export default defineConfig({
  plugins: [tanstackRouter({ target: "react", autoCodeSplitting: true }), react(), tailwindcss()],
  // connection-ui.html is a template whose entry is supplied by the provider build.
  optimizeDeps: { entries: ["index.html"] },
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      "@trytilde/agent-avatar": fileURLToPath(
        new URL("../sdk/ts/packages/agent-avatar/src/agent-avatar.tsx", import.meta.url),
      ),
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@trytilde/connection-ui": fileURLToPath(
        new URL("../sdk/ts/packages/connection-ui/src/index.ts", import.meta.url),
      ),
    },
  },
  server: {
    host: process.env.WEB_HOST ?? "127.0.0.1",
    port: Number(process.env.WEB_PORT ?? 5173),
    strictPort: true,
    proxy: {
      "^/auth/(login|exchange|session|logout)$": {
        target: process.env.ENGINE_DEV_URL ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
      "^/tilde\\.(management|setup)\\.": {
        target: process.env.ENGINE_DEV_URL ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
      "/identity/verify": {
        target: process.env.ENGINE_DEV_URL ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
      "/connections/callback": {
        target: process.env.ENGINE_DEV_URL ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
      "/readyz": {
        target: process.env.ENGINE_DEV_URL ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
    },
  },
});
