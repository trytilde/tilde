import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      "@trytilde/connection-ui": fileURLToPath(
        new URL("../sdk/ts/packages/connection-ui/src/index.ts", import.meta.url),
      ),
    },
  },
  test: {
    environment: "jsdom",
    server: { deps: { inline: [/@base-ui/, /react-hook-form/] } },
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
