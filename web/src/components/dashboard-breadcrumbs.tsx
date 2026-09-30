import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { createPortal } from "react-dom";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
} from "react";

type AgentCrumb = Pick<Agent, "id" | "name" | "avatarSeed" | "avatarUrl">;
/** A listing page between the section and a detail page, e.g. Tools › Connections › Tavily. */
export type ParentCrumb = { label: string; to: "/tools/connections" | "/tools/remote-servers" };
/** The last crumb of a detail page, keyed by the path it names. */
type PageCrumb = { path: string; label: string; parent?: ParentCrumb };
const BreadcrumbContext = createContext<{
  agent?: AgentCrumb;
  page?: PageCrumb;
  setPage: Dispatch<SetStateAction<PageCrumb | undefined>>;
  navigationHost: HTMLDivElement | null;
  setNavigationHost: Dispatch<SetStateAction<HTMLDivElement | null>>;
  actionsHost: HTMLDivElement | null;
  setActionsHost: Dispatch<SetStateAction<HTMLDivElement | null>>;
  setAgent: Dispatch<SetStateAction<AgentCrumb | undefined>>;
} | null>(null);

/** Route data supplies the display name without making the dashboard fetch the agent again. */
export function DashboardBreadcrumbProvider({ children }: { children: ReactNode }) {
  const [agent, setAgent] = useState<AgentCrumb>();
  const [page, setPage] = useState<PageCrumb>();
  const [navigationHost, setNavigationHost] = useState<HTMLDivElement | null>(null);
  const [actionsHost, setActionsHost] = useState<HTMLDivElement | null>(null);
  const value = useMemo(
    () => ({
      agent,
      setAgent,
      page,
      setPage,
      navigationHost,
      setNavigationHost,
      actionsHost,
      setActionsHost,
    }),
    [agent, page, navigationHost, actionsHost],
  );
  return <BreadcrumbContext.Provider value={value}>{children}</BreadcrumbContext.Provider>;
}
export function useDashboardBreadcrumbs() {
  const value = useContext(BreadcrumbContext);
  if (!value) throw new Error("DashboardBreadcrumbProvider is missing");
  return value;
}

/** Pages contribute header navigation while retaining their React contexts and local state.
 * Standalone page renders keep the navigation inline when no dashboard host is present.
 */
export function DashboardNavigation({ children }: { children: ReactNode }) {
  const context = useContext(BreadcrumbContext);
  return context?.navigationHost ? createPortal(children, context.navigationHost) : <>{children}</>;
}
/** Page-level actions shown at the right of the breadcrumb row. */
export function DashboardActions({ children }: { children: ReactNode }) {
  const context = useContext(BreadcrumbContext);
  return context?.actionsHost ? createPortal(children, context.actionsHost) : <>{children}</>;
}
/** Names the current detail page in the header; a no-op outside the dashboard. */
export function usePageCrumb(path: string, label: string | undefined, parent?: ParentCrumb) {
  const setPage = useContext(BreadcrumbContext)?.setPage;
  const parentLabel = parent?.label;
  const parentTo = parent?.to;
  useEffect(() => {
    if (!setPage || !label) return;
    setPage({
      path,
      label,
      parent: parentLabel && parentTo ? { label: parentLabel, to: parentTo } : undefined,
    });
    return () => setPage(undefined);
  }, [setPage, path, label, parentLabel, parentTo]);
}
