import { createRouter, type RouterHistory } from "@tanstack/react-router";
import { routeTree } from "./routeTree.gen";

/** The router lives below the Vite base, so one build serves at `/` or below `/app`. */
export const basepath = import.meta.env.BASE_URL.replace(/\/$/, "") || "/";
export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, history, basepath });
}
declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
