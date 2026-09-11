import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
  plugins: [tanstackRouter({ target: "react" })],
  resolve: {
    dedupe: ["react", "react-dom", "lucide-react"],
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
  test: {
    environment: "jsdom",
    server: { deps: { inline: [/@base-ui/, /react-hook-form/, /motion/, /lucide-react/] } },
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
